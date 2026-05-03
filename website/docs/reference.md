---
title: Reference
sidebar_position: 5
toc_min_heading_level: 2
toc_max_heading_level: 4
---

> For the complete documentation index, see [llms.txt](pathname:///pg_grpc/llms.txt)

# Reference

Every SQL function `pg_grpc` exposes, with signatures and defaults. For prose-style explanation, see [Guides](/guides/tls-and-mtls).

## `grpc_call`

```sql
grpc_call(
  endpoint  TEXT,
  method    TEXT,
  request   JSONB,
  metadata  JSONB DEFAULT NULL,
  options   JSONB DEFAULT NULL
) RETURNS JSONB
```

| Parameter  | Required | Description                                                                                  |
| ---------- | -------- | -------------------------------------------------------------------------------------------- |
| `endpoint` | yes      | `host:port`. Never include a scheme - the scheme is chosen by `options.tls`.                 |
| `method`   | yes      | Fully-qualified `pkg.Service/Method`.                                                        |
| `request`  | yes      | JSON request body. Encoded against the input message descriptor.                             |
| `metadata` | no       | gRPC headers (see below). `NULL` is no headers.                                              |
| `options`  | no       | Per-call transport / runtime config (see below). `NULL` takes all defaults.                  |

Returns the decoded response message as JSONB. Any failure raises a Postgres `ERROR` and aborts the statement.

### `options` blob

Strict-parsed JSONB object. Unknown keys raise `Connection error` listing the accepted set.

| Key                              | Type    | Validation       | Default                       |
| -------------------------------- | ------- | ---------------- | ----------------------------- |
| `timeout_ms`                     | integer | `>= 1`           | `30000`                       |
| `use_reflection`                 | boolean | -                | `true`                        |
| `tls`                            | object  | see below        | `NULL` → plaintext            |
| `max_decode_message_size_bytes`  | integer | `[1, 4294967295]`| tonic default (4 MiB)         |
| `max_encode_message_size_bytes`  | integer | `[1, 4294967295]`| tonic default (unbounded)     |

`options.tls` (when set) accepts:

| Field         | Type   | Notes                                                                                |
| ------------- | ------ | ------------------------------------------------------------------------------------ |
| `ca_cert`     | string | PEM. Layered onto the OS trust store.                                                |
| `client_cert` | string | PEM. Required together with `client_key`.                                            |
| `client_key`  | string | PEM. Required together with `client_cert`.                                           |
| `domain_name` | string | SNI / cert-verification override.                                                    |

A bare `'{"tls": {}}'` enables TLS using the OS trust store.

### `metadata` blob

JSONB object. Keys are silently lowercased. Values may be a string or an array of strings (repeated headers).

```sql
SELECT grpc_call(
  'host:port', 'pkg.S/M', '{}'::jsonb,
  metadata => '{"authorization": "Bearer abc", "x-trace-id": ["t1", "t2"]}'::jsonb
);
```

Keys ending in `-bin` (binary metadata) are rejected in v1.

## `grpc_call_async`

```sql
grpc_call_async(
  endpoint  TEXT,
  method    TEXT,
  request   JSONB,
  metadata  JSONB DEFAULT NULL,
  options   JSONB DEFAULT NULL
) RETURNS BIGINT
```

Enqueues a gRPC call for the background worker and returns the call `id` immediately. The calling transaction does not block on the network. Options are validated at enqueue time using the same rules as `grpc_call`.

See [Async calls](/guides/async-calls) for setup, fan-out patterns and configuration.

## `grpc_call_result`

```sql
grpc_call_result(
  id     BIGINT,
  async  BOOLEAN DEFAULT TRUE
) RETURNS TABLE (
  id       BIGINT,
  status   TEXT,
  message  TEXT,
  response JSONB
)
```

Fetches the result for a previously enqueued call.

| Column     | Notes                                                                 |
| ---------- | --------------------------------------------------------------------- |
| `id`       | The call id.                                                          |
| `status`   | `PENDING` (not yet finished), `SUCCESS`, or `ERROR`.                  |
| `message`  | Error string on `ERROR`. `NULL` otherwise.                            |
| `response` | Decoded response JSONB on `SUCCESS`. `NULL` otherwise.                |

When `async = false` the function polls at 50 ms intervals until the result is no longer `PENDING`.  When `async = true` (the default) it returns immediately with whatever status exists at call time.

## Async GUCs

Configured in `postgresql.conf`. Identity GUCs require a server restart; operational GUCs take effect on `SIGHUP` / `SELECT pg_reload_conf()`.

| GUC                     | Type    | Default     | Reload   | Description                                            |
| ----------------------- | ------- | ----------- | -------- | ------------------------------------------------------ |
| `pg_grpc.database_name` | string  | `postgres`  | restart  | Database the background worker connects to.            |
| `pg_grpc.username`      | string  | (superuser) | restart  | Role the worker runs as. `NULL` = bootstrap superuser. |
| `pg_grpc.batch_size`    | integer | `200`       | SIGHUP   | Max rows dequeued per worker cycle.                    |
| `pg_grpc.ttl`           | string  | `6 hours`   | SIGHUP   | How long completed results are retained before TTL cleanup. |

## Proto management

User-supplied proto compilation surface. See [User-supplied protos](/guides/user-supplied-protos) for the lifecycle and worked examples.

### `grpc_proto_stage(filename text, source text)`

Stages one `.proto` file's source under the given filename. Re-staging the same filename overwrites.

### `grpc_proto_unstage(filename text) returns boolean`

Removes one staged file. Returns `true` if it was present.

### `grpc_proto_unstage_all()`

Clears every staged file. Registry untouched.

### `grpc_proto_compile()`

Compiles every staged file together. On success, every service is inserted into the registry and staging is cleared. On failure, both areas are left as they were.

### `grpc_proto_unregister(service_name text) returns boolean`

Removes one fully-qualified service from the registry. Returns `true` if it was present.

### `grpc_proto_unregister_all()`

Drops every registered service. Staging untouched.

### `grpc_proto_list_staged() returns table(filename text, source text)`

One row per currently-staged file.

### `grpc_proto_list_registered() returns table(service_name text origin text, filename text, source text, endpoint text)`

One row per registered service. `origin` is `'user'` (registered via stage+compile) or `'reflection'` (auto-registered on a `grpc_call` cache miss).

## Errors

Every failure raises a Postgres `ERROR` with a stable prefix. Match on the prefix in caller code.

| Prefix                   | Cause                                                                       |
| ------------------------ | --------------------------------------------------------------------------- |
| `Connection error: …`    | Could not reach the endpoint or invalid `options` / `tls` config.          |
| `Proto error: …`         | Reflection failed, symbol not found or JSON ↔ protobuf encode/decode error.|
| `Proto compile error: …` | `grpc_proto_compile` failed to parse or resolve staged files.               |
| `gRPC call failed: …`    | Server returned a non-OK gRPC status.                                       |
| `Request timeout: …ms`   | The call (connect + reflection + unary) did not finish within `timeout_ms`. |
