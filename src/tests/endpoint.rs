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
