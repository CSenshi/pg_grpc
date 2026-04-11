use pgrx::prelude::*;

::pgrx::pg_module_magic!(name, version);

#[pg_extern]
fn grpc_call(
    endpoint: &str,
    method: &str,
    request: pgrx::JsonB,
    _timeout_ms: default!(Option<i64>, "null"),
) -> pgrx::JsonB {
    pgrx::JsonB(serde_json::json!({
        "endpoint": endpoint,
        "method": method,
        "request": request.0,
    }))
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_grpc_call_returns_inputs() {
        let result = crate::grpc_call(
            "localhost:50051",
            "pkg.Service/Method",
            pgrx::JsonB(serde_json::json!({"foo": "bar"})),
            None,
        );
        assert_eq!(result.0["endpoint"], "localhost:50051");
        assert_eq!(result.0["method"], "pkg.Service/Method");
        assert_eq!(result.0["request"], serde_json::json!({"foo": "bar"}));
    }
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
