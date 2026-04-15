#[pg_test]
fn test_grpc_proto_unregister() {
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

    crate::grpc_proto_stage("grpcbin.proto", proto);
    crate::grpc_proto_compile();
    crate::grpc_proto_unregister_all();
    assert!(
        !crate::grpc_proto_unregister("grpcbin.GRPCBin"),
        "unregister_all should have cleared the registry"
    );
}

#[pg_test]
fn test_registry_precedence_over_reflection() {
    // Renames field 1 to `renamed_field`; wire tag is unchanged so the server still round-trips.
    // If reflection had been consulted, the decoded JSON would use `f_string`.
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();
    crate::grpc_proto_stage(
        "override.proto",
        r#"
        syntax = "proto3";
        package grpcbin;
        service GRPCBin { rpc DummyUnary(DummyMessage) returns (DummyMessage); }
        message DummyMessage { string renamed_field = 1; }
        "#,
    );
    crate::grpc_proto_compile();

    let result = crate::grpc_call(
        "grpcb.in:9000",
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"renamed_field": "winner"})),
        None,
        None,
    );
    assert_eq!(result.0["renamed_field"], "winner");
    assert!(
        result.0.get("f_string").is_none(),
        "reflection-derived field name should not appear when registry hits"
    );

    crate::grpc_proto_unregister_all();
}

#[pg_test]
fn test_multi_service_proto_file() {
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();
    crate::grpc_proto_stage(
        "multi.proto",
        r#"
        syntax = "proto3";
        package multi_svc;
        service Alpha { rpc M(Msg) returns (Msg); }
        service Beta  { rpc M(Msg) returns (Msg); }
        message Msg { string x = 1; }
        "#,
    );
    crate::grpc_proto_compile();

    assert!(
        crate::grpc_proto_unregister("multi_svc.Alpha"),
        "Alpha should have been registered"
    );
    assert!(
        crate::grpc_proto_unregister("multi_svc.Beta"),
        "Beta should have been registered independently"
    );
}
