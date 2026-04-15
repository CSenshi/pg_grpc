use once_cell::sync::Lazy;
use parking_lot::RwLock;
use prost_reflect::DescriptorPool;
use std::collections::HashMap;

/// A registered service: the compiled descriptor pool plus the name and
/// source of the `.proto` file that originally defined it (captured at
/// compile time). The filename + source are what `grpc_proto_list_registered`
/// exposes back to callers so they can inspect, diff, or dump their
/// registered schemas.
#[derive(Clone)]
struct RegisteredService {
    pool: DescriptorPool,
    filename: String,
    source: String,
}

/// Per-backend-process registry of compiled proto descriptors keyed by
/// fully-qualified service name (e.g. `"pkg.Service"`).
///
/// Populated by `grpc_proto_compile()` and consulted by `grpc_call()` before
/// it falls back to gRPC server reflection, so callers can target servers
/// that do not expose reflection.
static PROTO_REGISTRY: Lazy<RwLock<HashMap<String, RegisteredService>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Returns a clone of the registered pool for the given service name, if any.
pub fn get_proto(service_name: &str) -> Option<DescriptorPool> {
    PROTO_REGISTRY
        .read()
        .get(service_name)
        .map(|r| r.pool.clone())
}

/// Registers (or overwrites) an entry for the given service name.
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

/// Removes the entry registered under `service_name`, if any. Returns `true`
/// if an entry was removed.
pub fn remove(service_name: &str) -> bool {
    PROTO_REGISTRY.write().remove(service_name).is_some()
}

/// Clears every registered entry.
pub fn clear() {
    PROTO_REGISTRY.write().clear();
}

/// Returns `(service_name, filename, source)` for every registered service.
/// One row per service — a file defining multiple services produces multiple
/// rows (all sharing the same filename/source). Files that contribute no
/// services (e.g. shared message-only files pulled in via `import`) do not
/// appear — this is a list of *callable* services, not of every file seen.
pub fn list() -> Vec<(String, String, String)> {
    PROTO_REGISTRY
        .read()
        .iter()
        .map(|(service_name, r)| (service_name.clone(), r.filename.clone(), r.source.clone()))
        .collect()
}
