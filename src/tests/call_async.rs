#[pg_test]
fn test_call_async_enqueues_row() {
    crate::grpc_wait_until_running();
    let id = crate::grpc_call_async(
        "grpcb.in:9000",
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        None,
        None,
    );
    let status = Spi::get_one::<String>(
        &format!("SELECT status FROM grpc.call_queue WHERE id = {id}"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(status, "pending");
}

#[pg_test]
fn test_call_async_rollback_no_row() {
    crate::grpc_wait_until_running();
    // Use PostgreSQL's internal subtransaction API — SAVEPOINT/ROLLBACK TO SAVEPOINT
    // are rejected by SPI, but BeginInternalSubTransaction works directly.
    unsafe { pg_sys::BeginInternalSubTransaction(std::ptr::null()) };
    let id = crate::grpc_call_async(
        "grpcb.in:9000",
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        None,
        None,
    );
    unsafe { pg_sys::RollbackAndReleaseCurrentSubTransaction() };
    let count = Spi::get_one::<i64>(
        &format!("SELECT count(*) FROM grpc.call_queue WHERE id = {id}"),
    )
    .unwrap()
    .unwrap_or(0);
    assert_eq!(count, 0);
}

#[pg_test]
fn test_call_async_stores_metadata_and_options() {
    crate::grpc_wait_until_running();
    let id = crate::grpc_call_async(
        "grpcb.in:9000",
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        Some(pgrx::JsonB(serde_json::json!({"authorization": "Bearer token"}))),
        Some(pgrx::JsonB(serde_json::json!({"timeout_ms": 5000}))),
    );
    let metadata_is_some = Spi::get_one::<bool>(
        &format!("SELECT metadata IS NOT NULL FROM grpc.call_queue WHERE id = {id}"),
    )
    .unwrap()
    .unwrap_or(false);
    let options_is_some = Spi::get_one::<bool>(
        &format!("SELECT options IS NOT NULL FROM grpc.call_queue WHERE id = {id}"),
    )
    .unwrap()
    .unwrap_or(false);
    assert!(metadata_is_some, "metadata should be stored");
    assert!(options_is_some, "options should be stored");
}
