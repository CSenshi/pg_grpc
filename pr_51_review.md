# PR #51 Review — `grpc_call_async`

> Reviewer: Claude Code (2026-05-04)
> Files reviewed: `src/async_schema.rs`, `src/call.rs`, `src/guc.rs`, `src/lib.rs`, `src/queue.rs`, `src/shmem.rs`, `src/worker.rs`, `src/tests/call_async.rs`, `src/tests/queue.rs`, `src/tests/guc.rs`, `website/docs/guides/async-calls.md`, `website/docs/reference.md`

---

## Summary

The PR introduces a background-worker-based async gRPC dispatch system: `grpc_call_async` enqueues calls into a Postgres table; a background worker dequeues, executes, and writes results back. The architecture is sound (modelled after `pg_net`) and the test coverage is thorough. There are a few **critical correctness issues**, several **design-level concerns** that need to be decided before merge, and a handful of smaller code-quality items.

---

## Critical

### 1. `RegisterXactCallback` accumulates callbacks across transactions — potential memory leak

**File:** [src/lib.rs](src/lib.rs)

```rust
if !WAKE_CB_ACTIVE.swap(true, Ordering::Relaxed) {
    unsafe {
        pg_sys::RegisterXactCallback(Some(wake_worker_on_commit), std::ptr::null_mut());
    }
}
```

`RegisterXactCallback` appends to a process-global list and **never removes entries**. `WAKE_CB_ACTIVE` only prevents duplicate registration within a single transaction, but is reset to `false` after each commit. On the next transaction, the guard sees `false` again and registers a second callback. After N transactions with at least one `grpc_call_async` call each, the backend has N registered callbacks, all firing on every subsequent commit. Each one is a no-op (the CAS fails after the first), but the list grows unboundedly for the lifetime of a long-lived connection.

**Fix:** Use a second boolean to track whether the callback has ever been registered in this backend process:

```rust
static XACT_CB_REGISTERED: AtomicBool = AtomicBool::new(false);

if !XACT_CB_REGISTERED.swap(true, Ordering::Relaxed) {
    unsafe {
        pg_sys::RegisterXactCallback(Some(wake_worker_on_commit), std::ptr::null_mut());
    }
}
// Then separately guard the wake signal with WAKE_CB_ACTIVE as before.
```

---

### 2. Dequeued rows are permanently lost if the worker crashes between `dequeue` and `insert_results`

**File:** [src/worker.rs](src/worker.rs)

```rust
let rows = queue::dequeue(batch_size); // DELETE … RETURNING: rows gone from queue

// -- worker crashes here: SIGKILL, OOM, ereport(FATAL) --

queue::insert_results(results);        // never reached
queue::ttl_cleanup(&guc::ttl());
```

The `DELETE … RETURNING` in `dequeue` atomically removes rows from `call_queue` in the same transaction. If the worker process dies (SIGKILL, OOM, any `ereport(FATAL)`) after the commit but before `insert_results` writes to `_call_result`, those calls vanish silently. Neither the queue nor the result table will have any trace of them.

`on_worker_exit` sets `should_wake = true`, which causes the restarted worker to process new items—but the lost in-flight rows are gone. Users will see the `id` returned by `grpc_call_async` disappear from both tables and get `"not found"` from `grpc_call_result`.

This is a fundamental property of the current design. It **must be explicitly documented** in the docs (`async-calls.md`) under a "Reliability" or "Limitations" section so users understand the at-most-once delivery guarantee. Silently swallowing calls is worse than a clear documented limitation.

---

### 3. Silent `unwrap_or_default()` swallows options parse errors at execution time

**File:** [src/call.rs](src/call.rs), line 49

```rust
let opts = match &row.options {
    None => crate::options::OptionsConfig::default(),
    Some(v) => crate::options::OptionsConfig::parse(v).unwrap_or_default(),  // ← silently drops error
};
```

Options are validated at enqueue time (in `grpc_call_async`), so in normal operation this should never fail. But if an old row was inserted by a previous extension version with different options semantics, or if `_call_result` is ever manually inserted, a bad `options` blob silently falls back to defaults—including a different `tls` config or `use_reflection` flag. The call may then fail for a completely opaque reason. 

Better to log a `pgrx::warning!` and continue with defaults, or to propagate an error into the result as `CallOutcome::Error`.

---

### 4. `timeout_ms` overflow on cast `u64 → i32`

**File:** [src/lib.rs](src/lib.rs), line 234

```rust
let timeout_ms = opts.timeout_ms.unwrap_or(30_000) as i32;
```

`opts.timeout_ms` is parsed as a JSON integer (potentially up to `i64::MAX`). Casting directly to `i32` silently overflows for any value above `2_147_483_647` (about 24 days). The stored `i32` will be negative, and then in `call_async_row`:

```rust
let timeout_ms = row.timeout_ms as u64;  // negative i32 wraps to huge u64
```

A user passing `"timeout_ms": 9_999_999_999` gets a ~596 year timeout instead of an error.

**Fix:** Validate at enqueue that `timeout_ms` fits in `i32`, or store it as `bigint` / use `i64` throughout.

---

## Major

### 5. UNLOGGED tables lose data on crash — not clearly documented

**File:** [src/async_schema.rs](src/async_schema.rs)

```sql
CREATE UNLOGGED TABLE grpc.call_queue ( … );
CREATE UNLOGGED TABLE grpc._call_result ( … );
```

Both tables are `UNLOGGED`. A server crash causes PostgreSQL to truncate them on recovery (not roll back—completely truncate). Every pending call, every stored result, every in-flight row disappears. This is appropriate for a performance-first design, but it's surprising enough to warrant a dedicated callout in the documentation.

The current docs (`async-calls.md`) do not mention this at all. A user who stores `grpc_call_async` IDs in their application and then experiences a crash will find those IDs return `"not found"`.

**Recommendation:** Add a "Reliability guarantee" section to `async-calls.md` that explicitly states:
- Delivery is at-most-once (crash can lose enqueued or in-flight calls)
- Both tables are UNLOGGED and are truncated on crash recovery
- Result TTL is wall-clock based; a very short TTL can cause results to disappear before the caller polls

---

### 6. Single-database limitation is undocumented

**File:** [src/guc.rs](src/guc.rs), `pg_grpc.database_name`

The worker connects to exactly one database. If `pg_grpc` is installed in multiple databases on the same cluster, only one database's queue is processed. Calls enqueued in other databases sit indefinitely.

The docs mention `pg_grpc.database_name` but do not warn about this constraint. This should be stated clearly in the configuration table.

---

### 7. `grpc_call_result` with `async = false` can block indefinitely

**File:** [src/lib.rs](src/lib.rs)

```rust
loop {
    let r = queue::lookup(id);
    match r.status {
        queue::LookupStatus::Pending => unsafe {
            pg_sys::WaitLatch(pg_sys::MyLatch, …, 50, pg_sys::PG_WAIT_EXTENSION);
            pg_sys::ResetLatch(pg_sys::MyLatch);
            pg_sys::check_for_interrupts!();
        },
        _ => break r,
    }
}
```

If the worker crashes and is not restarted (e.g., `max_worker_processes` exhausted), or if the called ID simply never appears in results (e.g., it was TTL-cleaned before the poll started), this loop runs forever at 50 ms intervals. The only escape is `check_for_interrupts` (catches Ctrl-C / `pg_cancel_backend`) or `statement_timeout`. Neither is mentioned in the docs.

Additionally, the worker does **not** signal the caller's latch when a result is written, so polling will always incur the full 50 ms wait even if the result appears milliseconds later. The docs correctly note this ("the wait is purely time-based"), but without a maximum wait cap the function is not safe to use in production code without a surrounding `statement_timeout`.

**Recommendation:** Document the requirement for `statement_timeout`, or add an optional `timeout_ms` parameter to `grpc_call_result`.

---

### 8. `"not found"` conflates TTL expiry and genuine errors

**File:** [src/queue.rs](src/queue.rs), `lookup`

```rust
LookupResult {
    id,
    status: LookupStatus::Error("not found — result may have expired".to_string()),
}
```

When a caller polls `grpc_call_result(id)` and the row has been TTL-cleaned (or was never written due to a crash), the function returns `status = 'ERROR'` with `message = 'not found — result may have expired'`. From the caller's perspective, this is indistinguishable from a real gRPC error. An application that logs all `ERROR` statuses will receive false positives.

A distinct `EXPIRED` (or `NOT_FOUND`) status would allow callers to handle expiry separately from actual call failures.

---

### 9. `options` blob and `timeout_ms` column are redundant

**File:** [src/async_schema.rs](src/async_schema.rs)

`call_queue` has both a `timeout_ms` column (extracted from options at enqueue time) and an `options` column (the raw blob, which also contains `timeout_ms`). At execution time the worker uses the column, not the blob. The `options.timeout_ms` field inside the JSONB is effectively ignored after enqueueing.

This is confusing for anyone who inspects the queue directly. Consider either:
- Removing `timeout_ms` from the column and parsing it back from `options` at execution time (single source of truth), or
- Adding a comment to the schema SQL explaining why the column exists alongside `options`.

---

### 10. `guc::ttl()` invalid value crashes the worker into a restart loop

**File:** [src/queue.rs](src/queue.rs), `ttl_cleanup`

```rust
pub fn ttl_cleanup(ttl: &str) {
    Spi::connect_mut(|client| {
        client.update(
            "DELETE FROM grpc._call_result WHERE created < now() - $1::interval",
            …
        )
        .unwrap_or_else(|e| {
            pgrx::error!("TTL cleanup failed (pg_grpc.ttl = {:?}): {}", ttl, e)
        });
    });
}
```

`pgrx::error!` in a background worker calls `ereport(ERROR)`, which `pg_guard` converts to a Rust panic, which propagates out of the worker's main loop, killing the process. Since `set_restart_time(Some(Duration::from_secs(1)))` is set, it restarts—but the GUC still has the invalid interval, so it crashes again immediately in an infinite 1-second restart loop.

**Fix:** Change `pgrx::error!` to `pgrx::warning!` in the worker context for TTL failures, or validate the GUC as a valid interval string with a `check_hook` during `GucRegistry::define_string_guc`.

---

## Minor

### 11. `jsonb_to_value` panics on unconvertible JSON

**File:** [src/queue.rs](src/queue.rs), line 548

```rust
fn jsonb_to_value(v: Option<pgrx::JsonB>) -> JsonValue {
    v.map(|j| serde_json::from_str(&j.0.to_string()).unwrap())
        .unwrap_or(JsonValue::Null)
}
```

`serde_json::from_str(…).unwrap()` panics if `j.0.to_string()` produces invalid JSON (edge case: pgrx `JsonB::to_string()` should always produce valid JSON, but it is an internal invariant not a guarantee). Use `unwrap_or(JsonValue::Null)` or propagate through a `Result`.

---

### 12. Dequeue does not protect against future multi-worker scenarios

**File:** [src/queue.rs](src/queue.rs), `dequeue`

```sql
WITH rows AS (
    SELECT id FROM grpc.call_queue ORDER BY id LIMIT $1
)
DELETE FROM grpc.call_queue q USING rows …
```

Without `FOR UPDATE SKIP LOCKED` in the CTE, two concurrent worker processes would attempt to delete the same rows, with one blocking on the other's lock. The current single-worker design avoids this, but if a second worker is ever added (or a user accidentally starts a second instance), contention will occur rather than graceful lock skipping.

Changing to `SELECT id FROM grpc.call_queue ORDER BY id LIMIT $1 FOR UPDATE SKIP LOCKED` costs nothing now and avoids a hard-to-debug latency issue later.

---

### 13. `async` is a Rust keyword used as a parameter name

**File:** [src/lib.rs](src/lib.rs)

```rust
fn grpc_call_result(
    id: i64,
    r#async: default!(bool, true),
```

`r#async` is valid Rust but unusual and forces readers to look twice. The SQL surface exposes it as `async` which is a reserved word in SQL:2003. Consider renaming to `wait` or `blocking`:

```sql
grpc_call_result(id, wait => false)     -- clearer intent
grpc_call_result(id, blocking => true)  -- alternative
```

---

### 14. Tests use hardcoded large IDs that may conflict across runs

**File:** [src/tests/call_async.rs](src/tests/call_async.rs), [src/tests/queue.rs](src/tests/queue.rs)

Tests insert rows with explicit IDs like `10001`, `10002`, `20001`, `20002`, etc. All pgrx tests share one backend process, so if a test leaves a row behind on failure (e.g., the assertion panics before the `DELETE`), the next run will fail with a primary-key conflict. Since `call_queue` is a `bigserial`, it would be safer to either:
- Not specify IDs and let the sequence allocate them, or
- Use `ON CONFLICT DO NOTHING` / `TRUNCATE` in a test setup step.

---

### 15. `test_call_async_rollback_no_row` does not test wake suppression

**File:** [src/tests/call_async.rs](src/tests/call_async.rs)

The test verifies that the queue row is rolled back (correct), but does not verify that `WAKE_CB_ACTIVE` is reset to `false` after the abort. If the flag is still `true` after a rollback, the next transaction's call to `grpc_call_async` will not re-register the callback (given the accumulation bug in point 1), meaning the worker might not be woken. A test assertion of `WAKE_CB_ACTIVE.load(…) == false` after the rollback would catch this.

---

### 16. `grpc._call_result` table is internal but publicly visible

**File:** [src/async_schema.rs](src/async_schema.rs)

The underscore prefix on `_call_result` signals "internal", but the table is in the `grpc` schema with no row-level security or privilege restrictions. Any user with `USAGE` on the `grpc` schema can `SELECT * FROM grpc._call_result`, seeing all results for all users. For multi-tenant deployments this is a concern. Consider:
- Adding a `REVOKE ALL ON grpc._call_result FROM PUBLIC` and granting only to the worker role, or
- Documenting that the tables contain potentially sensitive response data and recommending schema-level access restrictions.

---

### 17. No `grpc_call_result_batch` function for fan-out pattern

**File:** [website/docs/guides/async-calls.md](website/docs/guides/async-calls.md)

The docs show the fan-out pattern (enqueue N calls in one statement), but there is no API to fetch multiple results at once. Users must either:
- Call `grpc_call_result(id)` N times, or
- Write their own query against `grpc._call_result` directly (exposing the internal table).

A `grpc_call_results(ids bigint[])` or `grpc_call_results_all()` returning a `SETOF` record would make the fan-out pattern much more usable.

---

## Documentation-only

### 18. `async-calls.md` does not mention `shared_preload_libraries` requirement

The configuration section shows a minimal `postgresql.conf` snippet but buries `shared_preload_libraries = 'pg_grpc'` only at the bottom. Background workers require `shared_preload_libraries` — without it, `grpc_call_async` inserts rows into `call_queue` that are never processed. This should be the **first** item in the setup section, with an explicit note that a server restart is required.

---

### 19. `async-calls.md` blocking-wait section understates the risk

> "Useful in scripts or tests where you want the result inline without writing a loop."

For production code, `async => false` without a `statement_timeout` is a potential hung connection. The docs should advise setting `statement_timeout` when using `async => false` outside of test/script contexts.

---

## Verdict

The core async architecture is well-designed and the test coverage is impressive. The three critical issues (#1 callback accumulation, #2 data loss on crash, #4 timeout overflow) need fixes before merge. Issues #5 (UNLOGGED tables undocumented) and #10 (TTL crash loop) are merge-blockers from a production-readiness perspective. The remaining items can be addressed as follow-up issues.
