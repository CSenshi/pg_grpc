use bytes::{Buf, BufMut, Bytes};
use prost::Message as _;
use prost_reflect::{
    DescriptorPool, DynamicMessage, MessageDescriptor, MethodDescriptor, SerializeOptions,
};
use serde::de::DeserializeSeed as _;
use tonic::transport::Channel;

use crate::error::{GrpcError, GrpcResult};
use crate::proto;

pub fn make_grpc_call(
    endpoint: &str,
    method: &str,
    request_json: serde_json::Value,
) -> GrpcResult<serde_json::Value> {
    // pgrx backends are single-threaded; a multi-thread runtime would unsafely cross the Postgres boundary.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| GrpcError::Connection(e.to_string()))?;
    rt.block_on(call_async(endpoint, method, request_json))
}

async fn call_async(
    endpoint: &str,
    method: &str,
    request_json: serde_json::Value,
) -> GrpcResult<serde_json::Value> {
    let (service_name, method_name) = parse_method(method)?;
    let channel = connect(endpoint).await?;
    let pool = match crate::proto_registry::get_proto(&service_name) {
        Some(pool) => pool,
        None => proto::fetch_pool(channel.clone(), &service_name).await?,
    };
    let method_desc = resolve_method(&pool, &service_name, &method_name)?;
    let request_bytes = encode_request(method_desc.input(), request_json)?;
    let response_bytes = unary_call(channel, &service_name, &method_name, request_bytes).await?;
    decode_response(method_desc.output(), response_bytes)
}

fn parse_method(method: &str) -> GrpcResult<(String, String)> {
    let (service, method_name) = method.rsplit_once('/').ok_or_else(|| {
        GrpcError::Proto(format!(
            "invalid method path (expected 'Service/Method'): {method}"
        ))
    })?;
    Ok((service.to_string(), method_name.to_string()))
}

// TODO: cache channels by endpoint to avoid a full TCP+HTTP/2 handshake on every SQL call.
async fn connect(endpoint: &str) -> GrpcResult<Channel> {
    Channel::from_shared(format!("http://{endpoint}"))
        .map_err(|e| GrpcError::Connection(e.to_string()))?
        .connect()
        .await
        .map_err(|e| GrpcError::Connection(format!("{endpoint}: {e}")))
}

fn resolve_method<'a>(
    pool: &'a DescriptorPool,
    service_name: &str,
    method_name: &str,
) -> GrpcResult<MethodDescriptor> {
    let svc = pool
        .get_service_by_name(service_name)
        .ok_or_else(|| GrpcError::Proto(format!("service not found: {service_name}")))?;
    let method = svc
        .methods()
        .find(|m| m.name() == method_name)
        .ok_or_else(|| GrpcError::Proto(format!("method not found: {method_name}")))?;
    Ok(method)
}

fn encode_request(desc: MessageDescriptor, json: serde_json::Value) -> GrpcResult<Bytes> {
    let json_str = serde_json::to_string(&json)
        .map_err(|e| GrpcError::Proto(format!("serialize request: {e}")))?;
    let mut de = serde_json::Deserializer::from_str(&json_str);
    let msg: DynamicMessage = desc
        .deserialize(&mut de)
        .map_err(|e| GrpcError::Proto(format!("encode request: {e}")))?;
    Ok(Bytes::from(msg.encode_to_vec()))
}

async fn unary_call(
    channel: Channel,
    service_name: &str,
    method_name: &str,
    body: Bytes,
) -> GrpcResult<Bytes> {
    let path = format!("/{service_name}/{method_name}")
        .parse::<http::uri::PathAndQuery>()
        .map_err(|e| GrpcError::Proto(e.to_string()))?;

    let mut grpc = tonic::client::Grpc::new(channel);
    grpc.ready()
        .await
        .map_err(|e| GrpcError::Connection(e.to_string()))?;

    grpc.unary(tonic::Request::new(body), path, RawBytesCodec)
        .await
        .map(|r| r.into_inner())
        .map_err(|s| GrpcError::Call(s.to_string()))
}

fn decode_response(desc: MessageDescriptor, bytes: Bytes) -> GrpcResult<serde_json::Value> {
    let msg = DynamicMessage::decode(desc, bytes)
        .map_err(|e| GrpcError::Proto(format!("decode response: {e}")))?;

    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::new(&mut buf);
    // use_proto_field_name: emit snake_case `.proto` field names, not the default camelCase JSON names.
    msg.serialize_with_options(
        &mut ser,
        &SerializeOptions::new().use_proto_field_name(true),
    )
    .map_err(|e| GrpcError::Proto(format!("serialize response: {e}")))?;
    serde_json::from_slice(&buf)
        .map_err(|e| GrpcError::Proto(format!("parse serialized response: {e}")))
}

// Passes raw request/response bytes through without generated prost types,
// so we can drive gRPC calls from runtime-resolved schemas.
#[derive(Default)]
struct RawBytesCodec;
struct RawEncoder;
struct RawDecoder;

impl tonic::codec::Encoder for RawEncoder {
    type Item = Bytes;
    type Error = tonic::Status;

    fn encode(
        &mut self,
        item: Bytes,
        dst: &mut tonic::codec::EncodeBuf<'_>,
    ) -> Result<(), tonic::Status> {
        dst.put(item);
        Ok(())
    }
}

impl tonic::codec::Decoder for RawDecoder {
    type Item = Bytes;
    type Error = tonic::Status;

    fn decode(
        &mut self,
        src: &mut tonic::codec::DecodeBuf<'_>,
    ) -> Result<Option<Bytes>, tonic::Status> {
        let remaining = src.remaining();
        if remaining == 0 {
            return Ok(None);
        }
        Ok(Some(src.copy_to_bytes(remaining)))
    }
}

impl tonic::codec::Codec for RawBytesCodec {
    type Encode = Bytes;
    type Decode = Bytes;
    type Encoder = RawEncoder;
    type Decoder = RawDecoder;

    fn encoder(&mut self) -> RawEncoder {
        RawEncoder
    }
    fn decoder(&mut self) -> RawDecoder {
        RawDecoder
    }
}
