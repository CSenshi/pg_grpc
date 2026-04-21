use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::LazyLock;
use tonic::transport::Channel;

use crate::endpoint::validate_endpoint;
use crate::error::{GrpcError, GrpcResult};
use crate::tls::TlsConfig;

// Cache key is (endpoint, Option<TlsConfig>) so two calls with the same host
// but different TLS settings resolve to distinct Channels.
type Key = (String, Option<TlsConfig>);

static CHANNELS: LazyLock<RwLock<HashMap<Key, Channel>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn get_or_connect(endpoint: &str, tls: Option<&TlsConfig>) -> GrpcResult<Channel> {
    let host = validate_endpoint(endpoint)?;
    let key: Key = (host.clone(), tls.cloned());

    if let Some(channel) = CHANNELS.read().get(&key).cloned() {
        return Ok(channel);
    }

    // Scheme flips to https so tonic's transport negotiates ALPN/TLS; the
    // host part is identical to the plaintext case.
    let scheme = if tls.is_some() { "https" } else { "http" };
    let mut builder = Channel::from_shared(format!("{scheme}://{host}"))
        .map_err(|e| GrpcError::Connection(e.to_string()))?;

    if let Some(tls_cfg) = tls {
        builder = builder
            .tls_config(tls_cfg.build_client_tls_config())
            .map_err(|e| GrpcError::Connection(format!("TLS config: {e}")))?;
    }

    let channel = builder
        .connect()
        .await
        .map_err(|e| GrpcError::Connection(format!("{host}: {e}")))?;

    Ok(CHANNELS.write().entry(key).or_insert(channel).clone())
}

#[cfg(any(test, feature = "pg_test"))]
pub fn len() -> usize {
    CHANNELS.read().len()
}

#[cfg(any(test, feature = "pg_test"))]
pub fn clear() {
    CHANNELS.write().clear();
}
