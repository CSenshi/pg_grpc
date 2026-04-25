use futures::stream;
use prost::Message as _;
use prost_reflect::DescriptorPool;
use prost_types::FileDescriptorProto;
use protox::file::{ChainFileResolver, File, FileResolver, GoogleFileResolver};
use std::collections::HashMap;
use tonic::transport::Channel;

use crate::error::{GrpcError, GrpcResult};

pub async fn fetch_pool(
    channel: Channel,
    service_name: &str,
    max_decode: Option<u32>,
    max_encode: Option<u32>,
) -> GrpcResult<DescriptorPool> {
    // Prefer the stable v1 service (grpc.reflection.v1.ServerReflection).
    // Fall back to v1alpha if the server hasn't implemented v1 yet.
    match fetch_v1(channel.clone(), service_name, max_decode, max_encode).await {
        Ok(pool) => Ok(pool),
        Err(s) if s.code() == tonic::Code::Unimplemented => {
            fetch_v1alpha(channel, service_name, max_decode, max_encode).await
        }
        Err(s) => Err(s),
    }
    .map_err(|s| GrpcError::Proto(format!("reflection: {}", s.message())))
}

macro_rules! define_fetch {
    ($name:ident, $pb:path) => {
        async fn $name(
            channel: Channel,
            service_name: &str,
            max_decode: Option<u32>,
            max_encode: Option<u32>,
        ) -> Result<DescriptorPool, tonic::Status> {
            use $pb::{
                server_reflection_client::ServerReflectionClient,
                server_reflection_request::MessageRequest,
                server_reflection_response::MessageResponse, ServerReflectionRequest,
            };

            let mut client = ServerReflectionClient::new(channel);
            if let Some(n) = max_decode {
                client = client.max_decoding_message_size(n as usize);
            }
            if let Some(n) = max_encode {
                client = client.max_encoding_message_size(n as usize);
            }
            let request = ServerReflectionRequest {
                host: String::new(),
                message_request: Some(MessageRequest::FileContainingSymbol(
                    service_name.to_string(),
                )),
            };
            let mut resp_stream = client
                .server_reflection_info(tonic::Request::new(stream::iter(vec![request])))
                .await?
                .into_inner();

            let mut pool = DescriptorPool::new();
            while let Some(msg) = resp_stream.message().await? {
                match msg.message_response {
                    Some(MessageResponse::FileDescriptorResponse(fdr)) => {
                        for bytes in fdr.file_descriptor_proto {
                            let fdp = FileDescriptorProto::decode(bytes.as_ref()).map_err(|e| {
                                tonic::Status::internal(format!("decode FileDescriptorProto: {e}"))
                            })?;
                            pool.add_file_descriptor_proto(fdp).map_err(|e| {
                                tonic::Status::internal(format!("add to descriptor pool: {e}"))
                            })?;
                        }
                    }
                    Some(MessageResponse::ErrorResponse(e)) => {
                        return Err(tonic::Status::internal(format!(
                            "reflection error (code {}): {}",
                            e.error_code, e.error_message
                        )));
                    }
                    _ => {}
                }
            }

            if pool.services().count() == 0 {
                return Err(tonic::Status::internal(format!(
                    "reflection returned no descriptors for service: {service_name}"
                )));
            }
            Ok(pool)
        }
    };
}

define_fetch!(fetch_v1, tonic_reflection::pb::v1);
define_fetch!(fetch_v1alpha, tonic_reflection::pb::v1alpha);

// Imports resolve against the filename keys in `files` and against protox's
// bundled Google Well-Known Types. No filesystem or network access.
pub fn compile_proto_files(files: HashMap<String, String>) -> GrpcResult<DescriptorPool> {
    if files.is_empty() {
        return Err(GrpcError::ProtoCompile("no proto files supplied".into()));
    }

    let filenames: Vec<String> = files.keys().cloned().collect();

    let mut chain = ChainFileResolver::new();
    chain.add(InMemoryResolver { files });
    chain.add(GoogleFileResolver::new());

    let mut compiler = protox::Compiler::with_file_resolver(chain);
    // Without include_imports, transitively-imported files (incl. WKTs) are omitted from the
    // FileDescriptorSet and add_file_descriptor_proto below fails to resolve their types.
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

    backfill_wkts(&mut pool)?;
    Ok(pool)
}

// Seed the pool with every Google Well-Known Type that prost_reflect ships
// (Any, Duration, Timestamp, wrappers, Struct, ...). Without this, a JSON
// `Any` payload whose `@type` URL references a WKT (e.g. StringValue) cannot
// be resolved at encode time unless the user happened to import the
// containing proto. User-supplied or server-returned files added before this
// call always win — same-name files are skipped, never overridden.
pub fn backfill_wkts(pool: &mut DescriptorPool) -> GrpcResult<()> {
    let global = DescriptorPool::global();
    for file in global.files() {
        let name = file.name().to_owned();
        // prost_reflect's add_file_descriptor_proto dedupes by filename
        // already; the explicit guard here makes the user-staged-wins
        // policy visible to readers and avoids relying on that dedup.
        if pool.get_file_by_name(&name).is_some() {
            continue;
        }
        pool.add_file_descriptor_proto(file.file_descriptor_proto().clone())
            .map_err(|e| GrpcError::ProtoCompile(format!("seed WKT {name}: {e}")))?;
    }
    Ok(())
}

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
