use futures::stream;
use prost::Message as _;
use prost_reflect::DescriptorPool;
use prost_types::FileDescriptorProto;
use protox::file::{ChainFileResolver, File, FileResolver, GoogleFileResolver};
use std::collections::HashMap;
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

/// Compiles a set of in-memory `.proto` files into a [`DescriptorPool`].
///
/// `files` maps filename → proto source. Files may `import` one another
/// (using the filename as the import key) and may import Google Well-Known
/// Types, which are resolved from protox's bundled copies. No filesystem
/// access or network calls are made.
pub fn compile_proto_files(files: HashMap<String, String>) -> GrpcResult<DescriptorPool> {
    if files.is_empty() {
        return Err(GrpcError::ProtoCompile("no proto files supplied".into()));
    }

    let filenames: Vec<String> = files.keys().cloned().collect();

    let mut chain = ChainFileResolver::new();
    chain.add(InMemoryResolver { files });
    chain.add(GoogleFileResolver::new());

    let mut compiler = protox::Compiler::with_file_resolver(chain);
    compiler.include_imports(true);
    for name in &filenames {
        compiler
            .open_file(name)
            .map_err(|e| GrpcError::ProtoCompile(e.to_string()))?;
    }
    let fds = compiler.file_descriptor_set();

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

/// A [`FileResolver`] that serves a set of in-memory `.proto` files.
struct InMemoryResolver {
    files: HashMap<String, String>,
}

impl FileResolver for InMemoryResolver {
    fn open_file(&self, name: &str) -> Result<File, protox::Error> {
        match self.files.get(name) {
            Some(source) => File::from_source(name, source),
            None => Err(protox::Error::file_not_found(name)),
        }
    }
}
