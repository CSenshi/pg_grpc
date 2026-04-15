use parking_lot::RwLock;
use prost_reflect::DescriptorPool;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Clone)]
pub enum Origin {
    UserStaged { filename: String, source: String },
    Reflection { endpoint: String },
}

#[derive(Clone)]
struct RegisteredService {
    pool: DescriptorPool,
    origin: Origin,
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

pub fn insert_proto_manual(
    service_name: &str,
    pool: DescriptorPool,
    filename: String,
    source: String,
) {
    PROTO_REGISTRY.write().insert(
        service_name.to_owned(),
        RegisteredService {
            pool,
            origin: Origin::UserStaged { filename, source },
        },
    );
}

pub fn insert_proto_reflection(service_name: &str, pool: DescriptorPool, endpoint: String) {
    PROTO_REGISTRY.write().insert(
        service_name.to_owned(),
        RegisteredService {
            pool,
            origin: Origin::Reflection { endpoint },
        },
    );
}

pub fn remove(service_name: &str) -> bool {
    PROTO_REGISTRY.write().remove(service_name).is_some()
}

pub fn clear() {
    PROTO_REGISTRY.write().clear();
}

pub fn list() -> Vec<(String, Origin)> {
    PROTO_REGISTRY
        .read()
        .iter()
        .map(|(service_name, r)| (service_name.clone(), r.origin.clone()))
        .collect()
}
