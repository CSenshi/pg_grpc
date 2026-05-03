#[pg_test]
fn test_dequeue_on_fresh_transaction_does_not_error() {
    // No prior write in this transaction — is_xact_still_immutable() returns true,
    // so pgrx passes read_only=true to SPI_execute. PostgreSQL treats SELECT with a
    // data-modifying CTE as non-read-only and raises ERROR if read_only=true is passed.
    // This test verifies that dequeue() uses Spi::connect_mut (read_only=false) so the
    // UPDATE CTE executes correctly even as the very first operation in a fresh transaction.
    let rows = crate::queue::dequeue(10);
    assert_eq!(rows.len(), 0);
}

#[pg_test]
fn test_dequeue_returns_queued_row() {
    Spi::run(
        "INSERT INTO grpc.call_queue (endpoint, method, request)
         VALUES ('host:50051', 'pkg.S/M', '{\"key\": \"val\"}')",
    )
    .unwrap();

    let rows = crate::queue::dequeue(10);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].endpoint, "host:50051");
    assert_eq!(rows[0].method, "pkg.S/M");
    assert_eq!(rows[0].request["key"], "val");
    // dequeue uses DELETE ... RETURNING, so the row is already gone from the queue
}

#[pg_test]
fn test_dequeue_removes_row_preventing_double_dequeue() {
    Spi::run(
        "INSERT INTO grpc.call_queue (endpoint, method, request)
         VALUES ('host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();

    let first = crate::queue::dequeue(10);
    let second = crate::queue::dequeue(10);

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 0); // deleted by first dequeue, not visible to second
}

#[pg_test]
fn test_insert_results_writes_success() {
    Spi::run(
        "INSERT INTO grpc.call_queue (id, endpoint, method, request)
         VALUES (1001, 'host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();
    // dequeue atomically deletes the row; insert_results only writes to _call_result
    crate::queue::dequeue(10);

    crate::queue::insert_results(vec![crate::queue::CallResult {
        id: 1001,
        outcome: crate::queue::CallOutcome::Success(serde_json::json!({"ok": true})),
    }]);

    let queue_count = Spi::get_one::<i64>(
        "SELECT count(*)::bigint FROM grpc.call_queue WHERE id = 1001",
    )
    .unwrap()
    .unwrap();
    assert_eq!(queue_count, 0); // removed by dequeue, not by insert_results

    let result_status = Spi::get_one::<String>(
        "SELECT status FROM grpc._call_result WHERE id = 1001",
    )
    .unwrap()
    .unwrap();
    assert_eq!(result_status, "SUCCESS");

    Spi::run("DELETE FROM grpc._call_result WHERE id = 1001").unwrap();
}

#[pg_test]
fn test_lookup_returns_pending_for_queued_row() {
    Spi::run(
        "INSERT INTO grpc.call_queue (id, endpoint, method, request)
         VALUES (2001, 'host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();

    let result = crate::queue::lookup(2001);

    assert!(matches!(result.status, crate::queue::LookupStatus::Pending));

    Spi::run("DELETE FROM grpc.call_queue WHERE id = 2001").unwrap();
}

#[pg_test]
fn test_lookup_returns_success_for_completed_result() {
    Spi::run(
        "INSERT INTO grpc._call_result (id, status, response)
         VALUES (3001, 'SUCCESS', '{\"value\": 42}')",
    )
    .unwrap();

    let result = crate::queue::lookup(3001);

    assert!(matches!(result.status, crate::queue::LookupStatus::Success(_)));
    if let crate::queue::LookupStatus::Success(v) = result.status {
        assert_eq!(v["value"], 42);
    }

    Spi::run("DELETE FROM grpc._call_result WHERE id = 3001").unwrap();
}

#[pg_test]
fn test_lookup_returns_error_for_failed_result() {
    Spi::run(
        "INSERT INTO grpc._call_result (id, status, error_msg)
         VALUES (4001, 'ERROR', 'connection refused')",
    )
    .unwrap();

    let result = crate::queue::lookup(4001);

    assert!(matches!(result.status, crate::queue::LookupStatus::Error(_)));
    if let crate::queue::LookupStatus::Error(msg) = result.status {
        assert_eq!(msg, "connection refused");
    }

    Spi::run("DELETE FROM grpc._call_result WHERE id = 4001").unwrap();
}

#[pg_test]
fn test_lookup_returns_error_for_missing_id() {
    let result = crate::queue::lookup(99999);

    assert!(matches!(result.status, crate::queue::LookupStatus::Error(_)));
    if let crate::queue::LookupStatus::Error(msg) = result.status {
        assert!(msg.contains("not found"));
    }
}

#[pg_test]
fn test_ttl_cleanup_removes_expired_rows() {
    Spi::run(
        "INSERT INTO grpc._call_result (id, status, created)
         VALUES (5001, 'SUCCESS', now() - INTERVAL '7 hours')",
    )
    .unwrap();

    crate::queue::ttl_cleanup("6 hours");

    let count = Spi::get_one::<i64>(
        "SELECT count(*)::bigint FROM grpc._call_result WHERE id = 5001",
    )
    .unwrap()
    .unwrap();
    assert_eq!(count, 0);
}

fn queue_count() -> i64 {
    Spi::connect(|client| {
        client
            .select("SELECT count(*) FROM grpc.call_queue", None, &[])
            .unwrap()
            .first()
            .get_one::<i64>()
            .unwrap()
            .unwrap_or(0)
    })
}

// --- queue::count() tests (queue starvation fix) ---

#[pg_test]
fn test_count_returns_zero_on_empty_queue() {
    assert_eq!(queue_count(), 0);
}

#[pg_test]
fn test_count_returns_queue_depth() {
    Spi::run(
        "INSERT INTO grpc.call_queue (endpoint, method, request)
         VALUES ('host:50051', 'pkg.S/M', '{}'),
                ('host:50051', 'pkg.S/M', '{}'),
                ('host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();

    let n = queue_count();

    // Clean up before asserting so we don't leak rows on failure.
    Spi::run("DELETE FROM grpc.call_queue WHERE endpoint = 'host:50051' AND method = 'pkg.S/M'")
        .unwrap();

    assert_eq!(n, 3);
}

#[pg_test]
fn test_count_decreases_after_partial_dequeue() {
    Spi::run(
        "INSERT INTO grpc.call_queue (endpoint, method, request)
         VALUES ('host:50051', 'pkg.S/M', '{}'),
                ('host:50051', 'pkg.S/M', '{}'),
                ('host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();

    let before = queue_count();
    crate::queue::dequeue(2);
    let after = queue_count();

    Spi::run("DELETE FROM grpc.call_queue WHERE endpoint = 'host:50051' AND method = 'pkg.S/M'")
        .unwrap();

    assert_eq!(before, 3);
    assert_eq!(after, 1);
}

// Behavioral test: documents the multi-cycle drain contract.
//
// When the queue depth exceeds batch_size the worker must run multiple
// dequeue cycles.  This test simulates two cycles (without real gRPC)
// and asserts that all enqueued rows end up in _call_result.
// If the worker loop ever stops re-arming after a full batch this test
// serves as a specification of the expected drain behavior.
#[pg_test]
fn test_queue_is_fully_drained_across_two_cycles() {
    // Insert 3 rows; simulate a batch_size of 2.
    Spi::run(
        "INSERT INTO grpc.call_queue (id, endpoint, method, request)
         VALUES (7001, 'host:50051', 'pkg.S/M', '{}'),
                (7002, 'host:50051', 'pkg.S/M', '{}'),
                (7003, 'host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();

    let batch_size: i32 = 2;

    // Cycle 1: dequeue up to 2 rows; produce fake error results (no network).
    let first_batch = crate::queue::dequeue(batch_size);
    assert_eq!(first_batch.len(), 2, "first batch should fill batch_size");

    // A full batch means more rows may remain — count() confirms this.
    assert_eq!(
        queue_count(),
        1,
        "one row should remain after first batch"
    );

    crate::queue::insert_results(
        first_batch
            .into_iter()
            .map(|r| crate::queue::CallResult {
                id: r.id,
                outcome: crate::queue::CallOutcome::Error("simulated".to_string()),
            })
            .collect(),
    );

    // Cycle 2: drain the remainder.
    let second_batch = crate::queue::dequeue(batch_size);
    assert_eq!(second_batch.len(), 1, "second batch should drain the last row");
    assert_eq!(queue_count(), 0, "queue should be empty after second batch");

    crate::queue::insert_results(
        second_batch
            .into_iter()
            .map(|r| crate::queue::CallResult {
                id: r.id,
                outcome: crate::queue::CallOutcome::Error("simulated".to_string()),
            })
            .collect(),
    );

    // All three rows must have a result entry.
    let result_count = Spi::get_one::<i64>(
        "SELECT count(*)::bigint FROM grpc._call_result WHERE id IN (7001, 7002, 7003)",
    )
    .unwrap()
    .unwrap();
    assert_eq!(result_count, 3, "all rows must have results after two cycles");

    Spi::run("DELETE FROM grpc._call_result WHERE id IN (7001, 7002, 7003)").unwrap();
}
