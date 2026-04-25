use serde_json::Value;

use crate::error::GrpcResult;
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
        match value {
            Value::Null => Ok(Self::default()),
            Value::Object(_) => Ok(Self::default()),
            _ => unreachable!("non-null/non-object handled in later cycle"),
        }
    }
}
