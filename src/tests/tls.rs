#[pg_test]
fn test_tls_parse_empty_object_uses_system_roots() {
    let cfg = crate::tls::TlsConfig::parse(&serde_json::json!({})).unwrap();
    assert!(cfg.ca_cert.is_none());
}

#[pg_test]
fn test_tls_parse_with_ca_cert() {
    let pem = "-----BEGIN CERTIFICATE-----\nMIIB...\n-----END CERTIFICATE-----\n";
    let cfg =
        crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": pem })).unwrap();
    assert_eq!(cfg.ca_cert.as_deref(), Some(pem.as_bytes()));
}

#[pg_test]
fn test_tls_parse_rejects_unknown_key() {
    let err =
        crate::tls::TlsConfig::parse(&serde_json::json!({ "client_cert": "x" }))
            .expect_err("unknown key must fail");
    let msg = err.to_string();
    assert!(msg.contains("unknown key"), "unexpected: {msg}");
    assert!(msg.contains("ca_cert"), "should list accepted fields: {msg}");
}

#[pg_test]
fn test_tls_parse_rejects_empty_ca_cert() {
    let err = crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": "" }))
        .expect_err("empty ca_cert must fail");
    assert!(err.to_string().contains("must not be empty"), "{err}");
}

#[pg_test]
fn test_tls_parse_rejects_non_object() {
    crate::tls::TlsConfig::parse(&serde_json::json!("plain-string"))
        .expect_err("non-object must fail");
    crate::tls::TlsConfig::parse(&serde_json::json!([1, 2, 3]))
        .expect_err("array must fail");
    crate::tls::TlsConfig::parse(&serde_json::json!(42)).expect_err("number must fail");
}

#[pg_test]
fn test_tls_parse_rejects_non_string_ca_cert() {
    crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": 42 }))
        .expect_err("non-string ca_cert must fail");
}

#[pg_test]
fn test_tls_parse_null_ca_cert_treated_as_absent() {
    let cfg =
        crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": null })).unwrap();
    assert!(cfg.ca_cert.is_none());
}

#[pg_test]
fn test_tls_config_eq_same_ca_cert() {
    let a = crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": "PEM" })).unwrap();
    let b = crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": "PEM" })).unwrap();
    assert_eq!(a, b);
}

#[pg_test]
fn test_tls_config_ne_different_ca_cert() {
    let a = crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": "PEM-A" })).unwrap();
    let b = crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": "PEM-B" })).unwrap();
    assert_ne!(a, b);
}

#[pg_test]
fn test_tls_config_empty_ne_with_ca_cert() {
    let empty = crate::tls::TlsConfig::parse(&serde_json::json!({})).unwrap();
    let with_ca =
        crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": "PEM" })).unwrap();
    assert_ne!(empty, with_ca);
}
