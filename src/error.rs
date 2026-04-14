use thiserror::Error;

#[derive(Debug, Error)]
pub enum GrpcError {
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Proto error: {0}")]
    Proto(String),
    #[error("Proto compile error: {0}")]
    ProtoCompile(String),
    #[error("gRPC call failed: {0}")]
    Call(String),
}

pub type GrpcResult<T> = Result<T, GrpcError>;
