use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;

/// Per-backend-process staging area for `.proto` sources awaiting compile.
///
/// `grpc_proto_register(filename, source)` inserts here; `grpc_proto_compile()`
/// snapshots this map, hands it to the compiler, and clears the staging area
/// on success so the next register/compile cycle starts fresh.
///
/// Key: filename as referenced by `import` statements. Re-registering the
/// same filename overwrites.
static PENDING_FILES: Lazy<RwLock<HashMap<String, String>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Stages a `.proto` file for the next compile. Overwrites any existing
/// entry with the same filename.
pub fn stage_file(filename: &str, source: &str) {
    PENDING_FILES
        .write()
        .insert(filename.to_owned(), source.to_owned());
}

/// Returns a snapshot of the currently staged files (without clearing).
pub fn snapshot() -> HashMap<String, String> {
    PENDING_FILES.read().clone()
}

/// Clears the staging area. Called after a successful compile.
pub fn clear() {
    PENDING_FILES.write().clear();
}
