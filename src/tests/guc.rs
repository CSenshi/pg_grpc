#[pg_test]
fn test_guc_batch_size_default() {
    let val = Spi::get_one::<String>("SHOW pg_grpc.batch_size")
        .unwrap()
        .unwrap();
    assert_eq!(val, "200");
}

#[pg_test]
fn test_guc_ttl_default() {
    let val = Spi::get_one::<String>("SHOW pg_grpc.ttl")
        .unwrap()
        .unwrap();
    assert_eq!(val, "6 hours");
}

#[pg_test]
fn test_guc_database_name_default() {
    let val = Spi::get_one::<String>("SHOW pg_grpc.database_name")
        .unwrap()
        .unwrap();
    // postgresql_conf_options() sets this to "pgrx_tests" for the test runner.
    assert_eq!(val, "pgrx_tests");
}

#[pg_test]
fn test_guc_username_default() {
    let val = Spi::get_one::<String>("SHOW pg_grpc.username")
        .unwrap()
        .unwrap();
    assert_eq!(val, "");
}
