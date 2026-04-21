use crate::error::{GrpcError, GrpcResult};

pub fn validate_endpoint(endpoint: &str) -> GrpcResult<String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(GrpcError::Connection(
            "endpoint must not be empty".to_string(),
        ));
    }
    if trimmed.contains("://") {
        return Err(GrpcError::Connection(format!(
            "endpoint must not contain scheme (found '://'): {trimmed}"
        )));
    }
    if trimmed.contains('/') {
        return Err(GrpcError::Connection(format!(
            "endpoint must not contain path (found '/'): {trimmed}"
        )));
    }
    Ok(trimmed.to_owned())
}
