#[pg_test]
fn test_channel_cache_first_lookup_stores_channel() {
    crate::channel_cache::clear();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let endpoint = grpcbin_endpoint();
    rt.block_on(async {
        crate::channel_cache::get_or_connect(&endpoint, None)
            .await
            .expect("first lookup should connect");
    });
    assert_eq!(crate::channel_cache::len(), 1);
    crate::channel_cache::clear();
}

#[pg_test]
fn test_channel_cache_second_lookup_is_cache_hit() {
    crate::channel_cache::clear();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let endpoint = grpcbin_endpoint();
    rt.block_on(async {
        crate::channel_cache::get_or_connect(&endpoint, None)
            .await
            .expect("first lookup should connect");
        crate::channel_cache::get_or_connect(&endpoint, None)
            .await
            .expect("second lookup should hit cache");
    });
    assert_eq!(
        crate::channel_cache::len(),
        1,
        "second lookup must not create a new entry"
    );
    crate::channel_cache::clear();
}
