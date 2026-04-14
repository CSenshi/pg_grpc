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
