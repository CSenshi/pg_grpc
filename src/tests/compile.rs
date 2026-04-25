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

// User staging a file at a WKT filename must win: the user's StringValue
// declares a custom field, and after compile the pool must hold *that*
// version, not the bundled wrappers.proto's StringValue.
#[pg_test]
fn user_staged_wkt_name_does_not_conflict() {
    let mut files = HashMap::new();
    files.insert(
        "google/protobuf/wrappers.proto".to_string(),
        r#"
        syntax = "proto3";
        package google.protobuf;
        message StringValue { string custom_value = 1; }
        "#
        .to_string(),
    );
    files.insert(
        "service.proto".to_string(),
        r#"
        syntax = "proto3";
        import "google/protobuf/wrappers.proto";
        package custom_wkt_test;
        service S { rpc M(Msg) returns (Msg); }
        message Msg { google.protobuf.StringValue v = 1; }
        "#
        .to_string(),
    );

    let pool = crate::proto::compile_proto_files(files)
        .expect("user wrappers.proto override should compile cleanly");

    let sv = pool
        .get_message_by_name("google.protobuf.StringValue")
        .expect("user-staged StringValue should be present");
    assert!(
        sv.get_field_by_name("custom_value").is_some(),
        "pool must hold the user's StringValue (custom_value field), not the bundled one"
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
