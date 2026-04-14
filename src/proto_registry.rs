use once_cell::sync::Lazy;
use parking_lot::RwLock;
use prost_reflect::DescriptorPool;
use std::collections::HashMap;

/// Per-backend-process registry of compiled proto descriptors keyed by
/// fully-qualified service name (e.g. `"pkg.Service"`).
///
/// Populated by `grpc_proto_compile()` and consulted by `grpc_call()` before
/// it falls back to gRPC server reflection, so callers can target servers
/// that do not expose reflection.
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
