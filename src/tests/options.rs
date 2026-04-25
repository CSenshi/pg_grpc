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

#[pg_test]
fn test_options_parse_use_reflection_propagates() {
    let cfg =
        crate::options::OptionsConfig::parse(&serde_json::json!({"use_reflection": true}))
            .unwrap();
    assert_eq!(cfg.use_reflection, Some(true));
    let cfg =
        crate::options::OptionsConfig::parse(&serde_json::json!({"use_reflection": false}))
            .unwrap();
    assert_eq!(cfg.use_reflection, Some(false));
}

#[pg_test]
fn test_options_parse_use_reflection_string_rejected() {
    let err = crate::options::OptionsConfig::parse(
        &serde_json::json!({"use_reflection": "yes"}),
    )
    .expect_err("string must fail");
    let msg = err.to_string();
    assert!(msg.contains("options.use_reflection"), "{msg}");
    assert!(msg.contains("boolean"), "{msg}");
}

#[pg_test]
fn test_options_parse_tls_propagates() {
    let cfg =
        crate::options::OptionsConfig::parse(&serde_json::json!({"tls": {"ca_cert": "PEM"}}))
            .unwrap();
    let tls = cfg.tls.expect("tls config must be set");
    assert_eq!(tls.ca_cert.as_deref(), Some("PEM".as_bytes()));
}

#[pg_test]
fn test_options_parse_tls_null_treated_as_absent() {
    let cfg = crate::options::OptionsConfig::parse(&serde_json::json!({"tls": null})).unwrap();
    assert!(cfg.tls.is_none());
}

#[pg_test]
fn test_options_parse_tls_array_rejected() {
    let err = crate::options::OptionsConfig::parse(&serde_json::json!({"tls": [1, 2]}))
        .expect_err("array must fail");
    let msg = err.to_string();
    assert!(msg.contains("options.tls"), "{msg}");
    assert!(msg.contains("object"), "{msg}");
}

#[pg_test]
fn test_options_parse_tls_inner_error_surfaces() {
    // Inner TlsConfig::parse rejects unknown keys; the outer parser must let
    // that error through so users see a tls-prefixed diagnostic.
    let err = crate::options::OptionsConfig::parse(
        &serde_json::json!({"tls": {"not_a_field": "x"}}),
    )
    .expect_err("inner tls error must surface");
    let msg = err.to_string();
    assert!(msg.contains("tls"), "{msg}");
    assert!(msg.contains("unknown key"), "{msg}");
}
