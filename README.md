# pg_grpc

Make gRPC calls directly from PostgreSQL SQL.

```sql
SELECT grpc_call('localhost:50051', 'package.Service/Method', '{"key": "value"}'::jsonb);
```

No `.proto` files or code generation needed — message schemas are resolved at runtime via gRPC server reflection.

## Usage

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

## Errors

All errors raise a PostgreSQL `ERROR` and abort the current statement:

| Prefix | Cause |
|---|---|
| `Connection error: …` | Could not reach the endpoint |
| `Proto error: …` | Reflection failed, symbol not found, or JSON ↔ protobuf encode/decode error |
| `gRPC call failed: …` | Server returned a non-OK gRPC status |

## Limitations

- **HTTP only** — TLS/mTLS not supported
- **Unary only** — streaming methods not supported
- **No caching** — a new connection and reflection request are made on every call
- **Endpoint format** — `host:port`, never include a scheme
- **Reflection** — server must expose `grpc.reflection.v1alpha.ServerReflection`
