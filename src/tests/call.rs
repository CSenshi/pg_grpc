#[pg_test]
fn test_grpc_call_dummyunary() {
    // Force the reflection path.
    crate::grpc_proto_unregister_all();
    let result = crate::grpc_call(
        "grpcb.in:9000",
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        None,
        None,
    );
    assert_eq!(result.0["f_string"], "hello");
}

#[pg_test(
    error = "Proto error: invalid method path (expected 'Service/Method'): no-slash-here"
)]
fn test_grpc_call_invalid_method_path() {
    crate::grpc_call(
        "grpcb.in:9000",
        "no-slash-here",
        pgrx::JsonB(serde_json::json!({})),
        None,
        None,
    );
}

#[pg_test(
    error = "Proto error: service not found in registry and reflection disabled: grpcbin.GRPCBin"
)]
fn test_grpc_call_reflection_disabled_misses_registry() {
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();
    crate::grpc_call(
        "grpcb.in:9000",
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        None,
        Some(false),
    );
}

#[pg_test]
fn test_grpc_call_reflection_disabled_hits_registry() {
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();
    crate::grpc_proto_stage(
        "dummy.proto",
        r#"
        syntax = "proto3";
        package grpcbin;
        service GRPCBin { rpc DummyUnary(DummyMessage) returns (DummyMessage); }
        message DummyMessage { string f_string = 1; }
        "#,
    );
    crate::grpc_proto_compile();

    let result = crate::grpc_call(
        "grpcb.in:9000",
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "no-reflection"})),
        None,
        Some(false),
    );
    assert_eq!(result.0["f_string"], "no-reflection");

    crate::grpc_proto_unregister_all();
}

#[pg_test(error = "Proto error: method not found: NoSuchMethod")]
fn test_grpc_call_method_not_found() {
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();
    crate::grpc_proto_stage(
        "dummy.proto",
        r#"
        syntax = "proto3";
        package grpcbin;
        service GRPCBin { rpc DummyUnary(DummyMessage) returns (DummyMessage); }
        message DummyMessage { string f_string = 1; }
        "#,
    );
    crate::grpc_proto_compile();
    crate::grpc_call(
        "grpcb.in:9000",
        "grpcbin.GRPCBin/NoSuchMethod",
        pgrx::JsonB(serde_json::json!({"f_string": "x"})),
        None,
        None,
    );
}
