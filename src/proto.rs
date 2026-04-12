use futures::stream;
use prost::Message as _;
use prost_reflect::DescriptorPool;
use prost_types::FileDescriptorProto;
use tonic::transport::Channel;
use tonic_reflection::pb::v1alpha::{
    server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
    server_reflection_response::MessageResponse, ServerReflectionRequest,
};

use crate::error::{GrpcError, GrpcResult};

pub async fn fetch_pool(channel: Channel, service_name: &str) -> GrpcResult<DescriptorPool> {
    let mut client = ServerReflectionClient::new(channel);

    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::FileContainingSymbol(
            service_name.to_string(),
        )),
    };

    let mut resp_stream = client
        .server_reflection_info(tonic::Request::new(stream::iter(vec![request])))
        .await
        .map_err(|e| GrpcError::Proto(format!("reflection call failed: {e}")))?
        .into_inner();

    let mut pool = DescriptorPool::new();

    while let Some(msg) = resp_stream
        .message()
        .await
        .map_err(|e| GrpcError::Proto(format!("reflection stream error: {e}")))?
    {
        match msg.message_response {
            Some(MessageResponse::FileDescriptorResponse(fdr)) => {
                for bytes in fdr.file_descriptor_proto {
                    let fdp = FileDescriptorProto::decode(bytes.as_ref()).map_err(|e| {
                        GrpcError::Proto(format!("decode FileDescriptorProto: {e}"))
                    })?;
                    pool.add_file_descriptor_proto(fdp)
                        .map_err(|e| GrpcError::Proto(format!("add to descriptor pool: {e}")))?;
                }
            }
            Some(MessageResponse::ErrorResponse(e)) => {
                return Err(GrpcError::Proto(format!(
                    "reflection error (code {}): {}",
                    e.error_code, e.error_message
                )));
            }
            _ => {}
        }
    }

    if pool.services().count() == 0 {
        return Err(GrpcError::Proto(format!(
            "reflection returned no descriptors for service: {service_name}"
        )));
    }

    Ok(pool)
}
