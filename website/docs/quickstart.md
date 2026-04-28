---
title: Quickstart
sidebar_position: 3
---

> For the complete documentation index, see [llms.txt](pathname:///pg_grpc/llms.txt)

# Quickstart

Two paths, depending on whether your gRPC server exposes [reflection](https://grpc.io/docs/guides/reflection/).

This page assumes pg_grpc is already installed (see [Installation](/installation)) and you're connected to a Postgres backend with the extension enabled:

```sql
CREATE EXTENSION IF NOT EXISTS pg_grpc;
```

## Path 1 - Server exposes reflection

One SQL call. Schemas are fetched from the server on first use, then cached so subsequent calls to the same service skip reflection.

```sql
SELECT grpc_call(
  'localhost:50051',
  'auth.AuthService/GetUser',
  '{"id": "42"}'::jsonb
);
```

## Path 2 - Server does not expose reflection

Stage your `.proto` files, compile them, then call:

```sql
-- 1. Stage one or more .proto files
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

-- 2. Compile every staged file at once. Failure preserves staging.
SELECT grpc_proto_compile();

-- 3. Call as if reflection had been used.
SELECT grpc_call(
  'localhost:50051',
  'auth.AuthService/GetUser',
  '{"id": "42"}'::jsonb
);
```

See [User-supplied protos](/guides/user-supplied-protos) for multi-file imports, well-known types and the full lifecycle.

## One important caveat

All caches (channel pool, staged files, registered services) live **inside a single Postgres backend process**. They do not persist across reconnects and they are not shared between concurrent backends. (Tracking this issue here: [#14: Persist staged and compiled protos across restarts](https://github.com/CSenshi/pg_grpc/issues/14))

In practice this means:

- A new `psql` session starts with an empty channel cache and an empty proto registry.
- After connection-pooler churn (PgBouncer in transaction mode, etc.), your first call on a new backend pays the reflection cost again.
- To force a refresh inside the current session, call [`grpc_proto_unregister(...)`](/reference#proto-management) or [`grpc_proto_unregister_all()`](/reference#proto-management).
