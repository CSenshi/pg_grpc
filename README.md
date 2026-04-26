<div align="center">

# pg_grpc

**Make gRPC calls directly from PostgreSQL**

[![Test](https://github.com/CSenshi/pg_grpc/actions/workflows/test.yml/badge.svg)](https://github.com/CSenshi/pg_grpc/actions/workflows/test.yml)
[![Release](https://img.shields.io/github/v/release/CSenshi/pg_grpc?logo=github)](https://github.com/CSenshi/pg_grpc/releases/latest)
[![License](https://img.shields.io/github/license/CSenshi/pg_grpc)](LICENSE)
[![Postgres](https://img.shields.io/badge/postgres-13%E2%80%9318-316192?logo=postgresql&logoColor=white)](#)
[![Rust](https://img.shields.io/badge/rust-2024-dea584?logo=rust&logoColor=white)](#)
[![Docs](https://img.shields.io/badge/docs-online-3578e5?logo=docusaurus&logoColor=white)](https://csenshi.github.io/pg_grpc)

[Documentation](https://csenshi.github.io/pg_grpc) · [Quickstart](https://csenshi.github.io/pg_grpc/quickstart) · [Reference](https://csenshi.github.io/pg_grpc/reference)

![pg_grpc demo](website/static/img/demo.gif)

</div>

`pg_grpc` turns any gRPC service into a first-class SQL function call. No codegen, no middleware, no app-layer glue. Invoke RPCs from triggers, materialized views, scheduled jobs, or ad-hoc queries.

## 30-second quickstart

```bash
# Install (see https://csenshi.github.io/pg_grpc/installation for all paths)
cargo pgrx install --release --no-default-features --features pg18
```

```sql
CREATE EXTENSION pg_grpc;

SELECT grpc_call(
    'grpcb.in:9001',
    'grpcbin.GRPCBin/DummyUnary',
    '{"f_string": "hello"}'::jsonb,
    options => '{"tls": {}}'::jsonb
);
```

## Documentation

📚 **[csenshi.github.io/pg_grpc](https://csenshi.github.io/pg_grpc)** - installation, TLS, user-supplied protos, recipes, full reference.

## License

[MIT](LICENSE)
