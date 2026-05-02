#[pg_test]
fn test_dequeue_returns_queued_row() {
    Spi::run(
        "INSERT INTO grpc.call_queue (endpoint, method, request)
         VALUES ('host:50051', 'pkg.S/M', '{\"key\": \"val\"}')",
    )
    .unwrap();

    let rows = crate::queue::dequeue(10);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].endpoint, "host:50051");
    assert_eq!(rows[0].method, "pkg.S/M");
    assert_eq!(rows[0].request["key"], "val");

    Spi::run("DELETE FROM grpc.call_queue").unwrap();
}

#[pg_test]
fn test_dequeue_marks_row_processing_preventing_double_dequeue() {
    Spi::run(
        "INSERT INTO grpc.call_queue (endpoint, method, request)
         VALUES ('host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();

    let first = crate::queue::dequeue(10);
    let second = crate::queue::dequeue(10);

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 0); // already 'processing', not re-dequeued

    Spi::run("DELETE FROM grpc.call_queue").unwrap();
}

#[pg_test]
fn test_insert_results_writes_success_and_removes_from_queue() {
    Spi::run(
        "INSERT INTO grpc.call_queue (id, endpoint, method, request)
         VALUES (1001, 'host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();
    crate::queue::dequeue(10); // mark as processing

    crate::queue::insert_results(vec![crate::queue::CallResult {
        id: 1001,
        outcome: crate::queue::CallOutcome::Success(serde_json::json!({"ok": true})),
    }]);

    let queue_count = Spi::get_one::<i64>(
        "SELECT count(*)::bigint FROM grpc.call_queue WHERE id = 1001",
    )
    .unwrap()
    .unwrap();
    assert_eq!(queue_count, 0); // removed from queue

    let result_status = Spi::get_one::<String>(
        "SELECT status FROM grpc._call_result WHERE id = 1001",
    )
    .unwrap()
    .unwrap();
    assert_eq!(result_status, "SUCCESS");

    Spi::run("DELETE FROM grpc._call_result WHERE id = 1001").unwrap();
}

#[pg_test]
fn test_lookup_returns_pending_for_queued_row() {
    Spi::run(
        "INSERT INTO grpc.call_queue (id, endpoint, method, request)
         VALUES (2001, 'host:50051', 'pkg.S/M', '{}')",
    )
    .unwrap();

    let result = crate::queue::lookup(2001);

    assert!(matches!(result.status, crate::queue::LookupStatus::Pending));

    Spi::run("DELETE FROM grpc.call_queue WHERE id = 2001").unwrap();
}

#[pg_test]
fn test_lookup_returns_success_for_completed_result() {
    Spi::run(
        "INSERT INTO grpc._call_result (id, status, response)
         VALUES (3001, 'SUCCESS', '{\"value\": 42}')",
    )
    .unwrap();

    let result = crate::queue::lookup(3001);

    assert!(matches!(result.status, crate::queue::LookupStatus::Success(_)));
    if let crate::queue::LookupStatus::Success(v) = result.status {
        assert_eq!(v["value"], 42);
    }

    Spi::run("DELETE FROM grpc._call_result WHERE id = 3001").unwrap();
}

#[pg_test]
fn test_lookup_returns_error_for_failed_result() {
    Spi::run(
        "INSERT INTO grpc._call_result (id, status, error_msg)
         VALUES (4001, 'ERROR', 'connection refused')",
    )
    .unwrap();

    let result = crate::queue::lookup(4001);

    assert!(matches!(result.status, crate::queue::LookupStatus::Error(_)));
    if let crate::queue::LookupStatus::Error(msg) = result.status {
        assert_eq!(msg, "connection refused");
    }

    Spi::run("DELETE FROM grpc._call_result WHERE id = 4001").unwrap();
}

#[pg_test]
fn test_lookup_returns_error_for_missing_id() {
    let result = crate::queue::lookup(99999);

    assert!(matches!(result.status, crate::queue::LookupStatus::Error(_)));
    if let crate::queue::LookupStatus::Error(msg) = result.status {
        assert!(msg.contains("not found"));
    }
}

#[pg_test]
fn test_ttl_cleanup_removes_expired_rows() {
    Spi::run(
        "INSERT INTO grpc._call_result (id, status, created)
         VALUES (5001, 'SUCCESS', now() - INTERVAL '7 hours')",
    )
    .unwrap();

    crate::queue::ttl_cleanup("6 hours");

    let count = Spi::get_one::<i64>(
        "SELECT count(*)::bigint FROM grpc._call_result WHERE id = 5001",
    )
    .unwrap()
    .unwrap();
    assert_eq!(count, 0);
}
