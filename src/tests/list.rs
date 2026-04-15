#[pg_test]
fn test_list_staged_empty() {
    crate::grpc_proto_unstage_all();

    let rows: Vec<(String, String)> = crate::grpc_proto_list_staged().collect();
    assert_eq!(
        rows.len(),
        0,
        "list_staged should return no rows when nothing is staged"
    );
}

#[pg_test]
fn test_list_staged_returns_filename_and_source() {
    crate::grpc_proto_unstage_all();

    let a_source = "syntax = \"proto3\";\npackage a;\nmessage A { string x = 1; }\n";
    let b_source = "syntax = \"proto3\";\npackage b;\nmessage B { int32 y = 1; int32 z = 2; }\n";
    crate::grpc_proto_stage("a.proto", a_source);
    crate::grpc_proto_stage("b.proto", b_source);

    let mut rows: Vec<(String, String)> = crate::grpc_proto_list_staged().collect();
    rows.sort_by(|l, r| l.0.cmp(&r.0));

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "a.proto");
    assert_eq!(rows[0].1, a_source);
    assert_eq!(rows[1].0, "b.proto");
    assert_eq!(rows[1].1, b_source);

    crate::grpc_proto_unstage_all();
}

#[pg_test]
fn test_list_staged_cleared_after_successful_compile() {
    // Compile moves everything from staging to registry, so list_staged
    // should be empty afterwards.
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();
    crate::grpc_proto_stage(
        "svc.proto",
        r#"
        syntax = "proto3";
        package list_compile_test;
        service S { rpc M(Msg) returns (Msg); }
        message Msg { string x = 1; }
        "#,
    );
    assert_eq!(
        crate::grpc_proto_list_staged().count(),
        1,
        "file should appear in staging before compile"
    );

    crate::grpc_proto_compile();
    assert_eq!(
        crate::grpc_proto_list_staged().count(),
        0,
        "staging should be empty after successful compile"
    );

    crate::grpc_proto_unregister_all();
}

#[pg_test]
fn test_list_registered_empty() {
    crate::grpc_proto_unregister_all();

    let rows: Vec<(String, String, String)> = crate::grpc_proto_list_registered().collect();
    assert_eq!(
        rows.len(),
        0,
        "list_registered should return no rows when nothing is compiled"
    );
}

#[pg_test]
fn test_list_registered_row_per_service() {
    // Two separate service-bearing files plus a shared message-only
    // common.proto. Expect one row per service, each carrying its own
    // originating filename and source. common.proto contributes no services
    // and must not appear.
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();

    let common_source = r#"
        syntax = "proto3";
        package list_reg_test;
        message Msg { string x = 1; }
    "#;
    let alpha_source = r#"
        syntax = "proto3";
        import "common.proto";
        package list_reg_test;
        service Alpha { rpc M(Msg) returns (Msg); }
    "#;
    let beta_source = r#"
        syntax = "proto3";
        import "common.proto";
        package list_reg_test;
        service Beta { rpc M(Msg) returns (Msg); }
    "#;

    crate::grpc_proto_stage("common.proto", common_source);
    crate::grpc_proto_stage("alpha.proto", alpha_source);
    crate::grpc_proto_stage("beta.proto", beta_source);
    crate::grpc_proto_compile();

    let mut rows: Vec<(String, String, String)> = crate::grpc_proto_list_registered()
        .filter(|(svc, _, _)| svc.starts_with("list_reg_test."))
        .collect();
    rows.sort_by(|l, r| l.0.cmp(&r.0));

    assert_eq!(rows.len(), 2, "one row per registered service");
    assert_eq!(rows[0].0, "list_reg_test.Alpha");
    assert_eq!(rows[0].1, "alpha.proto");
    assert_eq!(rows[0].2, alpha_source);
    assert_eq!(rows[1].0, "list_reg_test.Beta");
    assert_eq!(rows[1].1, "beta.proto");
    assert_eq!(rows[1].2, beta_source);

    // common.proto must NOT appear — it has no services
    assert!(
        crate::grpc_proto_list_registered()
            .all(|(_, filename, _)| filename != "common.proto"),
        "import-only files should not appear in list_registered"
    );

    crate::grpc_proto_unregister_all();
}

#[pg_test]
fn test_list_registered_multi_service_file() {
    // A single file with two services produces TWO rows — one per service —
    // both sharing the same filename and source. Users who want the deduped
    // file view can `SELECT DISTINCT filename, source`.
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();

    let multi_source = r#"
        syntax = "proto3";
        package list_multi_test;
        service Alpha { rpc One(Msg) returns (Msg); }
        service Beta  { rpc Two(Msg) returns (Msg); }
        message Msg { string x = 1; }
    "#;
    crate::grpc_proto_stage("multi.proto", multi_source);
    crate::grpc_proto_compile();

    let mut rows: Vec<(String, String, String)> = crate::grpc_proto_list_registered()
        .filter(|(svc, _, _)| svc.starts_with("list_multi_test."))
        .collect();
    rows.sort_by(|l, r| l.0.cmp(&r.0));

    assert_eq!(rows.len(), 2, "multi-service file should produce one row per service");
    for row in &rows {
        assert_eq!(row.1, "multi.proto");
        assert_eq!(row.2, multi_source);
    }
    assert_eq!(rows[0].0, "list_multi_test.Alpha");
    assert_eq!(rows[1].0, "list_multi_test.Beta");

    crate::grpc_proto_unregister_all();
}

#[pg_test]
fn test_list_registered_shrinks_per_service_unregister() {
    // Per-service unregistration should remove exactly that row. The other
    // service from the same file remains.
    crate::grpc_proto_unregister_all();
    crate::grpc_proto_unstage_all();
    crate::grpc_proto_stage(
        "pair.proto",
        r#"
        syntax = "proto3";
        package list_unregister_test;
        service Alpha { rpc M(Msg) returns (Msg); }
        service Beta  { rpc M(Msg) returns (Msg); }
        message Msg { string x = 1; }
        "#,
    );
    crate::grpc_proto_compile();

    assert_eq!(
        crate::grpc_proto_list_registered()
            .filter(|(svc, _, _)| svc.starts_with("list_unregister_test."))
            .count(),
        2,
        "both services should be listed after compile"
    );

    assert!(crate::grpc_proto_unregister("list_unregister_test.Alpha"));

    let rows: Vec<(String, String, String)> = crate::grpc_proto_list_registered()
        .filter(|(svc, _, _)| svc.starts_with("list_unregister_test."))
        .collect();
    assert_eq!(rows.len(), 1, "only Beta should remain");
    assert_eq!(rows[0].0, "list_unregister_test.Beta");
    assert_eq!(rows[0].1, "pair.proto");

    crate::grpc_proto_unregister_all();
}
