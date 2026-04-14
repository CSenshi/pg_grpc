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
