#[pg_test]
fn test_wait_until_running_returns() {
    // If the BGW started and published RUNNING, this returns quickly.
    // If it hangs, the test runner will time out — BGW never became ready.
    crate::grpc_wait_until_running();
}

#[pg_test]
fn test_worker_restart_returns_true() {
    crate::grpc_wait_until_running();
    let ok = crate::grpc_worker_restart();
    assert!(ok);
}
