use std::collections::HashMap;

fn one_file(name: &str, source: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(name.to_string(), source.to_string());
    m
}

#[pg_test]
fn test_compile_empty_input_errors() {
    let err = crate::proto::compile_proto_files(HashMap::new())
        .expect_err("empty input should error");
    assert!(
        matches!(&err, crate::error::GrpcError::ProtoCompile(m) if m.contains("no proto files")),
        "unexpected error: {err}"
    );
}

#[pg_test]
fn test_compile_no_services_errors() {
    let files = one_file(
        "msg_only.proto",
        r#"
        syntax = "proto3";
        package test_no_svc;
        message Foo { string x = 1; }
        "#,
    );
    let err =
        crate::proto::compile_proto_files(files).expect_err("no-services file should error");
    assert!(
        matches!(&err, crate::error::GrpcError::ProtoCompile(m) if m.contains("defines no services")),
        "unexpected error: {err}"
    );
}

#[pg_test]
fn test_compile_missing_import_errors() {
    let files = one_file(
        "a.proto",
        r#"
        syntax = "proto3";
        import "definitely_missing.proto";
        package test_miss;
        service S { rpc M(Msg) returns (Msg); }
        message Msg { string x = 1; }
        "#,
    );
    let err = crate::proto::compile_proto_files(files)
        .expect_err("missing import should error");
    assert!(
        matches!(err, crate::error::GrpcError::ProtoCompile(_)),
        "expected ProtoCompile error"
    );
}

#[pg_test]
fn test_compile_syntax_error() {
    let files = one_file("bad.proto", "this is not valid proto");
    let err =
        crate::proto::compile_proto_files(files).expect_err("syntax error should fail compile");
    assert!(matches!(err, crate::error::GrpcError::ProtoCompile(_)));
}

#[pg_test]
fn test_compile_google_wkt_import() {
    // Exercises the GoogleFileResolver branch: WKT imports resolve without staging.
    let files = one_file(
        "evented.proto",
        r#"
        syntax = "proto3";
        import "google/protobuf/timestamp.proto";
        package test_wkt;
        service Event { rpc Emit(EventMsg) returns (EventMsg); }
        message EventMsg { google.protobuf.Timestamp at = 1; }
        "#,
    );
    let pool = crate::proto::compile_proto_files(files).expect("WKT import should compile");
    assert!(
        pool.get_service_by_name("test_wkt.Event").is_some(),
        "service test_wkt.Event should be in the compiled pool"
    );
}
