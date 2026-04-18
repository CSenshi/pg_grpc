# pg_grpc

[![Test](https://github.com/CSenshi/pg_grpc/actions/workflows/test.yml/badge.svg)](https://github.com/CSenshi/pg_grpc/actions/workflows/test.yml)

Make gRPC calls directly from PostgreSQL SQL.


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

Both return:

```
       grpc_call
-----------------------
 {"f_string": "hello"}
(1 row)
```

### `grpc_call` signature

```sql
grpc_call(
    endpoint       TEXT,
    method         TEXT,
    request        JSONB,
    metadata       JSONB   DEFAULT NULL,  -- optional gRPC metadata / headers
    timeout_ms     BIGINT  DEFAULT NULL,  -- defaults to 30_000ms; must be > 0
    use_reflection BOOLEAN DEFAULT TRUE
) RETURNS JSONB
```

Returns the response as `JSONB` with proto field names (snake_case).

## Quickstart: stage protos and call

### 1. Compile protos

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


> **Optional.** Skip this step if your server exposes gRPC reflection — `grpc_call` will resolve schemas automatically. 

### 2. Call the service

```sql
SELECT grpc_call(
    'localhost:50051',
    'auth.AuthService/GetUser',
    '{"id": "42"}'::jsonb
);
```

## Proto management API

| Function                              | Description                                |
| ------------------------------------- | ------------------------------------------ |
| `grpc_proto_stage(filename, source)`  | Stage a `.proto` file for the next compile |
| `grpc_proto_unstage(filename)`        | Remove one staged file                     |
| `grpc_proto_unstage_all()`            | Clear all staged files                     |
| `grpc_proto_compile()`                | Parse + compile staged files               |
| `grpc_proto_unregister(service_name)` | Remove one compiled service                |
| `grpc_proto_unregister_all()`         | Remove all compiled services               |
| `grpc_proto_list_staged()`            | List all staged `.proto`                   |
| `grpc_proto_list_registered()`        | List all registered services               |


## Errors

All errors raise a PostgreSQL `ERROR` and abort the current statement:

| Prefix                   | Cause                                                                       |
| ------------------------ | --------------------------------------------------------------------------- |
| `Connection error: …`    | Could not reach the endpoint                                                |
| `Proto error: …`         | Reflection failed, symbol not found, or JSON ↔ protobuf encode/decode error |
| `Proto compile error: …` | `grpc_proto_compile` failed to parse/resolve the staged files               |
| `gRPC call failed: …`    | Server returned a non-OK gRPC status                                        |
| `Request timeout: …ms`   | The call (connect + reflection + unary) did not finish within `timeout_ms`  |

## Limitations

- **HTTP only** — TLS/mTLS not supported
- **Unary only** — streaming methods not supported
- **No caching** — a new connection and (for the reflection path) a new reflection request are made on every call
- **Endpoint format** — `host:port`, never include a scheme
- **Reflection** — required unless you use `grpc_proto_stage` + `grpc_proto_compile`
- **Per-connection state** — the staged/registered protos live inside a single Postgres backend; new connections start empty
