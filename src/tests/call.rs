#[pg_test]
fn test_grpc_call_dummyunary() {
    // Force the reflection path.
    crate::grpc_proto_unregister_all();
    let result = crate::grpc_call(
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        None,
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
        &grpcbin_endpoint(),
        "no-slash-here",
        pgrx::JsonB(serde_json::json!({})),
        None,
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
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        None,
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
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "no-reflection"})),
        None,
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
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/NoSuchMethod",
        pgrx::JsonB(serde_json::json!({"f_string": "x"})),
        None,
        None,
        None,
    );
}

#[pg_test]
fn test_grpc_call_with_metadata_smoke() {
    crate::grpc_proto_unregister_all();
    let result = crate::grpc_call(
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        Some(pgrx::JsonB(serde_json::json!({
            "authorization": "Bearer tok",
            "x-trace-id": "abc"
        }))),
        None,
        None,
    );
    assert_eq!(result.0["f_string"], "hello");
}

#[pg_test]
fn test_metadata_none() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(&mut req, None).unwrap();
    assert_eq!(req.metadata().len(), 0);
}

#[pg_test]
fn test_metadata_null_is_noop() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(&mut req, Some(serde_json::Value::Null)).unwrap();
    assert_eq!(req.metadata().len(), 0);
}

#[pg_test]
fn test_metadata_ascii_single() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(
        &mut req,
        Some(serde_json::json!({"authorization": "Bearer tok"})),
    )
    .unwrap();
    assert_eq!(
        req.metadata().get("authorization").unwrap().to_str().ok(),
        Some("Bearer tok"),
    );
}

#[pg_test]
fn test_metadata_uppercase_key_lowercased() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(&mut req, Some(serde_json::json!({"Authorization": "x"}))).unwrap();
    assert!(req.metadata().get("authorization").is_some());
    assert!(req.metadata().get("Authorization").is_some());
}

#[pg_test]
fn test_metadata_multi_value() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(&mut req, Some(serde_json::json!({"x-multi": ["a", "b"]}))).unwrap();
    let values: Vec<_> = req
        .metadata()
        .get_all("x-multi")
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    assert_eq!(values, vec!["a".to_string(), "b".to_string()]);
}

#[pg_test]
fn test_metadata_bin_key_rejected() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(&mut req, Some(serde_json::json!({"foo-bin": "x"})))
        .expect_err("bin key must error");
}

#[pg_test]
fn test_metadata_invalid_key_chars() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(&mut req, Some(serde_json::json!({"bad key": "x"})))
        .expect_err("space in key must error");
}

#[pg_test]
fn test_metadata_scalar_coercion() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(
        &mut req,
        Some(serde_json::json!({"x-num": 42, "x-bool": true})),
    )
    .unwrap();
    assert_eq!(req.metadata().get("x-num").unwrap().to_str().ok(), Some("42"));
    assert_eq!(req.metadata().get("x-bool").unwrap().to_str().ok(), Some("true"));
}

#[pg_test]
fn test_metadata_object_serialized_as_json() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(
        &mut req,
        Some(serde_json::json!({"x-user": {"id": 1, "name": "alice"}})),
    )
    .unwrap();
    assert_eq!(
        req.metadata().get("x-user").unwrap().to_str().ok(),
        Some(r#"{"id":1,"name":"alice"}"#),
    );
}

#[pg_test]
fn test_metadata_null_value_skipped() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(
        &mut req,
        Some(serde_json::json!({"x-skip": null, "x-keep": "v"})),
    )
    .unwrap();
    assert!(req.metadata().get("x-skip").is_none());
    assert_eq!(req.metadata().get("x-keep").unwrap().to_str().ok(), Some("v"));
}

#[pg_test]
fn test_metadata_mixed_array_serialized() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(
        &mut req,
        Some(serde_json::json!({"x-mix": ["a", 42, {"k": "v"}]})),
    )
    .unwrap();
    let values: Vec<_> = req
        .metadata()
        .get_all("x-mix")
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    assert_eq!(values, vec!["a", "42", r#"{"k":"v"}"#]);
}

#[pg_test]
fn test_metadata_non_object() {
    let mut req = tonic::Request::new(bytes::Bytes::new());
    crate::call::apply_metadata(&mut req, Some(serde_json::json!([1, 2, 3])))
        .expect_err("array at top must error");
}

#[pg_test(error = "Request timeout: 200ms")]
fn test_grpc_call_timeout_fires() {
    // 10.255.255.1 is in TEST-NET-1-adjacent unroutable space; the connect
    // will hang long past our 200ms budget, forcing the timeout path.
    crate::grpc_call(
        "10.255.255.1:50051",
        "x.Y/Z",
        pgrx::JsonB(serde_json::json!({})),
        None,
        Some(200),
        None,
    );
}

#[pg_test(error = "timeout_ms must be positive (got 0)")]
fn test_grpc_call_timeout_zero_rejected() {
    crate::grpc_call(
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hi"})),
        None,
        Some(0),
        None,
    );
}

#[pg_test(error = "timeout_ms must be positive (got -5)")]
fn test_grpc_call_timeout_negative_rejected() {
    crate::grpc_call(
        &grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hi"})),
        None,
        Some(-5),
        None,
    );
}
