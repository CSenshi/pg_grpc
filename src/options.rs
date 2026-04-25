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
        if let Some(v) = obj.get("use_reflection") {
            cfg.use_reflection = Some(parse_bool("use_reflection", v)?);
        }
        if let Some(v) = obj.get("tls") {
            cfg.tls = match v {
                Value::Null => None,
                Value::Object(_) => Some(TlsConfig::parse(v)?),
                _ => {
                    return Err(GrpcError::Call(
                        "options.tls must be an object".to_string(),
                    ));
                }
            };
        }
        if let Some(v) = obj.get("max_decode_message_size_bytes") {
            cfg.max_decode_message_size_bytes =
                Some(parse_size_u32("max_decode_message_size_bytes", v)?);
        }
        if let Some(v) = obj.get("max_encode_message_size_bytes") {
            cfg.max_encode_message_size_bytes =
                Some(parse_size_u32("max_encode_message_size_bytes", v)?);
        }
        Ok(cfg)
    }
}

// gRPC's wire framing uses a 4-byte length prefix; values above u32::MAX
// can never be a valid single-message size.
fn parse_size_u32(key: &str, value: &Value) -> GrpcResult<u32> {
    let n = value
        .as_i64()
        .ok_or_else(|| GrpcError::Call(format!("options.{key} must be an integer")))?;
    if n < 1 {
        return Err(GrpcError::Call(format!(
            "options.{key} must be in [1, {}] (got {n})",
            u32::MAX
        )));
    }
    if n > u32::MAX as i64 {
        return Err(GrpcError::Call(format!(
            "options.{key} must be in [1, {}] (got {n})",
            u32::MAX
        )));
    }
    Ok(n as u32)
}

fn parse_bool(key: &str, value: &Value) -> GrpcResult<bool> {
    value
        .as_bool()
        .ok_or_else(|| GrpcError::Call(format!("options.{key} must be a boolean")))
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
