#[pg_test]
fn test_grpc_proto_stage_single_file() {
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
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "via-registry"})),
        None,
        None,
    );
    assert_eq!(result.0["f_string"], "via-registry");
}

#[pg_test]
fn test_grpc_proto_stage_cross_import() {
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
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "multi-file"})),
        None,
        None,
    );
    assert_eq!(result.0["f_string"], "multi-file");
}

#[pg_test]
fn test_grpc_proto_unstage_recovers_bad_file() {
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

    assert!(crate::grpc_proto_unstage("bad.proto"));
    assert!(!crate::grpc_proto_unstage("bad.proto"));

    crate::grpc_proto_compile();

    let result = crate::grpc_call(
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "recovered"})),
        None,
        None,
    );
    assert_eq!(result.0["f_string"], "recovered");

    crate::grpc_proto_unregister_all();
}

#[pg_test]
fn test_stage_overwrite_uses_latest() {
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();

    crate::grpc_proto_stage(
        "svc.proto",
        r#"
        syntax = "proto3";
        package overwrite_test;
        service First { rpc M(Msg) returns (Msg); }
        message Msg { string x = 1; }
        "#,
    );
    crate::grpc_proto_stage(
        "svc.proto",
        r#"
        syntax = "proto3";
        package overwrite_test;
        service Second { rpc M(Msg) returns (Msg); }
        message Msg { string x = 1; }
        "#,
    );
    crate::grpc_proto_compile();

    assert!(
        !crate::grpc_proto_unregister("overwrite_test.First"),
        "overwritten source should not have been compiled"
    );
    assert!(
        crate::grpc_proto_unregister("overwrite_test.Second"),
        "latest source should be the one compiled"
    );
}

#[pg_test(error = "Proto compile error: no proto files supplied")]
fn test_compile_empty_staging_errors() {
    crate::grpc_proto_unstage_all();
    crate::grpc_proto_compile();
}

#[pg_test]
fn test_unstage_nonexistent_returns_false() {
    crate::grpc_proto_unstage_all();
    assert!(
        !crate::grpc_proto_unstage("never_staged.proto"),
        "unstaging a file that was never staged should return false"
    );
}

#[pg_test]
fn test_validate_stage_input_accepts_normal() {
    crate::validate_stage_input("a.proto", r#"syntax = "proto3";"#).unwrap();
}

#[pg_test]
fn test_validate_stage_input_rejects_empty_filename() {
    crate::validate_stage_input("", "x").expect_err("empty filename must fail");
}

#[pg_test]
fn test_validate_stage_input_rejects_whitespace_filename() {
    crate::validate_stage_input("   ", "x").expect_err("whitespace filename must fail");
}

#[pg_test]
fn test_validate_stage_input_rejects_tab_newline_filename() {
    crate::validate_stage_input("\t\n", "x").expect_err("tab/newline filename must fail");
}

#[pg_test]
fn test_validate_stage_input_rejects_empty_source() {
    crate::validate_stage_input("a.proto", "").expect_err("empty source must fail");
}

#[pg_test]
fn test_validate_stage_input_rejects_whitespace_source() {
    crate::validate_stage_input("a.proto", "   ").expect_err("whitespace source must fail");
}

#[pg_test]
fn test_validate_stage_input_rejects_tab_newline_source() {
    crate::validate_stage_input("a.proto", "\t\n").expect_err("tab/newline source must fail");
}

#[pg_test(error = "Proto compile error: grpc_proto_stage: filename must not be empty")]
fn test_grpc_proto_stage_empty_filename_errors() {
    crate::grpc_proto_stage("", r#"syntax = "proto3";"#);
}

#[pg_test(error = "Proto compile error: grpc_proto_stage: filename must not be empty")]
fn test_grpc_proto_stage_whitespace_filename_errors() {
    crate::grpc_proto_stage("   ", r#"syntax = "proto3";"#);
}

#[pg_test(error = "Proto compile error: grpc_proto_stage: source must not be empty")]
fn test_grpc_proto_stage_empty_source_errors() {
    crate::grpc_proto_stage("a.proto", "");
}

#[pg_test(error = "Proto compile error: grpc_proto_stage: source must not be empty")]
fn test_grpc_proto_stage_whitespace_source_errors() {
    crate::grpc_proto_stage("a.proto", "  \t\n ");
}
