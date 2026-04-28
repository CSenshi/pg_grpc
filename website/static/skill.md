---
name: pg_grpc
description: Make gRPC calls from PostgreSQL SQL queries using the pg_grpc extension
---

# pg_grpc

pg_grpc is a PostgreSQL extension (Postgres 13–18) that lets you invoke unary gRPC methods directly from a SQL query — no sidecar, no codegen, no application layer.

## When to use

- Call a gRPC service from inside a SQL query, trigger, or function
- Join gRPC service responses with database rows in one query
- Invoke an external service within a Postgres transaction
- Test a gRPC endpoint interactively from psql

## Setup

```sql
CREATE EXTENSION IF NOT EXISTS pg_grpc;
```

## Core function

```sql
grpc_call(
  endpoint TEXT,                  -- host:port — no http:// prefix
  method   TEXT,                  -- fully-qualified: pkg.Service/Method
  request  JSONB,                 -- request payload as JSON
  metadata JSONB DEFAULT NULL,    -- optional gRPC headers
  options  JSONB DEFAULT NULL     -- optional transport config
) RETURNS JSONB
```

## Quick patterns

**Server exposes gRPC reflection (most common):**
```sql
SELECT grpc_call(
  'localhost:50051',
  'auth.AuthService/GetUser',
  '{"id": "42"}'::jsonb
);
```

**User-supplied proto (server has no reflection):**
```sql
SELECT grpc_proto_stage('auth.proto', $$
  syntax = "proto3";
  package auth;
  service AuthService { rpc GetUser(UserId) returns (User); }
  message UserId { string id = 1; }
  message User { string id = 1; string email = 2; }
$$);
SELECT grpc_proto_compile();
SELECT grpc_call('localhost:50051', 'auth.AuthService/GetUser', '{"id": "42"}'::jsonb);
```

**With TLS (OS trust store):**
```sql
SELECT grpc_call('api.example.com:443', 'pkg.Service/Method', '{}'::jsonb,
  options => '{"tls": {}}'::jsonb);
```

**With custom timeout:**
```sql
SELECT grpc_call('host:port', 'pkg.S/M', '{}'::jsonb,
  options => '{"timeout_ms": 5000}'::jsonb);
```

**With gRPC metadata (auth header):**
```sql
SELECT grpc_call('host:port', 'pkg.S/M', '{}'::jsonb,
  metadata => '{"authorization": "Bearer <token>"}'::jsonb);
```

## `options` keys

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `timeout_ms` | integer | `30000` | Must be >= 1 |
| `use_reflection` | boolean | `true` | Set false to force user-supplied proto |
| `tls` | object | null (plaintext) | `{}` = OS trust store; see TLS guide |
| `max_decode_message_size_bytes` | integer | 4194304 (4 MiB) | Raise for large responses |
| `max_encode_message_size_bytes` | integer | unbounded | Cap outgoing request size |

## `options.tls` fields

| Field | Notes |
|-------|-------|
| `ca_cert` | PEM root CA, added to OS trust store |
| `client_cert` | PEM client cert (mTLS) — must pair with `client_key` |
| `client_key` | PEM client key (mTLS) — must pair with `client_cert` |
| `domain_name` | SNI / cert-verification override (useful for IP endpoints) |

## Error prefixes

Every failure raises a Postgres `ERROR` with a stable prefix:

| Prefix | Meaning |
|--------|---------|
| `Connection error: ...` | Bad endpoint, TLS config, or unknown `options` key |
| `Proto error: ...` | Reflection failed or JSON ↔ proto encode/decode issue |
| `Proto compile error: ...` | Staged `.proto` syntax or import resolution failed |
| `gRPC call failed: ...` | Server returned a non-OK gRPC status |
| `Request timeout: ...ms` | Call did not complete within `timeout_ms` |

## Key constraints

- **Unary RPCs only** — streaming not yet supported
- **No scheme prefix on endpoint** — `'host:port'`, not `'grpc://host:port'`
- **Per-backend cache** — proto registry and channel cache reset on each new Postgres connection; not shared across backends
- **Proto import names must match stage keys** — `import "common.proto"` only resolves if staged under exactly `'common.proto'`
- **Alpha software** — API may change between releases

## Proto management functions

```sql
grpc_proto_stage(filename TEXT, source TEXT)           -- stage a .proto file
grpc_proto_unstage(filename TEXT) RETURNS BOOLEAN      -- remove one staged file
grpc_proto_unstage_all()                               -- clear all staged files
grpc_proto_compile()                                   -- compile staged → registry
grpc_proto_unregister(service_name TEXT) RETURNS BOOLEAN  -- remove one service
grpc_proto_unregister_all()                            -- clear entire registry
grpc_proto_list_staged() RETURNS TABLE(filename, source)
grpc_proto_list_registered() RETURNS TABLE(service_name, origin, filename, source, endpoint)
```

## Full documentation

- Index: https://csenshi.github.io/pg_grpc/llms.txt
- Full content: https://csenshi.github.io/pg_grpc/llms-full.txt
- Reference: https://csenshi.github.io/pg_grpc/reference
- TLS guide: https://csenshi.github.io/pg_grpc/guides/tls-and-mtls
- Proto guide: https://csenshi.github.io/pg_grpc/guides/user-supplied-protos
