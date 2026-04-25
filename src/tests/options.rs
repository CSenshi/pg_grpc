#[pg_test]
fn test_options_parse_null_yields_all_none() {
    let cfg = crate::options::OptionsConfig::parse(&serde_json::Value::Null).unwrap();
    assert!(cfg.timeout_ms.is_none());
    assert!(cfg.use_reflection.is_none());
    assert!(cfg.tls.is_none());
    assert!(cfg.max_decode_message_size_bytes.is_none());
    assert!(cfg.max_encode_message_size_bytes.is_none());
}

#[pg_test]
fn test_options_parse_empty_object_yields_all_none() {
    let cfg = crate::options::OptionsConfig::parse(&serde_json::json!({})).unwrap();
    assert!(cfg.timeout_ms.is_none());
    assert!(cfg.use_reflection.is_none());
    assert!(cfg.tls.is_none());
    assert!(cfg.max_decode_message_size_bytes.is_none());
    assert!(cfg.max_encode_message_size_bytes.is_none());
}

#[pg_test]
fn test_options_parse_timeout_ms_propagates() {
    let cfg = crate::options::OptionsConfig::parse(&serde_json::json!({"timeout_ms": 5000}))
        .unwrap();
    assert_eq!(cfg.timeout_ms, Some(5000));
}

#[pg_test]
fn test_options_parse_timeout_ms_zero_rejected() {
    let err = crate::options::OptionsConfig::parse(&serde_json::json!({"timeout_ms": 0}))
        .expect_err("zero must fail");
    let msg = err.to_string();
    assert!(msg.contains("options.timeout_ms"), "{msg}");
    assert!(msg.contains(">= 1"), "{msg}");
}

#[pg_test]
fn test_options_parse_timeout_ms_negative_rejected() {
    let err = crate::options::OptionsConfig::parse(&serde_json::json!({"timeout_ms": -1}))
        .expect_err("negative must fail");
    let msg = err.to_string();
    assert!(msg.contains("options.timeout_ms"), "{msg}");
    assert!(msg.contains(">= 1"), "{msg}");
}

#[pg_test]
fn test_options_parse_timeout_ms_string_rejected() {
    let err = crate::options::OptionsConfig::parse(&serde_json::json!({"timeout_ms": "fast"}))
        .expect_err("string must fail");
    let msg = err.to_string();
    assert!(msg.contains("options.timeout_ms"), "{msg}");
    assert!(msg.contains("integer"), "{msg}");
}

#[pg_test]
fn test_options_parse_timeout_ms_float_rejected() {
    let err = crate::options::OptionsConfig::parse(&serde_json::json!({"timeout_ms": 1.5}))
        .expect_err("float must fail");
    let msg = err.to_string();
    assert!(msg.contains("options.timeout_ms"), "{msg}");
    assert!(msg.contains("integer"), "{msg}");
}
