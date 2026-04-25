use serde_json::Value;

use crate::error::{GrpcError, GrpcResult};
use crate::tls::TlsConfig;

#[derive(Debug, Default)]
pub struct OptionsConfig {
    pub timeout_ms: Option<u64>,
    pub use_reflection: Option<bool>,
    pub tls: Option<TlsConfig>,
    pub max_decode_message_size_bytes: Option<u32>,
    pub max_encode_message_size_bytes: Option<u32>,
}

impl OptionsConfig {
    pub fn parse(value: &Value) -> GrpcResult<Self> {
        let obj = match value {
            Value::Null => return Ok(Self::default()),
            Value::Object(m) => m,
            _ => unreachable!("non-null/non-object handled in later cycle"),
        };

        let mut cfg = Self::default();
        if let Some(v) = obj.get("timeout_ms") {
            cfg.timeout_ms = Some(parse_positive_u64("timeout_ms", v)?);
        }
        Ok(cfg)
    }
}

fn parse_positive_u64(key: &str, value: &Value) -> GrpcResult<u64> {
    let n = value
        .as_i64()
        .ok_or_else(|| GrpcError::Call(format!("options.{key} must be an integer")))?;
    if n < 1 {
        return Err(GrpcError::Call(format!(
            "options.{key} must be >= 1 (got {n})"
        )));
    }
    Ok(n as u64)
}
