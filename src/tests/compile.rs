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

// Regression for #31: encoding a JSON Any payload whose @type points at a WKT
// (StringValue, lives in google/protobuf/wrappers.proto) must succeed even
// though the user proto only imports google/protobuf/any.proto. Without the
// WKT auto-seed in compile_proto_files, MessageDescriptor::deserialize errors
// with "message 'google.protobuf.StringValue' not found".
#[pg_test]
fn wkt_any_payload_encodes() {
    use prost_reflect::DynamicMessage;
    use serde::de::DeserializeSeed as _;

    let files = one_file(
        "any_test.proto",
        r#"
        syntax = "proto3";
        import "google/protobuf/any.proto";
        package wkt_any_test;
        service S { rpc M(Msg) returns (Msg); }
        message Msg { google.protobuf.Any payload = 1; }
        "#,
    );
    let pool = crate::proto::compile_proto_files(files).expect("compile must succeed");

    let desc = pool
        .get_message_by_name("wkt_any_test.Msg")
        .expect("wkt_any_test.Msg should be in pool");
    let json = r#"{"payload":{"@type":"type.googleapis.com/google.protobuf.StringValue","value":"hi"}}"#;
    let mut de = serde_json::Deserializer::from_str(json);
    let _msg: DynamicMessage = desc
        .deserialize(&mut de)
        .expect("Any payload referencing WKT should deserialize against compiled pool");
}
