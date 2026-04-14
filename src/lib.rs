use pgrx::prelude::*;

::pgrx::pg_module_magic!(name, version);

mod call;
mod error;
mod proto;
mod proto_registry;
mod proto_staging;

#[pg_extern]
fn grpc_call(
    endpoint: &str,
    method: &str,
    request: pgrx::JsonB,
    _timeout_ms: default!(Option<i64>, "null"),
) -> pgrx::JsonB {
    match call::make_grpc_call(endpoint, method, request.0) {
        Ok(value) => pgrx::JsonB(value),
        Err(e) => pgrx::error!("{}", e),
    }
}

/// Stages a `.proto` file under `filename` for the next `grpc_proto_compile`
/// call. Nothing is parsed or validated until compile time. Re-registering
/// the same filename overwrites the previous content.
///
/// ```sql
/// SELECT grpc_proto_register('common.proto', $$ syntax = "proto3"; ... $$);
/// SELECT grpc_proto_register('service.proto', $$ import "common.proto"; ... $$);
/// SELECT grpc_proto_compile();
/// ```
#[pg_extern]
fn grpc_proto_register(filename: &str, source: &str) {
    proto_staging::stage_file(filename, source);
}

/// Compiles every file previously staged via `grpc_proto_register`, resolving
/// imports between them (and against the Google Well-Known Types), then
/// registers every service discovered so `grpc_call` can target servers
/// without reflection.
///
/// On success, the staging area is cleared. On failure, staged files are
/// left intact so the caller can fix the offending file and retry.
#[pg_extern]
fn grpc_proto_compile() {
    let staged = proto_staging::snapshot();
    match proto::compile_proto_files(staged) {
        Ok(pool) => {
            for svc in pool.services() {
                proto_registry::insert_proto(svc.full_name(), pool.clone());
            }
            proto_staging::clear();
        }
        Err(e) => pgrx::error!("{}", e),
    }
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_grpc_call_dummyunary() {
        let result = crate::grpc_call(
            "grpcb.in:9000",
            "grpcbin.GRPCBin/DummyUnary",
            pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
            None,
        );
        assert_eq!(result.0["f_string"], "hello");
    }

    #[pg_test]
    fn test_grpc_proto_register_single_file() {
        // Stage one self-contained proto, compile, then call via the registry
        // path (reflection is bypassed).
        crate::grpc_proto_register(
            "grpcbin.proto",
            r#"
            syntax = "proto3";
            package grpcbin;
            service GRPCBin {
              rpc DummyUnary(DummyMessage) returns (DummyMessage);
            }
            message DummyMessage {
              string f_string = 1;
            }
            "#,
        );
        crate::grpc_proto_compile();

        let result = crate::grpc_call(
            "grpcb.in:9000",
            "grpcbin.GRPCBin/DummyUnary",
            pgrx::JsonB(serde_json::json!({"f_string": "via-registry"})),
            None,
        );
        assert_eq!(result.0["f_string"], "via-registry");
    }

    #[pg_test]
    fn test_grpc_proto_register_cross_import() {
        // Two files: common.proto defines the message, service.proto imports
        // it and declares the service. Exercises the cross-file resolver path.
        crate::grpc_proto_register(
            "common.proto",
            r#"
            syntax = "proto3";
            package grpcbin;
            message DummyMessage { string f_string = 1; }
            "#,
        );
        crate::grpc_proto_register(
            "service.proto",
            r#"
            syntax = "proto3";
            import "common.proto";
            package grpcbin;
            service GRPCBin {
              rpc DummyUnary(DummyMessage) returns (DummyMessage);
            }
            "#,
        );
        crate::grpc_proto_compile();

        let result = crate::grpc_call(
            "grpcb.in:9000",
            "grpcbin.GRPCBin/DummyUnary",
            pgrx::JsonB(serde_json::json!({"f_string": "multi-file"})),
            None,
        );
        assert_eq!(result.0["f_string"], "multi-file");
    }
}

/// This module is required by `cargo pgrx test` invocations.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {}

    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        vec![]
    }
}
