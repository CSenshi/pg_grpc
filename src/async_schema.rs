use pgrx::extension_sql;

extension_sql!(
    r#"
    CREATE SCHEMA IF NOT EXISTS grpc;

    CREATE UNLOGGED TABLE grpc.call_queue (
        id          bigserial PRIMARY KEY,
        endpoint    text      NOT NULL,
        method      text      NOT NULL,
        request     jsonb     NOT NULL,
        metadata    jsonb,
        options     jsonb,
        timeout_ms  int       NOT NULL DEFAULT 30000
    );

    CREATE UNLOGGED TABLE grpc._call_result (
        id        bigint PRIMARY KEY,
        status    text   NOT NULL CHECK (status IN ('SUCCESS', 'ERROR')),
        response  jsonb,
        error_msg text,
        created   timestamptz DEFAULT now()
    );

    CREATE INDEX ON grpc._call_result (created);
    "#,
    name = "create_grpc_async_schema",
    bootstrap,
);
