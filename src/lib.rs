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
/// SELECT grpc_proto_stage('common.proto', $$ syntax = "proto3"; ... $$);
/// SELECT grpc_proto_stage('service.proto', $$ import "common.proto"; ... $$);
/// SELECT grpc_proto_compile();
/// ```
#[pg_extern]
fn grpc_proto_stage(filename: &str, source: &str) {
    proto_staging::stage_file(filename, source);
}

/// Removes one file from the staging area by filename. Returns `true` if an
/// entry was removed, `false` if no matching file was staged. The registry of
/// already-compiled services is untouched — use this to recover from a bad
/// staged file without disturbing in-flight `grpc_call`s.
#[pg_extern]
fn grpc_proto_unstage(filename: &str) -> bool {
    proto_staging::remove(filename)
}

/// Clears every file from the staging area. The registry of already-compiled
/// services is untouched, so in-flight `grpc_call`s are unaffected.
#[pg_extern]
fn grpc_proto_unstage_all() {
    proto_staging::clear();
}

/// Compiles every file previously staged via `grpc_proto_stage`, resolving
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

/// Removes a single registered service by its fully-qualified name (e.g.
/// `"pkg.Service"`). Returns `true` if an entry was removed, `false` if no
/// matching service was registered. Does not touch the staging area.
#[pg_extern]
fn grpc_proto_unregister(service_name: &str) -> bool {
    proto_registry::remove(service_name)
}

/// Clears every registered service. Does not touch the staging area.
#[pg_extern]
fn grpc_proto_unregister_all() {
    proto_registry::clear();
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
    fn test_grpc_proto_stage_single_file() {
        // Stage one self-contained proto, compile, then call via the registry
        // path (reflection is bypassed).
        crate::grpc_proto_stage(
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
    fn test_grpc_proto_stage_cross_import() {
        // Two files: common.proto defines the message, service.proto imports
        // it and declares the service. Exercises the cross-file resolver path.
        crate::grpc_proto_stage(
            "common.proto",
            r#"
            syntax = "proto3";
            package grpcbin;
            message DummyMessage { string f_string = 1; }
            "#,
        );
        crate::grpc_proto_stage(
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

    #[pg_test]
    fn test_grpc_proto_unstage_recovers_bad_file() {
        // Scenario: user stages a good file, a bad file, another good file,
        // then tries to compile. Compile fails. User unstages the bad file
        // and re-compiles without having to re-stage the good files — and
        // without touching the registry.
        crate::grpc_proto_stage(
            "good.proto",
            r#"
            syntax = "proto3";
            package grpcbin;
            message DummyMessage { string f_string = 1; }
            "#,
        );
        crate::grpc_proto_stage("bad.proto", "this is not valid proto");
        crate::grpc_proto_stage(
            "service.proto",
            r#"
            syntax = "proto3";
            import "good.proto";
            package grpcbin;
            service GRPCBin {
              rpc DummyUnary(DummyMessage) returns (DummyMessage);
            }
            "#,
        );

        // unstage the bad file; good and service remain staged
        assert!(crate::grpc_proto_unstage("bad.proto"));
        assert!(!crate::grpc_proto_unstage("bad.proto")); // already gone

        // compile should now succeed with just the good files
        crate::grpc_proto_compile();

        let result = crate::grpc_call(
            "grpcb.in:9000",
            "grpcbin.GRPCBin/DummyUnary",
            pgrx::JsonB(serde_json::json!({"f_string": "recovered"})),
            None,
        );
        assert_eq!(result.0["f_string"], "recovered");

        // leave a clean state so other tests aren't affected
        crate::grpc_proto_unregister_all();
    }

    #[pg_test]
    fn test_grpc_proto_unregister() {
        // Register a service, confirm unregister removes it, then re-register
        // and use unregister_all to wipe everything.
        let proto = r#"
            syntax = "proto3";
            package grpcbin;
            service GRPCBin {
              rpc DummyUnary(DummyMessage) returns (DummyMessage);
            }
            message DummyMessage { string f_string = 1; }
        "#;

        crate::grpc_proto_stage("grpcbin.proto", proto);
        crate::grpc_proto_compile();
        assert!(
            crate::grpc_proto_unregister("grpcbin.GRPCBin"),
            "unregister should return true for an existing service"
        );
        assert!(
            !crate::grpc_proto_unregister("grpcbin.GRPCBin"),
            "second unregister should return false"
        );

        // Re-register, then wipe via unregister_all.
        crate::grpc_proto_stage("grpcbin.proto", proto);
        crate::grpc_proto_compile();
        crate::grpc_proto_unregister_all();
        assert!(
            !crate::grpc_proto_unregister("grpcbin.GRPCBin"),
            "unregister_all should have cleared the registry"
        );
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
