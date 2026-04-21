#[pg_test]
fn test_channel_cache_first_lookup_stores_channel() {
    crate::channel_cache::clear();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let endpoint = grpcbin_endpoint();
    rt.block_on(async {
        crate::channel_cache::get_or_connect(&endpoint)
            .await
            .expect("first lookup should connect");
    });
    assert_eq!(crate::channel_cache::len(), 1);
    crate::channel_cache::clear();
}
