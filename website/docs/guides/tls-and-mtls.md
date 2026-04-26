---
title: TLS & mTLS
sidebar_position: 1
---

# TLS & mTLS

Connection security lives inside the `options.tls` JSONB sub-object. Four scenarios are covered, each progressively more restrictive.

:::info[HTTP vs HTTPS - the switch is the `tls` option]

The presence of `options.tls` decides the wire protocol. There is no `http`/`https` prefix on the endpoint; the scheme is implied:

| `options.tls`                | Wire protocol                |
| ---------------------------- | ---------------------------- |
| omitted or `options` absent | **HTTP/2 plaintext** (h2c)   |
| `'{}'::jsonb` (empty object) | **HTTPS** (OS trust store)   |
| `'{...}'::jsonb` (populated) | **HTTPS** (with CA / mTLS)   |

So `'{"tls": {}}'::jsonb` is the minimum to flip a call from HTTP to HTTPS — the empty object is meaningful, not a no-op.

:::

## Plaintext (default)

Omit `options.tls` or omit `options` entirely.

```sql
SELECT grpc_call('localhost:50051', 'pkg.S/M', '{}'::jsonb);
```

The endpoint is dialed over plain HTTP/2. Use only on trusted networks (loopback, internal mesh).

## TLS with the OS trust store

Set `tls` to an empty object. pg_grpc dials with tonic's native-roots - the same CA bundle your OS uses for `curl`, `wget`, etc.

```sql
SELECT grpc_call(
  'grpcb.in:9001',
  'grpcbin.GRPCBin/DummyUnary',
  '{"f_string": "hello"}'::jsonb,
  options => '{"tls": {}}'::jsonb
);
```

Use this for any service with a publicly-trusted certificate.

## TLS with a private CA

Layer in a PEM-encoded root via `ca_cert`. The OS trust store remains in effect - your private CA is **added** to it, not substituted for it.

```sql
SELECT grpc_call(
  'internal.example.com:443',
  'pkg.Service/Method',
  '{"foo": "bar"}'::jsonb,
  options => jsonb_build_object(
    'tls', jsonb_build_object(
      'ca_cert', pg_read_file('/etc/ssl/certs/internal-root.pem')
    )
  )
);
```

`pg_read_file` requires superuser. To embed certificates without filesystem access, paste the PEM directly into the JSONB value.

## mTLS (mutual TLS)

Supply both `client_cert` and `client_key` in PEM form. They are attached as a tonic `Identity` and presented during the TLS handshake.

```sql
SELECT grpc_call(
  '10.0.0.7:8443',
  'pkg.Service/Method',
  '{"foo": "bar"}'::jsonb,
  options => jsonb_build_object(
    'tls', jsonb_build_object(
      'ca_cert',     pg_read_file('/etc/ssl/certs/internal-root.pem'),
      'client_cert', pg_read_file('/etc/ssl/certs/client.pem'),
      'client_key',  pg_read_file('/etc/ssl/private/client.key'),
      'domain_name', 'internal.example.com'
    )
  )
);
```

:::warning[Both halves required]

`client_cert` and `client_key` must be set together. Supplying one without the other is a parse error:

```
Connection error: tls: client_cert requires client_key
```

:::

## SNI / domain-name override

When dialing an IP literal or any endpoint where the certificate's CN/SAN won't match the hostname you typed, set `domain_name`. It is used as both the SNI value and the cert-verification name.

```sql
SELECT grpc_call(
  '10.0.0.7:8443',
  'pkg.Service/Method',
  '{}'::jsonb,
  options => '{"tls": {"domain_name": "internal.example.com"}}'::jsonb
);
```

Without this, you'll see TLS handshake errors complaining that `10.0.0.7` does not appear in the certificate.

## Reflection over TLS

Reflection runs over the **same channel** as the unary call. If `tls` is set, reflection is fetched over TLS automatically. There's no separate switch.

If you've also disabled reflection via `options.use_reflection = false`, you must have already staged the proto schema (see [User-supplied protos](/guides/user-supplied-protos)).

## Field reference

All fields live under `options.tls`. Strings are required to be non-empty; unknown keys raise a parse error listing the accepted set.

| Field         | Type   | Required             | Description                                                         |
| ------------- | ------ | -------------------- | ------------------------------------------------------------------- |
| `ca_cert`     | string | no                   | PEM-encoded root CA. Layered on top of the OS trust store.          |
| `client_cert` | string | with `client_key`    | PEM-encoded client certificate. Used as the mTLS identity.          |
| `client_key`  | string | with `client_cert`   | PEM-encoded client private key.                                     |
| `domain_name` | string | no                   | Override the SNI / cert-verification name. Useful for IP endpoints. |

A bare `'{"tls": {}}'::jsonb` is valid and means "negotiate TLS using the OS trust store with no extra material."
