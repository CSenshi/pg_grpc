use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::LazyLock;

// Key is the filename used as an `import` target. Re-staging the same filename overwrites.
static PENDING_FILES: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn stage_file(filename: &str, source: &str) {
    PENDING_FILES
        .write()
        .insert(filename.to_owned(), source.to_owned());
}

pub fn snapshot() -> HashMap<String, String> {
    PENDING_FILES.read().clone()
}

pub fn remove(filename: &str) -> bool {
    PENDING_FILES.write().remove(filename).is_some()
}

pub fn list() -> Vec<(String, String)> {
    PENDING_FILES
        .read()
        .iter()
        .map(|(name, source)| (name.clone(), source.clone()))
        .collect()
}

pub fn clear() {
    PENDING_FILES.write().clear();
}
