#[pg_test]
fn test_grpc_call_dummyunary() {
    // Clean registry so this test actually exercises the reflection path.
    crate::grpc_proto_unregister_all();
    let result = crate::grpc_call(
        "grpcb.in:9000",
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        None,
    );
    assert_eq!(result.0["f_string"], "hello");
}

#[pg_test(
    error = "Proto error: invalid method path (expected 'Service/Method'): no-slash-here"
)]
fn test_grpc_call_invalid_method_path() {
    // No slash → parse_method fails before any network I/O.
    crate::grpc_call(
        "grpcb.in:9000",
        "no-slash-here",
        pgrx::JsonB(serde_json::json!({})),
        None,
    );
}

#[pg_test(error = "Proto error: method not found: NoSuchMethod")]
fn test_grpc_call_method_not_found() {
    // Stage a service with exactly one method, then ask for a different
    // method name. resolve_method should surface a `method not found` error
    // without reaching the server's dispatch.
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
    );
}
