# Project: pg_grpc

PostgreSQL extension for making gRPC calls from SQL. Built with Rust and pgrx 0.18. Supports Postgres 13–18.

## Quick Start

```bash
cargo pgrx init --pg18        # first time only (picks Postgres version)
cargo pgrx run pg18           # build + start postgres with extension loaded
```

In psql (auto-opened):
```sql
CREATE EXTENSION pg_grpc;

-- Reflection path (server must expose gRPC reflection)
SELECT grpc_call('localhost:50051', 'pkg.Service/Method', '{"foo": "bar"}'::jsonb);

-- User-supplied proto path (no reflection needed)
SELECT grpc_proto_stage('api.proto', $$ syntax = "proto3"; package pkg; service Service { rpc Method(M) returns (M); } message M { string foo = 1; } $$);
SELECT grpc_proto_compile();
SELECT grpc_call('localhost:50051', 'pkg.Service/Method', '{"foo": "bar"}'::jsonb);
```

## Commands

| Command                             | Description                                    |
| ----------------------------------- | ---------------------------------------------- |
| `cargo pgrx run pg18`               | Build and start Postgres with extension loaded |
| `cargo pgrx test pg18`              | Run all pgrx tests (inside real Postgres)      |
| `cargo pgrx test pg18 -- test_name` | Run a single test                              |
| `cargo pgrx install --release`      | Install to system Postgres                     |
| `cargo clippy`                      | Lint                                           |
| `cargo fmt`                         | Format                                         |

## Project Structure

```
src/
├── lib.rs              # pg_module_magic!(), _PG_init (rustls ring), all #[pg_extern] SQL functions, #[pg_test]s
├── error.rs            # Centralized GrpcError enum + GrpcResult<T> type alias
├── call.rs             # gRPC call orchestration: parse_method → channel_cache → resolve_pool → encode → unary_call → decode
├── channel_cache.rs    # CHANNELS static: (endpoint, Option<TlsConfig>) → Channel, with auto-reconnect via tonic
├── endpoint.rs         # validate_endpoint: reject scheme/path/empty, trim whitespace
├── tls.rs              # TlsConfig: strict JSONB parsing → tonic::transport::ClientTlsConfig
├── proto.rs            # Reflection (fetch_pool) + user-supplied proto compilation (compile_proto_files)
├── proto_staging.rs    # PENDING_FILES static: grpc_proto_stage writes here, grpc_proto_compile drains
└── proto_registry.rs   # PROTO_REGISTRY static: compiled DescriptorPools keyed by fully-qualified service name
pg_grpc.control         # Extension metadata (name, version, schema, superuser)
```

`sql/` is not used — all schema is generated from `#[pg_extern]` attributes by pgrx.

## Dependencies

- **pgrx 0.18** — Postgres extension framework
- **tonic 0.14 / tonic-reflection 0.14** — gRPC client, reflection (features `channel`, `codegen`, `tls-ring`, `tls-native-roots`)
- **rustls 0.23** (`ring` feature) — provider installed once at `_PG_init`
- **prost 0.14 / prost-types 0.14 / prost-reflect 0.16** — protobuf encode/decode + dynamic schema
- **protox 0.9** — pure-Rust `.proto` compiler (used by `compile_proto_files`)
- **once_cell + parking_lot** — process-global staging / registry statics
- **tokio (rt, net, time)** — single-threaded runtime per SQL call
- **thiserror, futures, http, bytes, serde, serde_json** — support

All version constraints are pinned tightly; don't bump individual crates without re-running `cargo pgrx test pg18` (protox, prost, prost-reflect, and tonic must all agree on `prost-types` major versions).

## Code Style

- Rust 2021 edition
- Use `thiserror` for error types — never `impl Error` by hand
- Use `pgrx::JsonB` for JSON parameters and returns, not `String`
- Prefer `parking_lot::RwLock` over `std::sync::RwLock`
- All SQL-exposed functions: snake_case (`grpc_call`, not `grpcCall`)
- Use `Arc<T>` (or `Arc`-backed types like `DescriptorPool`, `Channel`) for data shared across SQL calls — clone out of the lock, release the lock before use, never hold a lock across an `await`

## Errors

Central error type lives in [src/error.rs](src/error.rs):

```rust
#[derive(Debug, Error)]
pub enum GrpcError {
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Proto error: {0}")]
    Proto(String),
    #[error("Proto compile error: {0}")]
    ProtoCompile(String),              // protox compilation failures for grpc_proto_stage
    #[error("gRPC call failed: {0}")]
    Call(String),
}

pub type GrpcResult<T> = Result<T, GrpcError>;
```

At the SQL boundary ([src/lib.rs](src/lib.rs)), convert to pgrx errors:
```rust
match result {
    Ok(v) => v,
    Err(e) => pgrx::error!("{}", e),   // raises Postgres ERROR, aborts statement
}
```

Rules:
- NEVER panic — use `Result` and convert at the SQL boundary
- NEVER `unwrap()` in library code; only in tests
- Map `tonic::Status` variants to meaningful `GrpcError` variants

## SQL Surface

```rust
#[pg_extern]
fn grpc_call(
    endpoint: &str,
    method: &str,
    request: pgrx::JsonB,
    metadata: default!(Option<pgrx::JsonB>, "null"),   // gRPC metadata / headers
    options: default!(Option<pgrx::JsonB>, "null"),    // transport / runtime knobs
) -> pgrx::JsonB
```

`metadata` is a JSON object whose values are strings or arrays of strings. Keys are silently lowercased. Keys ending in `-bin` (binary metadata) are rejected in v1.

`options` is a strict-parsed JSONB blob holding all per-call transport/runtime config. `NULL` or omitted keys leave defaults in place. Unknown keys raise a `Connection error` listing the accepted set, and per-key type/range errors carry the offending path (e.g. `options.timeout_ms must be an integer`). Accepted keys:

| Key                              | Type    | Validation                | Default behavior when omitted                 |
| -------------------------------- | ------- | ------------------------- | --------------------------------------------- |
| `timeout_ms`                     | integer | `>= 1`                    | 30_000 ms                                     |
| `use_reflection`                 | boolean | —                         | `true`                                        |
| `tls`                            | object  | delegated to TLS parser   | NULL → plaintext                              |
| `max_decode_message_size_bytes`  | integer | `[1, 4_294_967_295]`      | tonic default (4 MiB)                         |
| `max_encode_message_size_bytes`  | integer | `[1, 4_294_967_295]`      | tonic default (unbounded)                     |

Example mixing several keys:

```sql
SELECT grpc_call('host:port', 'pkg.S/M', '{}'::jsonb,
  options => '{
    "timeout_ms": 5000,
    "use_reflection": true,
    "tls": {"ca_cert": "<PEM>"},
    "max_decode_message_size_bytes": 67108864,
    "max_encode_message_size_bytes": 4194304
  }'::jsonb);
```

The size knobs apply to both the unary call and the reflection fetch on the same channel — a generous decode limit lets you both receive a large response and reflect a large schema. The 4_294_967_295 ceiling is the gRPC wire framing limit (4-byte length prefix); larger values can never be a valid single-message size.

`tls` (when supplied) controls transport security. `'{}'::jsonb` turns on TLS with the OS trust store; `'{"ca_cert": "<PEM>"}'::jsonb` adds a private-CA root on top. Reflection runs over the same channel, so `use_reflection => true` with a non-null `tls` reflects over TLS. The accepted inner fields are `ca_cert`, `client_cert`, `client_key`, and `domain_name`. For mTLS, `client_cert` and `client_key` must be set together (one without the other is a parse error); when both are present they're attached as a tonic `Identity`. `domain_name` overrides the SNI / certificate-verification name — needed for IP endpoints or when the server cert's CN/SAN doesn't match the dialed host.

User-supplied proto management (all `#[pg_extern]` in lib.rs):

| Function                                          | Purpose                                                                                                                                                             |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `grpc_proto_stage(filename TEXT, source TEXT)`    | Stage a `.proto` file for the next compile. Overwrites on duplicate filename.                                                                                       |
| `grpc_proto_unstage(filename TEXT) → bool`        | Remove one staged file. Returns `true` if removed. Registry untouched.                                                                                              |
| `grpc_proto_unstage_all()`                        | Clear every staged file. Registry untouched.                                                                                                                        |
| `grpc_proto_compile()`                            | Compile all staged files, resolving cross-imports and Google WKTs, then insert every service into the registry. Clears staging on success; preserves it on failure. |
| `grpc_proto_unregister(service_name TEXT) → bool` | Remove one compiled service by fully-qualified name (e.g. `"pkg.Service"`).                                                                                         |
| `grpc_proto_unregister_all()`                     | Remove every compiled service. Staging untouched.                                                                                                                   |

Rules for adding new functions:
- Always `#[pg_extern]`; use `name = "..."` to rename SQL-side if needed
- `default!(Type, "sql_literal")` for optional params with SQL-side defaults
- Return `pgrx::JsonB` for JSON, never `String`

## Async / Tokio

pgrx runs in a single-threaded Postgres backend. Never use `#[tokio::main]`.

```rust
fn make_grpc_call() -> GrpcResult<JsonB> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| GrpcError::Connection(e.to_string()))?;
    rt.block_on(async {
        // tonic calls here
    })
}
```

One runtime per SQL function call — do not store a runtime in global state.

## Proto Resolution

Every entry lives in a single `PROTO_REGISTRY`, tagged with an `Origin`:

```rust
enum Origin {
    UserStaged { filename, source },   // inserted via grpc_proto_compile
    Reflection { endpoint },           // inserted lazily on grpc_call miss
}
```

Lookup on every `grpc_call`:

```rust
let pool = match proto_registry::get_proto(&service_name) {
    Some(pool) => pool,                                         // cache hit (either origin)
    None => {
        let pool = proto::fetch_pool(channel, &service_name).await?;
        proto_registry::insert_proto_reflection(&service_name, pool.clone(), endpoint);
        pool
    }
};
```

`grpc_proto_unregister[_all]` is the only way to force a reflection re-fetch within a backend process.

### 1. User-supplied proto (via staging → compile → registry)

```rust
// proto_staging.rs — pending files awaiting compile
static PENDING_FILES: Lazy<RwLock<HashMap<String, String>>> = ...;

// proto_registry.rs — entries keyed by fully-qualified service name, each tagged with Origin
static PROTO_REGISTRY: LazyLock<RwLock<HashMap<String, RegisteredService>>> = ...;
```

Flow:
```
grpc_proto_stage("a.proto", "<src>")   → PENDING_FILES.insert("a.proto", "<src>")
grpc_proto_stage("b.proto", "<src>")   → PENDING_FILES.insert("b.proto", "<src>")
grpc_proto_compile()
  ├─ snapshot PENDING_FILES
  ├─ proto::compile_proto_files(snapshot)
  │    ├─ protox::Compiler with InMemoryResolver + GoogleFileResolver
  │    ├─ open_file(name) for each pending file (idempotent if already added)
  │    └─ file_descriptor_set → DescriptorPool
  ├─ for svc in pool.services(): proto_registry::insert_proto_manual(svc.full_name(), pool.clone(), ..)
  └─ PENDING_FILES.clear() on success; leave intact on error
```

The `InMemoryResolver` in `proto.rs` implements `protox::file::FileResolver` — it serves staged files by filename. It's chained with `GoogleFileResolver` so imports like `google/protobuf/timestamp.proto` resolve against protox's bundled WKT copies. No filesystem, no network.

After compile, `backfill_wkts` seeds every pool with `prost_reflect::DescriptorPool::global()`'s bundled WKTs. This lets `Any` payloads referencing a WKT (`google.protobuf.StringValue`, `Timestamp`, `Duration`, …) resolve at encode time even when the user proto only imports `any.proto`. User-staged files keep priority: same-name files added before the backfill are not overridden.

### 2. Reflection (fallback when nothing is registered)

`proto::fetch_pool(channel, service_name)` calls the server's `grpc.reflection.v1alpha.ServerReflection` service with a `FileContainingSymbol` request, decodes the streamed `FileDescriptorProto`s, and builds a `DescriptorPool`. The first call for a given service populates `PROTO_REGISTRY` with `Origin::Reflection { endpoint }`; subsequent calls hit the cache. Reflection caching is per-backend-process — reconnect (or `grpc_proto_unregister`) to force a refresh.

## Proto Resolution Flow (full picture)

```
grpc_call("host:port", "pkg.Service/Method", {...}, tls => NULL | '{...}')
    │
    ├─► parse_method → ("pkg.Service", "Method")
    ├─► tls::TlsConfig::parse(tls) (if non-null, strict; unknown keys rejected)
    ├─► channel_cache::get_or_connect(endpoint, tls)
    │       key = (endpoint, Option<TlsConfig>); http:// when tls is None, https:// otherwise
    ├─► proto_registry::get_proto("pkg.Service")
    │       ├─► hit  → use cached pool (user-staged or reflection)
    │       └─► miss → proto::fetch_pool via reflection, then insert_reflection
    ├─► resolve_method(pool, service, method)      (look up MethodDescriptor)
    ├─► encode_request(input_desc, json)           (JSON → DynamicMessage → bytes)
    ├─► unary_call(channel, service, method, bytes) with RawBytesCodec
    └─► decode_response(output_desc, bytes)        (bytes → DynamicMessage → JSON)
```

`src/call.rs` owns the whole pipeline; each step is a small private function. The `RawBytesCodec` (same file) is a tonic `Codec` that passes raw request/response bytes without needing generated prost types, which is what lets us drive gRPC calls from runtime-resolved schemas.

## Testing

Tests run inside real Postgres via pgrx. Example:

```rust
#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_grpc_call_dummyunary() {
        let result = crate::grpc_call(
            "grpcb.in:9000",
            "grpcbin.GRPCBin/DummyUnary",
            pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
            None, None, None, None,
        );
        assert_eq!(result.0["f_string"], "hello");
    }
}
```

```bash
cargo pgrx test pg18                                  # all tests
cargo pgrx test pg18 -- test_grpc_call_dummyunary     # single test
```

`#[cfg(any(test, feature = "pg_test"))]` — both conditions are required for the pgrx test runner. Existing tests target `grpcb.in:9000` as a real endpoint — they will fail if the process has no outbound network.

Tests share a single backend process, so the `PROTO_REGISTRY` and `PENDING_FILES` statics are shared across tests in one run. Each test that mutates them should clean up at the end (`grpc_proto_unregister_all` / `grpc_proto_unstage_all`) to avoid affecting siblings.

## Development Workflow

1. Edit `src/`
2. In psql: `DROP EXTENSION IF EXISTS pg_grpc; CREATE EXTENSION pg_grpc;`
3. Or: exit psql and `cargo pgrx run pg18` again

## Git Workflow

**Branches:** `<type>/<short-kebab-name>` — e.g. `feat/mtls`, `feat/tls-support`, `fix/validate-endpoint`, `chore/repo-housekeeping`, `test/hermetic-grpcbin-service`. Type matches the dominant commit type on the branch. Keep names short (1–3 words).

**Commit messages:** `type: subject` — lowercase type, lowercase first word after the colon, ~50–60 chars, no scope, no `Co-Authored-By` trailer.

| Type       | Use for                                             |
| ---------- | --------------------------------------------------- |
| `feat`     | new behavior, new module, new SQL function or field |
| `fix`      | bug fix; usually has a body explaining root cause   |
| `test`     | adding or tightening tests without behavior change  |
| `refactor` | code reshape, no behavior change                    |
| `docs`     | README / CLAUDE.md / inline doc updates             |
| `chore`    | release commits, dep bumps, repo hygiene            |
| `build`    | Cargo features, overall build config                |

**Bodies:** routine `feat`/`test`/`docs`/`refactor` commits stay one line. `fix` commits get a body when the root cause is non-obvious — state what was wrong, then what changed. Wrap body at ~72 chars.

**PR shape:** every branch lands via merge commit (`Merge pull request #N from <user>/<branch>`) — no squash. Inside the PR, prefer a series of small atomic commits over one big one: introduce a unit and its first test together, then layer follow-up `test:` commits on top (`feat: add channel_cache module with first lookup test` → `test: second lookup same endpoint is a cache hit` → `refactor: route grpc_call through channel_cache`).

**Releases:** `chore: Release pg_grpc version X.Y.Z`, produced by `cargo release` (see Release section).

## Common Issues

| Issue                                             | Solution                                                                                                                                          |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo pgrx init` fails                           | Run inside Docker, not host                                                                                                                       |
| Extension not found                               | `cargo pgrx install --release` or restart `pgrx run`                                                                                              |
| Async/threading panic                             | Use `new_current_thread()` — never multi-thread                                                                                                   |
| `Proto error: service not found`                  | Server lacks reflection AND nothing is staged — use `grpc_proto_stage` + `grpc_proto_compile`                                                     |
| Connection refused                                | Endpoint is `host:port`, no `http://` prefix                                                                                                      |
| Cache stale                                       | Reconnect to get a fresh backend process (staging + registry are per-process)                                                                     |
| `grpc_proto_compile` fails                        | Bad file is still staged — `grpc_proto_unstage('bad.proto')` or re-stage with fixed source, then compile again. Registry is untouched by failure. |
| Version mismatch between protox and prost-reflect | protox 0.9 needs prost-types 0.14 / prost-reflect 0.16; older protox lines pair with older prost. Don't mix.                                      |

## Current Limitations

- **Unary RPCs only** — streaming methods not supported
- **Multi-file proto imports must use filenames that match staging keys** — `import "common.proto";` only resolves if someone ran `grpc_proto_stage('common.proto', ...)`

## API Summary

```sql
-- gRPC call
grpc_call(endpoint TEXT, method TEXT, request JSONB [, metadata JSONB] [, options JSONB]) RETURNS JSONB
-- options keys: timeout_ms, use_reflection, tls, max_decode_message_size_bytes, max_encode_message_size_bytes

-- Staging
grpc_proto_stage(filename TEXT, source TEXT) RETURNS VOID
grpc_proto_unstage(filename TEXT) RETURNS BOOLEAN
grpc_proto_unstage_all() RETURNS VOID

-- Compile (staging → registry)
grpc_proto_compile() RETURNS VOID

-- Registry
grpc_proto_unregister(service_name TEXT) RETURNS BOOLEAN
grpc_proto_unregister_all() RETURNS VOID
```

## Release

cargo release [LEVEL|VERSION] -c release.toml