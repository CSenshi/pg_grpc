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
        timeout_ms  int       NOT NULL DEFAULT 30000,
        status      text      NOT NULL DEFAULT 'pending'
    );

    CREATE UNLOGGED TABLE grpc._call_result (
        id        bigint PRIMARY KEY,
        status    text   NOT NULL,
        response  jsonb,
        error_msg text,
        created   timestamptz DEFAULT now()
    );

    CREATE INDEX ON grpc._call_result (created);

    CREATE TYPE grpc.request_status AS ENUM ('PENDING', 'SUCCESS', 'ERROR');

    CREATE TYPE grpc.call_response_result AS (
        id       bigint,
        status   grpc.request_status,
        message  text,
        response jsonb
    );
    "#,
    name = "create_grpc_async_schema",
    bootstrap,
);
