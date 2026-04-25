<div align="center">

# pg_grpc

**Make gRPC calls directly from PostgreSQL SQL.**

[![Test](https://github.com/CSenshi/pg_grpc/actions/workflows/test.yml/badge.svg)](https://github.com/CSenshi/pg_grpc/actions/workflows/test.yml)
[![Release](https://img.shields.io/github/v/release/CSenshi/pg_grpc?logo=github)](https://github.com/CSenshi/pg_grpc/releases/latest)
[![License](https://img.shields.io/github/license/CSenshi/pg_grpc)](LICENSE)
[![Postgres](https://img.shields.io/badge/postgres-13%E2%80%9318-316192?logo=postgresql&logoColor=white)](#)
[![Rust](https://img.shields.io/badge/rust-2024-dea584?logo=rust&logoColor=white)](#)

![pg_grpc demo](docs/demo.gif)

</div>


`pg_grpc` turns any gRPC service into a first-class SQL function call. Invoke RPCs from triggers, materialized views, scheduled jobs or ad-hoc queries - no codegen, no middleware, no app-layer glue.

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

### 3. TLS (optional)

Pass a non-null `tls` JSONB to negotiate TLS. The OS trust store is used by default, so the public example below needs no extra configuration:

```sql
SELECT grpc_call(
    'grpcb.in:9001',
    'grpcbin.GRPCBin/DummyUnary',
    '{"f_string": "hello"}'::jsonb,
    tls => '{}'::jsonb
);
```

For private CAs, layer in a PEM via `ca_cert`:

```sql
SELECT grpc_call(
    'internal.example.com:443',
    'pkg.Service/Method',
    '{"foo": "bar"}'::jsonb,
    tls => jsonb_build_object('ca_cert', pg_read_file('/etc/ssl/certs/internal-root.pem'))
);
```

For mTLS, supply both `client_cert` and `client_key` PEMs (set together or not at
all — one without the other is a parse error). `domain_name` overrides the SNI /
certificate-verification name, useful for IP endpoints or mismatched cert CN/SAN:

```sql
SELECT grpc_call(
    '10.0.0.7:8443',
    'pkg.Service/Method',
    '{"foo": "bar"}'::jsonb,
    tls => jsonb_build_object(
        'ca_cert',     pg_read_file('/etc/ssl/certs/internal-root.pem'),
        'client_cert', pg_read_file('/etc/ssl/certs/client.pem'),
        'client_key',  pg_read_file('/etc/ssl/private/client.key'),
        'domain_name', 'internal.example.com'
    )
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

- **Unary only** — streaming methods not supported
- **Endpoint format** — `host:port`, never include a scheme (the scheme is chosen by the `tls` parameter)
- **Reflection** — required unless you use `grpc_proto_stage` + `grpc_proto_compile`
- **Per-connection state** — channel cache, staged, and registered protos all live inside a single Postgres backend; new connections start empty
