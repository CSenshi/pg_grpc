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
