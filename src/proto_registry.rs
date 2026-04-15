use parking_lot::RwLock;
use prost_reflect::DescriptorPool;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Clone)]
struct RegisteredService {
    pool: DescriptorPool,
    filename: String,
    source: String,
}

// Keyed by fully-qualified service name (e.g. `"pkg.Service"`).
static PROTO_REGISTRY: LazyLock<RwLock<HashMap<String, RegisteredService>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn get_proto(service_name: &str) -> Option<DescriptorPool> {
    PROTO_REGISTRY
        .read()
        .get(service_name)
        .map(|r| r.pool.clone())
}

pub fn insert_proto(service_name: &str, pool: DescriptorPool, filename: String, source: String) {
    PROTO_REGISTRY.write().insert(
        service_name.to_owned(),
        RegisteredService {
            pool,
            filename,
            source,
        },
    );
}

pub fn remove(service_name: &str) -> bool {
    PROTO_REGISTRY.write().remove(service_name).is_some()
}

pub fn clear() {
    PROTO_REGISTRY.write().clear();
}

pub fn list() -> Vec<(String, String, String)> {
    PROTO_REGISTRY
        .read()
        .iter()
        .map(|(service_name, r)| (service_name.clone(), r.filename.clone(), r.source.clone()))
        .collect()
}
