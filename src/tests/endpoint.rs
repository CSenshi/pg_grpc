#[pg_test]
fn test_validate_endpoint_accepts_host_port() {
    let got = crate::endpoint::validate_endpoint("localhost:50051").unwrap();
    assert_eq!(got, "localhost:50051");
}

#[pg_test]
fn test_validate_endpoint_rejects_empty() {
    let err = crate::endpoint::validate_endpoint("").expect_err("empty must fail");
    assert!(
        matches!(err, crate::error::GrpcError::Connection(_)),
        "expected Connection variant, got {err:?}"
    );
}

#[pg_test]
fn test_validate_endpoint_rejects_whitespace_only() {
    crate::endpoint::validate_endpoint("   \t\n").expect_err("whitespace-only must fail");
}

#[pg_test]
fn test_validate_endpoint_trims_surrounding_whitespace() {
    let got = crate::endpoint::validate_endpoint("  localhost:50051\n").unwrap();
    assert_eq!(got, "localhost:50051");
}

#[pg_test]
fn test_validate_endpoint_rejects_http_scheme() {
    let err =
        crate::endpoint::validate_endpoint("http://localhost:50051").expect_err("scheme must fail");
    let msg = err.to_string();
    assert!(msg.contains("scheme"), "unexpected error: {msg}");
    assert!(msg.contains("://"), "unexpected error: {msg}");
}

#[pg_test]
fn test_validate_endpoint_rejects_grpcs_scheme() {
    crate::endpoint::validate_endpoint("grpcs://host:443").expect_err("scheme must fail");
}

#[pg_test]
fn test_validate_endpoint_rejects_path() {
    let err = crate::endpoint::validate_endpoint("localhost:50051/foo")
        .expect_err("path must fail");
    let msg = err.to_string();
    assert!(msg.contains("path"), "unexpected error: {msg}");
    assert!(msg.contains('/'), "unexpected error: {msg}");
}

#[pg_test]
fn test_validate_endpoint_rejects_trailing_slash() {
    crate::endpoint::validate_endpoint("localhost:50051/").expect_err("trailing slash must fail");
}
