#[pg_test]
fn test_worker_restart_returns_true() {
    let ok = crate::grpc_worker_restart();
    assert!(ok);
}
