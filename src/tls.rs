use serde_json::Value;
use tonic::transport::{Certificate, ClientTlsConfig};

use crate::error::{GrpcError, GrpcResult};

const ACCEPTED_FIELDS: &[&str] = &["ca_cert"];

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct TlsConfig {
    pub ca_cert: Option<Vec<u8>>,
}

impl TlsConfig {
    pub fn parse(value: &Value) -> GrpcResult<Self> {
        let obj = match value {
            Value::Object(m) => m,
            _ => {
                return Err(GrpcError::Connection(format!(
                    "tls must be a JSON object (accepted fields: {})",
                    ACCEPTED_FIELDS.join(", ")
                )));
            }
        };

        for key in obj.keys() {
            if !ACCEPTED_FIELDS.contains(&key.as_str()) {
                return Err(GrpcError::Connection(format!(
                    "tls: unknown key '{key}' (accepted fields: {})",
                    ACCEPTED_FIELDS.join(", ")
                )));
            }
        }

        let ca_cert = match obj.get("ca_cert") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if s.is_empty() => {
                return Err(GrpcError::Connection(
                    "tls: ca_cert must not be empty".into(),
                ));
            }
            Some(Value::String(s)) => Some(s.as_bytes().to_vec()),
            Some(_) => {
                return Err(GrpcError::Connection(
                    "tls: ca_cert must be a PEM string".into(),
                ));
            }
        };

        Ok(Self { ca_cert })
    }

    pub fn build_client_tls_config(&self) -> ClientTlsConfig {
        let mut cfg = ClientTlsConfig::new().with_native_roots();
        if let Some(ca_cert) = &self.ca_cert {
            cfg = cfg.ca_certificate(Certificate::from_pem(ca_cert.clone()));
        }
        cfg
    }
}
