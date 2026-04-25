use serde_json::Value;
use tonic::transport::{Certificate, ClientTlsConfig, Identity};

use crate::error::{GrpcError, GrpcResult};

const ACCEPTED_FIELDS: &[&str] = &["ca_cert", "client_cert", "client_key", "domain_name"];

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct TlsConfig {
    pub ca_cert: Option<Vec<u8>>,
    pub client_cert: Option<Vec<u8>>,
    pub client_key: Option<Vec<u8>>,
    pub domain_name: Option<String>,
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

        let ca_cert = parse_pem_field(obj, "ca_cert")?;
        let client_cert = parse_pem_field(obj, "client_cert")?;
        let client_key = parse_pem_field(obj, "client_key")?;
        let domain_name = parse_string_field(obj, "domain_name")?;

        match (&client_cert, &client_key) {
            (Some(_), None) => {
                return Err(GrpcError::Connection(
                    "tls: client_cert requires client_key".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(GrpcError::Connection(
                    "tls: client_key requires client_cert".into(),
                ));
            }
            _ => {}
        }

        Ok(Self {
            ca_cert,
            client_cert,
            client_key,
            domain_name,
        })
    }

    pub fn build_client_tls_config(&self) -> ClientTlsConfig {
        let mut cfg = ClientTlsConfig::new().with_native_roots();
        if let Some(ca_cert) = &self.ca_cert {
            cfg = cfg.ca_certificate(Certificate::from_pem(ca_cert.clone()));
        }
        if let (Some(cert), Some(key)) = (&self.client_cert, &self.client_key) {
            cfg = cfg.identity(Identity::from_pem(cert.clone(), key.clone()));
        }
        if let Some(domain) = &self.domain_name {
            cfg = cfg.domain_name(domain.clone());
        }
        cfg
    }
}

fn parse_pem_field(
    obj: &serde_json::Map<String, Value>,
    field: &str,
) -> GrpcResult<Option<Vec<u8>>> {
    Ok(parse_string_field(obj, field)?.map(String::into_bytes))
}

fn parse_string_field(
    obj: &serde_json::Map<String, Value>,
    field: &str,
) -> GrpcResult<Option<String>> {
    match obj.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Err(GrpcError::Connection(format!(
            "tls: {field} must not be empty"
        ))),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(GrpcError::Connection(format!(
            "tls: {field} must be a string"
        ))),
    }
}
