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
