use pgrx::prelude::*;

::pgrx::pg_module_magic!(name, version);

mod call;
mod error;
mod proto;
mod registry;

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

/// Compiles raw `.proto` source and registers every service it defines so
/// subsequent `grpc_call` invocations can target servers without reflection.
/// Re-registering a service overwrites the previous descriptor.
#[pg_extern]
fn grpc_register_proto(proto_source: &str) {
    match proto::compile_proto_source(proto_source) {
        Ok(pool) => {
            for svc in pool.services() {
                registry::insert_proto(svc.full_name(), pool.clone());
            }
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
    fn test_grpc_register_proto_then_call() {
        // Register a minimal subset of grpcbin.GRPCBin/DummyUnary so the call
        // path uses the registry instead of reflection.
        crate::grpc_register_proto(
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

        let result = crate::grpc_call(
            "grpcb.in:9000",
            "grpcbin.GRPCBin/DummyUnary",
            pgrx::JsonB(serde_json::json!({"f_string": "via-registry"})),
            None,
        );
        assert_eq!(result.0["f_string"], "via-registry");
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
