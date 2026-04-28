---
title: Large messages
sidebar_position: 3
---

> For the complete documentation index, see [llms.txt](pathname:///pg_grpc/llms.txt)

# Large messages

gRPC ships with a hard size cap on every message in both directions. The defaults are conservative - bump them when you know your payloads exceed them.

## Defaults

| Direction        | Default            | Source         |
| ---------------- | ------------------ | -------------- |
| Decode (response) | **4 MiB**          | tonic default  |
| Encode (request)  | **unbounded**      | tonic default  |

These hold for every `grpc_call` unless you override them in `options`.

## Raising the decode cap

If a server returns more than 4 MiB in one response, the call fails with `gRPC call failed: ... decoded message length too large`. Lift the cap with `max_decode_message_size_bytes`:

```sql
SELECT grpc_call(
  'host:port',
  'pkg.Service/GetLargeThing',
  '{}'::jsonb,
  options => '{"max_decode_message_size_bytes": 67108864}'::jsonb  -- 64 MiB
);
```

The value is in raw bytes.

## Setting an encode guardrail

The default is unbounded - you can push arbitrarily large requests, limited only by available memory and the wire ceiling below. To set a guardrail (so a runaway query that builds a huge JSONB doesn't try to send a 2 GB request), use `max_encode_message_size_bytes`:

```sql
SELECT grpc_call(
  'host:port',
  'pkg.Service/UploadDocument',
  big_blob_jsonb,
  options => '{"max_encode_message_size_bytes": 16777216}'::jsonb  -- 16 MiB cap
);
```

## Wire ceiling

Both knobs accept any 32-bit unsigned integer up to **4 294 967 295** (`2^32 - 1`). That's the absolute upper bound - gRPC frames each message with a 4-byte length prefix, so a single message can never exceed it on the wire. Asking for more is rejected at parse time:

```
Connection error: options.max_decode_message_size_bytes must be in [1, 4294967295]
```

## Reflection shares the channel

The reflection fetch and the unary call run over the **same** tonic channel, so the decode cap applies to both. If your server's `FileDescriptorProto` set is large enough that reflection itself fails, raising `max_decode_message_size_bytes` lifts the cap on both the schema fetch and the response.

```sql
SELECT grpc_call(
  'host:port',
  'pkg.HugeService/SomeMethod',
  '{}'::jsonb,
  options => '{"max_decode_message_size_bytes": 33554432}'::jsonb
);
```

If you're sure the response is small but the reflection is huge, the same knob lets reflection through without changing how big a unary response you'll accept on subsequent calls - both share the limit.
