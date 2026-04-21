use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::LazyLock;
use tonic::transport::Channel;

use crate::endpoint::validate_endpoint;
use crate::error::{GrpcError, GrpcResult};

// Process-global Channel cache, keyed by the validated endpoint string.
// tonic::transport::Channel is Arc-backed, so clones share the same underlying
// connection pool and HTTP/2 session, including its auto-reconnect behavior.
static CHANNELS: LazyLock<RwLock<HashMap<String, Channel>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn get_or_connect(endpoint: &str) -> GrpcResult<Channel> {
    let key = validate_endpoint(endpoint)?;
    if let Some(channel) = CHANNELS.read().get(&key).cloned() {
        return Ok(channel);
    }
    let channel = Channel::from_shared(format!("http://{key}"))
        .map_err(|e| GrpcError::Connection(e.to_string()))?
        .connect()
        .await
        .map_err(|e| GrpcError::Connection(format!("{key}: {e}")))?;
    Ok(CHANNELS
        .write()
        .entry(key)
        .or_insert(channel)
        .clone())
}

#[cfg(any(test, feature = "pg_test"))]
pub fn len() -> usize {
    CHANNELS.read().len()
}

#[cfg(any(test, feature = "pg_test"))]
pub fn clear() {
    CHANNELS.write().clear();
}
