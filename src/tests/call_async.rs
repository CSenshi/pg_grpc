#[pg_test]
fn test_call_async_enqueues_row() {
    let id = crate::grpc_call_async(
        &crate::tests::tests::grpcbin_endpoint(),
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "hello"})),
        None,
        None,
    );
    let in_queue = Spi::get_one::<i64>(
        &format!("SELECT count(*)::bigint FROM grpc.call_queue WHERE id = {id}"),
    )
    .unwrap()
    .unwrap_or(0);
    assert_eq!(in_queue, 1);
}

#[pg_test]
fn test_call_async_rollback_no_row() {
    // Use PostgreSQL's internal subtransaction API — SAVEPOINT/ROLLBACK TO SAVEPOINT
    // are rejected by SPI, but BeginInternalSubTransaction works directly.
    unsafe { pg_sys::BeginInternalSubTransaction(std::ptr::null()) };
    let id = crate::grpc_call_async(
        &crate::tests::tests::grpcbin_endpoint(),
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
    let id = crate::grpc_call_async(
        &crate::tests::tests::grpcbin_endpoint(),
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

// --- grpc_call_result tests ---

#[pg_test]
fn test_call_result_returns_success_with_response() {
    Spi::run(
        "INSERT INTO grpc._call_result (id, status, response)
         VALUES (10001, 'SUCCESS', '{\"f_string\": \"hello\"}')",
    )
    .unwrap();

    let status = Spi::get_one::<String>("SELECT status FROM grpc_call_result(10001)")
        .unwrap()
        .unwrap();
    assert_eq!(status, "SUCCESS");

    let response = Spi::get_one::<pgrx::JsonB>("SELECT response FROM grpc_call_result(10001)")
        .unwrap()
        .unwrap();
    assert_eq!(response.0["f_string"], "hello");

    Spi::run("DELETE FROM grpc._call_result WHERE id = 10001").unwrap();
}

#[pg_test]
fn test_call_result_returns_error_with_message() {
    Spi::run(
        "INSERT INTO grpc._call_result (id, status, error_msg)
         VALUES (10002, 'ERROR', 'connection refused')",
    )
    .unwrap();

    let status = Spi::get_one::<String>("SELECT status FROM grpc_call_result(10002)")
        .unwrap()
        .unwrap();
    assert_eq!(status, "ERROR");

    let message = Spi::get_one::<String>("SELECT message FROM grpc_call_result(10002)")
        .unwrap()
        .unwrap();
    assert_eq!(message, "connection refused");

    Spi::run("DELETE FROM grpc._call_result WHERE id = 10002").unwrap();
}

#[pg_test]
fn test_call_result_returns_pending_for_queued_row() {
    Spi::run(
        "INSERT INTO grpc.call_queue (id, endpoint, method, request)
         VALUES (10003, 'host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();

    let status = Spi::get_one::<String>("SELECT status FROM grpc_call_result(10003)")
        .unwrap()
        .unwrap();
    assert_eq!(status, "PENDING");

    Spi::run("DELETE FROM grpc.call_queue WHERE id = 10003").unwrap();
}

#[pg_test]
fn test_call_result_returns_error_for_missing_id() {
    let status = Spi::get_one::<String>("SELECT status FROM grpc_call_result(99998)")
        .unwrap()
        .unwrap();
    assert_eq!(status, "ERROR");

    let message = Spi::get_one::<String>("SELECT message FROM grpc_call_result(99998)")
        .unwrap()
        .unwrap();
    assert!(message.contains("not found"), "expected 'not found' in: {message}");
}

// --- true e2e tests: grpc_call_async → call_async_row → grpc_call_result ---

#[pg_test]
fn test_e2e_async_success() {
    let endpoint = grpcbin_endpoint();

    // Enqueue via the public SQL function.
    let id = crate::grpc_call_async(
        &endpoint,
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "e2e_test"})),
        None,
        None,
    );

    // Confirm the row landed in the queue.
    let in_queue = Spi::get_one::<i64>(
        &format!("SELECT count(*)::bigint FROM grpc.call_queue WHERE id = {id}"),
    )
    .unwrap()
    .unwrap_or(0);
    assert_eq!(in_queue, 1);

    // Simulate the worker: dequeue → execute → persist.
    let rows = crate::queue::dequeue(10);
    let row = rows
        .into_iter()
        .find(|r| r.id == id)
        .expect("row should be dequeued");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(crate::call::call_async_row(row));
    crate::queue::insert_results(vec![result]);

    // Retrieve and assert via the public result function.
    let result_status = Spi::get_one::<String>(
        &format!("SELECT status FROM grpc_call_result({id})"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result_status, "SUCCESS");

    let response = Spi::get_one::<pgrx::JsonB>(
        &format!("SELECT response FROM grpc_call_result({id})"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(response.0["f_string"], "e2e_test");

    Spi::run(&format!("DELETE FROM grpc._call_result WHERE id = {id}")).unwrap();
}

#[pg_test]
fn test_e2e_async_with_metadata_and_options() {
    let endpoint = grpcbin_endpoint();

    let id = crate::grpc_call_async(
        &endpoint,
        "grpcbin.GRPCBin/DummyUnary",
        pgrx::JsonB(serde_json::json!({"f_string": "e2e_meta"})),
        Some(pgrx::JsonB(serde_json::json!({"x-custom-header": "test-value"}))),
        Some(pgrx::JsonB(serde_json::json!({"timeout_ms": 10000}))),
    );

    let rows = crate::queue::dequeue(10);
    let row = rows
        .into_iter()
        .find(|r| r.id == id)
        .expect("row should be dequeued");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(crate::call::call_async_row(row));
    crate::queue::insert_results(vec![result]);

    let result_status = Spi::get_one::<String>(
        &format!("SELECT status FROM grpc_call_result({id})"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result_status, "SUCCESS");

    let response = Spi::get_one::<pgrx::JsonB>(
        &format!("SELECT response FROM grpc_call_result({id})"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(response.0["f_string"], "e2e_meta");

    Spi::run(&format!("DELETE FROM grpc._call_result WHERE id = {id}")).unwrap();
}

#[pg_test]
fn test_e2e_async_connection_error() {

    // Enqueue against an unreachable endpoint with a short timeout.
    let id = crate::grpc_call_async(
        "127.0.0.1:1",
        "pkg.S/M",
        pgrx::JsonB(serde_json::json!({})),
        None,
        Some(pgrx::JsonB(serde_json::json!({"timeout_ms": 500}))),
    );

    let rows = crate::queue::dequeue(10);
    let row = rows
        .into_iter()
        .find(|r| r.id == id)
        .expect("row should be dequeued");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(crate::call::call_async_row(row));
    crate::queue::insert_results(vec![result]);

    let result_status = Spi::get_one::<String>(
        &format!("SELECT status FROM grpc_call_result({id})"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result_status, "ERROR");

    let message = Spi::get_one::<String>(
        &format!("SELECT message FROM grpc_call_result({id})"),
    )
    .unwrap();
    assert!(message.is_some(), "error message should be present");

    Spi::run(&format!("DELETE FROM grpc._call_result WHERE id = {id}")).unwrap();
}

// --- e2e pipeline test (simulates the worker inline) ---

#[pg_test]
fn test_worker_pipeline_processes_grpc_call() {

    let endpoint = grpcbin_endpoint();
    Spi::run(&format!(
        "INSERT INTO grpc.call_queue (id, endpoint, method, request)
         VALUES (20001, '{endpoint}', 'grpcbin.GRPCBin/DummyUnary',
                 '{{\"f_string\": \"pipeline_test\"}}')",
    ))
    .unwrap();

    // Dequeue the row (SPI sees it — same transaction).
    let rows = crate::queue::dequeue(10);
    let row = rows
        .into_iter()
        .find(|r| r.id == 20001)
        .expect("row 20001 should be dequeued");

    // Execute the gRPC call using the same pipeline the worker uses.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(crate::call::call_async_row(row));

    crate::queue::insert_results(vec![result]);

    // Verify via grpc_call_result.
    let status = Spi::get_one::<String>("SELECT status FROM grpc_call_result(20001)")
        .unwrap()
        .unwrap();
    assert_eq!(status, "SUCCESS");

    let response =
        Spi::get_one::<pgrx::JsonB>("SELECT response FROM grpc_call_result(20001)")
            .unwrap()
            .unwrap();
    assert_eq!(response.0["f_string"], "pipeline_test");

    Spi::run("DELETE FROM grpc._call_result WHERE id = 20001").unwrap();
}

#[pg_test]
fn test_worker_pipeline_records_grpc_error() {

    Spi::run(
        "INSERT INTO grpc.call_queue (id, endpoint, method, request, timeout_ms)
         VALUES (20002, '127.0.0.1:1', 'pkg.S/M', '{}', 500)",
    )
    .unwrap();

    let rows = crate::queue::dequeue(10);
    let row = rows
        .into_iter()
        .find(|r| r.id == 20002)
        .expect("row 20002 should be dequeued");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(crate::call::call_async_row(row));
    crate::queue::insert_results(vec![result]);

    let status = Spi::get_one::<String>("SELECT status FROM grpc_call_result(20002)")
        .unwrap()
        .unwrap();
    assert_eq!(status, "ERROR");

    let message = Spi::get_one::<String>("SELECT message FROM grpc_call_result(20002)")
        .unwrap();
    assert!(message.is_some(), "error message should be present");

    Spi::run("DELETE FROM grpc._call_result WHERE id = 20002").unwrap();
}
