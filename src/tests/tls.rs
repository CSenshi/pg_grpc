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
fn test_tls_parse_full_config_populates_all_fields() {
    let ca_pem = "-----BEGIN CERTIFICATE-----\nCA\n-----END CERTIFICATE-----\n";
    let cert_pem = "-----BEGIN CERTIFICATE-----\nCLIENT\n-----END CERTIFICATE-----\n";
    let key_pem = "-----BEGIN PRIVATE KEY-----\nKEY\n-----END PRIVATE KEY-----\n";
    let cfg = crate::tls::TlsConfig::parse(&serde_json::json!({
        "ca_cert": ca_pem,
        "client_cert": cert_pem,
        "client_key": key_pem,
        "domain_name": "internal.example.com",
    }))
    .unwrap();
    assert_eq!(cfg.ca_cert.as_deref(), Some(ca_pem.as_bytes()));
    assert_eq!(cfg.client_cert.as_deref(), Some(cert_pem.as_bytes()));
    assert_eq!(cfg.client_key.as_deref(), Some(key_pem.as_bytes()));
    assert_eq!(cfg.domain_name.as_deref(), Some("internal.example.com"));
}

#[pg_test]
fn test_tls_parse_rejects_unknown_key() {
    let err =
        crate::tls::TlsConfig::parse(&serde_json::json!({ "not_a_field": "x" }))
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

// Cache-key distinctness at the (endpoint, Option<TlsConfig>) level. Exercises
// the Hash/Eq contract directly rather than forcing real TLS handshakes —
// different TLS configs must be different keys regardless of endpoint.
#[pg_test]
fn test_cache_key_same_endpoint_same_tls_eq() {
    let endpoint = "host:9000".to_string();
    let tls = crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": "PEM" })).unwrap();
    let k1 = (endpoint.clone(), Some(tls.clone()));
    let k2 = (endpoint, Some(tls));
    assert_eq!(k1, k2);
}

#[pg_test]
fn test_cache_key_same_endpoint_different_tls_ne() {
    let endpoint = "host:9000".to_string();
    let a = crate::tls::TlsConfig::parse(&serde_json::json!({})).unwrap();
    let b = crate::tls::TlsConfig::parse(&serde_json::json!({ "ca_cert": "PEM" })).unwrap();
    let k_plain = (endpoint.clone(), None::<crate::tls::TlsConfig>);
    let k_tls_empty = (endpoint.clone(), Some(a));
    let k_tls_ca = (endpoint, Some(b));
    assert_ne!(k_plain, k_tls_empty);
    assert_ne!(k_tls_empty, k_tls_ca);
    assert_ne!(k_plain, k_tls_ca);
}

// End-to-end: real TLS handshake + reflection + unary call against grpcb.in:9001
// using the system trust store. Matches how the existing plaintext tests hit
// grpcb.in:9000 — relies on outbound network, same as the rest of the suite.
#[pg_test]
fn test_grpc_call_tls_reflection_e2e() {
    crate::grpc_proto_unregister_all();
    crate::channel_cache::clear();
    let result = crate::grpc_call(
        &grpcbin_tls_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "tls-hello"})),
        None,
        None,
        None,
        Some(pgrx::JsonB(serde_json::json!({}))),
    );
    assert_eq!(result.0["f_string"], "tls-hello");
    crate::grpc_proto_unregister_all();
    crate::channel_cache::clear();
}
