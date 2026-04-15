# Project: pg_grpc

PostgreSQL extension for making gRPC calls from SQL. Built with Rust and pgrx 0.17. Supports Postgres 13–18.

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

| Command | Description |
|---------|-------------|
| `cargo pgrx run pg18` | Build and start Postgres with extension loaded |
| `cargo pgrx test pg18` | Run all pgrx tests (inside real Postgres) |
| `cargo pgrx test pg18 -- test_name` | Run a single test |
| `cargo pgrx install --release` | Install to system Postgres |
| `cargo clippy` | Lint |
| `cargo fmt` | Format |

## Project Structure

```
src/
├── lib.rs              # pg_module_magic!(), all #[pg_extern] SQL functions, #[pg_test]s
├── error.rs            # Centralized GrpcError enum + GrpcResult<T> type alias
├── call.rs             # gRPC call orchestration: parse_method → connect → resolve_pool → encode → unary_call → decode
├── proto.rs            # Reflection (fetch_pool) + user-supplied proto compilation (compile_proto_files)
├── proto_staging.rs    # PENDING_FILES static: grpc_proto_stage writes here, grpc_proto_compile drains
└── proto_registry.rs   # PROTO_REGISTRY static: compiled DescriptorPools keyed by fully-qualified service name
pg_grpc.control         # Extension metadata (name, version, schema, superuser)
```

`sql/` is not used — all schema is generated from `#[pg_extern]` attributes by pgrx.

## Dependencies

- **pgrx 0.17** — Postgres extension framework
- **tonic 0.14 / tonic-reflection 0.14** — gRPC client, reflection
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
    timeout_ms: default!(Option<i64>, "null"),   // accepted, not yet wired up
) -> pgrx::JsonB
```

User-supplied proto management (all `#[pg_extern]` in lib.rs):

| Function | Purpose |
|---|---|
| `grpc_proto_stage(filename TEXT, source TEXT)` | Stage a `.proto` file for the next compile. Overwrites on duplicate filename. |
| `grpc_proto_unstage(filename TEXT) → bool` | Remove one staged file. Returns `true` if removed. Registry untouched. |
| `grpc_proto_unstage_all()` | Clear every staged file. Registry untouched. |
| `grpc_proto_compile()` | Compile all staged files, resolving cross-imports and Google WKTs, then insert every service into the registry. Clears staging on success; preserves it on failure. |
| `grpc_proto_unregister(service_name TEXT) → bool` | Remove one compiled service by fully-qualified name (e.g. `"pkg.Service"`). |
| `grpc_proto_unregister_all()` | Remove every compiled service. Staging untouched. |

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

### 2. Reflection (fallback when nothing is registered)

`proto::fetch_pool(channel, service_name)` calls the server's `grpc.reflection.v1alpha.ServerReflection` service with a `FileContainingSymbol` request, decodes the streamed `FileDescriptorProto`s, and builds a `DescriptorPool`. The first call for a given service populates `PROTO_REGISTRY` with `Origin::Reflection { endpoint }`; subsequent calls hit the cache. Reflection caching is per-backend-process — reconnect (or `grpc_proto_unregister`) to force a refresh.

## Proto Resolution Flow (full picture)

```
grpc_call("host:port", "pkg.Service/Method", {...})
    │
    ├─► parse_method → ("pkg.Service", "Method")
    ├─► connect(endpoint)                          (new Channel every call — no caching)
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
            None,
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

## Common Issues

| Issue | Solution |
|-------|----------|
| `cargo pgrx init` fails | Run inside Docker, not host |
| Extension not found | `cargo pgrx install --release` or restart `pgrx run` |
| Async/threading panic | Use `new_current_thread()` — never multi-thread |
| `Proto error: service not found` | Server lacks reflection AND nothing is staged — use `grpc_proto_stage` + `grpc_proto_compile` |
| Connection refused | Endpoint is `host:port`, no `http://` prefix |
| Cache stale | Reconnect to get a fresh backend process (staging + registry are per-process) |
| `grpc_proto_compile` fails | Bad file is still staged — `grpc_proto_unstage('bad.proto')` or re-stage with fixed source, then compile again. Registry is untouched by failure. |
| Version mismatch between protox and prost-reflect | protox 0.9 needs prost-types 0.14 / prost-reflect 0.16; older protox lines pair with older prost. Don't mix. |

## Current Limitations

- **HTTP only** — TLS/mTLS not supported (no `tonic::transport::ClientTlsConfig` wiring yet)
- **Unary RPCs only** — streaming methods not supported
- **No connection caching** — fresh TCP + HTTP/2 handshake on every `grpc_call`
- **`timeout_ms` is accepted but ignored**
- **Multi-file proto imports must use filenames that match staging keys** — `import "common.proto";` only resolves if someone ran `grpc_proto_stage('common.proto', ...)`

## API Summary

```sql
-- gRPC call
grpc_call(endpoint TEXT, method TEXT, request JSONB [, timeout_ms BIGINT]) RETURNS JSONB

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
