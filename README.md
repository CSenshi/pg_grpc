# pg_grpc

Make gRPC calls directly from PostgreSQL SQL.

```sql
SELECT grpc_call('localhost:50051', 'package.Service/Method', '{"key": "value"}'::jsonb);
```

Schemas are resolved at runtime. Two sources are supported:

1. **gRPC server reflection** (default) — no `.proto` files needed, the server does the work.
2. **User-supplied `.proto` source** (`grpc_proto_stage` + `grpc_proto_compile`) — for servers without reflection, or when you want deterministic schemas.

## Calling a service

```sql
-- named
SELECT grpc_call(
    endpoint => 'grpcb.in:9000',
    method   => 'grpcbin.GRPCBin/DummyUnary',
    request  => '{"f_string": "hello"}'::jsonb
);

-- positional
SELECT grpc_call('grpcb.in:9000', 'grpcbin.GRPCBin/DummyUnary', '{"f_string": "hello"}'::jsonb);
```

### `grpc_call` signature

```sql
grpc_call(
    endpoint   TEXT,
    method     TEXT,
    request    JSONB,
    timeout_ms BIGINT DEFAULT NULL  -- accepted but not yet implemented
) RETURNS JSONB
```

| Parameter | Description |
|---|---|
| `endpoint` | `host:port` — no scheme (e.g. `localhost:50051`, not `http://localhost:50051`) |
| `method` | `package.Service/Method` (e.g. `grpcbin.GRPCBin/DummyUnary`) |
| `request` | Request payload; field names must match proto field names |

Returns the response as `JSONB` with proto field names (snake_case).

On every call, pg_grpc first checks the user-staged proto registry for the service; if nothing is registered, it falls back to gRPC server reflection. Staged protos always win over reflection.

## Quickstart: stage protos and call

### 1. Compile protos

> **Optional.** Skip this step if your server exposes gRPC reflection — `grpc_call` will resolve schemas automatically. Stage protos only when reflection is unavailable or when you want deterministic, version-pinned schemas.

Stage each `.proto` file under the filename its `import` statements reference, then compile:

```sql
SELECT grpc_proto_stage('common.proto', $PROTO$
    syntax = "proto3";
    package auth;
    message UserId { string id = 1; }
    message User {
      string id = 1;
      string email = 2;
    }
$PROTO$);

SELECT grpc_proto_stage('auth.proto', $PROTO$
    syntax = "proto3";
    import "common.proto";
    package auth;
    service AuthService {
      rpc GetUser(UserId) returns (User);
    }
$PROTO$);

SELECT grpc_proto_compile();
```

Use Postgres dollar-quoted strings (`$$...$$` or `$TAG$...$TAG$`) to paste proto source without escaping. Google Well-Known Types (`google/protobuf/timestamp.proto`, etc.) are resolved automatically.

### 2. Call the service

```sql
SELECT grpc_call(
    'localhost:50051',
    'auth.AuthService/GetUser',
    '{"id": "42"}'::jsonb
);
```

### Proto management API

| Function | Description |
|---|---|
| `grpc_proto_stage(filename TEXT, source TEXT)` | Stage a `.proto` file for the next compile. Re-staging the same filename overwrites. Source is not parsed until compile. |
| `grpc_proto_unstage(filename TEXT) → BOOLEAN` | Remove one staged file. Returns `true` if it existed. Registry of already-compiled services is untouched. |
| `grpc_proto_unstage_all()` | Clear every staged file. Registry untouched. |
| `grpc_proto_compile()` | Parse all staged files, resolve cross-imports + Google WKTs, and insert every discovered service into the registry. On success, staging is cleared. On failure (e.g. syntax error), staging is preserved so you can fix the bad file and retry. |
| `grpc_proto_unregister(service_name TEXT) → BOOLEAN` | Remove one compiled service by fully-qualified name (e.g. `"auth.AuthService"`). Returns `true` if it existed. |
| `grpc_proto_unregister_all()` | Remove every compiled service. Staging untouched. |

The staging area and registry are **per-connection** (per backend process). They reset when you reconnect.

### Recovery

If `grpc_proto_compile()` fails because one staged file is broken, the other staged files remain in place:

```sql
SELECT grpc_proto_stage('good.proto',    $PROTO$ ... $PROTO$);
SELECT grpc_proto_stage('bad.proto',     'this is not valid proto');
SELECT grpc_proto_stage('service.proto', $PROTO$ ... $PROTO$);
SELECT grpc_proto_compile();                      -- ERROR: Proto compile error: ...

-- Fix: either re-stage bad.proto with correct source, or drop it:
SELECT grpc_proto_unstage('bad.proto');
SELECT grpc_proto_compile();                      -- now succeeds
```

Recovering never touches already-compiled services, so in-flight `grpc_call`s against previously-registered services keep working throughout.

## Errors

All errors raise a PostgreSQL `ERROR` and abort the current statement:

| Prefix | Cause |
|---|---|
| `Connection error: …` | Could not reach the endpoint |
| `Proto error: …` | Reflection failed, symbol not found, or JSON ↔ protobuf encode/decode error |
| `Proto compile error: …` | `grpc_proto_compile` failed to parse/resolve the staged files |
| `gRPC call failed: …` | Server returned a non-OK gRPC status |

## Limitations

- **HTTP only** — TLS/mTLS not supported
- **Unary only** — streaming methods not supported
- **No caching** — a new connection and (for the reflection path) a new reflection request are made on every call
- **Endpoint format** — `host:port`, never include a scheme
- **Reflection** — required unless you use `grpc_proto_stage` + `grpc_proto_compile`
- **Per-connection state** — the staged/registered protos live inside a single Postgres backend; new connections start empty
