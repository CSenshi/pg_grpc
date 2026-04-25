use pgrx::prelude::*;

::pgrx::pg_module_magic!(name, version);

mod call;
mod channel_cache;
mod endpoint;
mod error;
mod options;
mod proto;
mod proto_registry;
mod proto_staging;
mod tls;

#[pg_guard]
pub extern "C-unwind" fn _PG_init() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

use crate::error::{GrpcError, GrpcResult};

#[pg_extern]
fn grpc_call(
    endpoint: &str,
    method: &str,
    request: pgrx::JsonB,
    metadata: default!(Option<pgrx::JsonB>, "null"),
    options: default!(Option<pgrx::JsonB>, "null"),
) -> pgrx::JsonB {
    let opts = match options {
        None => options::OptionsConfig::default(),
        Some(pgrx::JsonB(v)) => match options::OptionsConfig::parse(&v) {
            Ok(c) => c,
            Err(e) => pgrx::error!("{}", e),
        },
    };
    match call::make_grpc_call(
        endpoint,
        method,
        request.0,
        opts.use_reflection.unwrap_or(true),
        metadata.map(|j| j.0),
        opts.timeout_ms.unwrap_or(30_000),
        opts.tls,
        opts.max_decode_message_size_bytes,
        opts.max_encode_message_size_bytes,
    ) {
        Ok(value) => pgrx::JsonB(value),
        Err(e) => pgrx::error!("{}", e),
    }
}

#[pg_extern]
fn grpc_proto_stage(filename: &str, source: &str) {
    match validate_stage_input(filename, source) {
        Ok(()) => proto_staging::stage_file(filename, source),
        Err(e) => pgrx::error!("{}", e),
    }
}

pub(crate) fn validate_stage_input(filename: &str, source: &str) -> GrpcResult<()> {
    if filename.trim().is_empty() {
        return Err(GrpcError::ProtoCompile(
            "grpc_proto_stage: filename must not be empty".to_string(),
        ));
    }
    if source.trim().is_empty() {
        return Err(GrpcError::ProtoCompile(
            "grpc_proto_stage: source must not be empty".to_string(),
        ));
    }
    Ok(())
}

#[pg_extern]
fn grpc_proto_unstage(filename: &str) -> bool {
    proto_staging::remove(filename)
}

#[pg_extern]
fn grpc_proto_unstage_all() {
    proto_staging::clear();
}

#[pg_extern]
fn grpc_proto_compile() {
    let staged = proto_staging::snapshot();
    match proto::compile_proto_files(staged.clone()) {
        Ok(pool) => {
            for svc in pool.services() {
                // protox uses our filenames verbatim, so parent_file().name() keys back into the snapshot.
                let filename = svc.parent_file().name().to_owned();
                let source = staged.get(&filename).cloned().unwrap_or_default();
                proto_registry::insert_proto_manual(
                    svc.full_name(),
                    pool.clone(),
                    filename,
                    source,
                );
            }
            proto_staging::clear();
        }
        Err(e) => pgrx::error!("{}", e),
    }
}

#[pg_extern]
fn grpc_proto_unregister(service_name: &str) -> bool {
    proto_registry::remove(service_name)
}

#[pg_extern]
fn grpc_proto_unregister_all() {
    proto_registry::clear();
}

#[pg_extern]
fn grpc_proto_list_staged(
) -> TableIterator<'static, (name!(filename, String), name!(source, String))> {
    TableIterator::new(proto_staging::list())
}

#[pg_extern]
#[allow(clippy::type_complexity)]
fn grpc_proto_list_registered() -> TableIterator<
    'static,
    (
        name!(service_name, String),
        name!(origin, String),
        name!(filename, Option<String>),
        name!(source, Option<String>),
        name!(endpoint, Option<String>),
    ),
> {
    let rows = proto_registry::list()
        .into_iter()
        .map(|(service_name, origin)| match origin {
            proto_registry::Origin::UserStaged { filename, source } => (
                service_name,
                "user".to_string(),
                Some(filename),
                Some(source),
                None,
            ),
            proto_registry::Origin::Reflection { endpoint } => (
                service_name,
                "reflection".to_string(),
                None,
                None,
                Some(endpoint),
            ),
        });
    TableIterator::new(rows)
}

#[cfg(any(test, feature = "pg_test"))]
mod tests;

// Required by `cargo pgrx test`.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {}

    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        vec![]
    }
}
