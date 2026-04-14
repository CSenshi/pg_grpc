use once_cell::sync::Lazy;
use parking_lot::RwLock;
use prost_reflect::DescriptorPool;
use std::collections::HashMap;

/// Per-backend-process registry of user-supplied proto descriptors.
///
/// Populated via `grpc_register_proto()` and consulted by `grpc_call()`
/// before falling back to gRPC server reflection. This lets callers target
/// servers that do not expose reflection.
///
/// Key: fully-qualified service name (e.g. "pkg.Service").
static PROTO_REGISTRY: Lazy<RwLock<HashMap<String, DescriptorPool>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Returns a clone of the registered pool for the given service name, if any.
pub fn get_proto(service_name: &str) -> Option<DescriptorPool> {
    PROTO_REGISTRY.read().get(service_name).cloned()
}

/// Registers (or overwrites) a pool for the given service name.
pub fn insert_proto(service_name: &str, pool: DescriptorPool) {
    PROTO_REGISTRY.write().insert(service_name.to_owned(), pool);
}
