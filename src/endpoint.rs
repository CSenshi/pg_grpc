use crate::error::{GrpcError, GrpcResult};

pub fn validate_endpoint(endpoint: &str) -> GrpcResult<String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(GrpcError::Connection(
            "endpoint must not be empty".to_string(),
        ));
    }
    Ok(trimmed.to_owned())
}
