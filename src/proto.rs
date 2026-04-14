use futures::stream;
use prost::Message as _;
use prost_reflect::DescriptorPool;
use prost_types::FileDescriptorProto;
use protox::file::{ChainFileResolver, File, FileResolver, GoogleFileResolver};
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

// ── User-supplied .proto compilation ─────────────────────────────────────────

/// Compiles raw `.proto` source text into a [`DescriptorPool`].
///
/// No filesystem access or network calls are made. Well-Known Types
/// (`google/protobuf/*.proto`) are resolved from protox's bundled copies.
pub fn compile_proto_source(proto_source: &str) -> GrpcResult<DescriptorPool> {
    // Build an in-memory resolver that serves the user's source, chained with
    // the bundled Google WKT resolver so imports like google/protobuf/timestamp.proto work.
    let mut chain = ChainFileResolver::new();
    chain.add(InMemoryResolver {
        name: "input.proto".to_owned(),
        source: proto_source.to_owned(),
    });
    chain.add(GoogleFileResolver::new());

    let fds = protox::Compiler::with_file_resolver(chain)
        .include_imports(true)
        .open_file("input.proto")
        .map_err(|e| GrpcError::ProtoCompile(e.to_string()))?
        .file_descriptor_set();

    let mut pool = DescriptorPool::new();
    for fdp in fds.file {
        pool.add_file_descriptor_proto(fdp)
            .map_err(|e| GrpcError::ProtoCompile(e.to_string()))?;
    }

    if pool.services().count() == 0 {
        return Err(GrpcError::ProtoCompile(
            "proto source defines no services".to_string(),
        ));
    }

    Ok(pool)
}

/// A [`FileResolver`] that serves a single in-memory `.proto` file.
struct InMemoryResolver {
    name: String,
    source: String,
}

impl FileResolver for InMemoryResolver {
    fn open_file(&self, name: &str) -> Result<File, protox::Error> {
        if name == self.name {
            File::from_source(&self.name, &self.source)
        } else {
            Err(protox::Error::file_not_found(name))
        }
    }
}
