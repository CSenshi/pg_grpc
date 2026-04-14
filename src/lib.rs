use pgrx::prelude::*;

::pgrx::pg_module_magic!(name, version);

mod call;
mod error;
mod proto;
mod proto_registry;
mod proto_staging;

#[pg_extern]
fn grpc_call(
    endpoint: &str,
    method: &str,
    request: pgrx::JsonB,
    _timeout_ms: default!(Option<i64>, "null"),
) -> pgrx::JsonB {
    match call::make_grpc_call(endpoint, method, request.0) {
        Ok(value) => pgrx::JsonB(value),
        Err(e) => pgrx::error!("{}", e),
    }
}

/// Stages a `.proto` file under `filename` for the next `grpc_proto_compile`
/// call. Nothing is parsed or validated until compile time. Re-registering
/// the same filename overwrites the previous content.
///
/// ```sql
/// SELECT grpc_proto_stage('common.proto', $$ syntax = "proto3"; ... $$);
/// SELECT grpc_proto_stage('service.proto', $$ import "common.proto"; ... $$);
/// SELECT grpc_proto_compile();
/// ```
#[pg_extern]
fn grpc_proto_stage(filename: &str, source: &str) {
    proto_staging::stage_file(filename, source);
}

/// Removes one file from the staging area by filename. Returns `true` if an
/// entry was removed, `false` if no matching file was staged. The registry of
/// already-compiled services is untouched — use this to recover from a bad
/// staged file without disturbing in-flight `grpc_call`s.
#[pg_extern]
fn grpc_proto_unstage(filename: &str) -> bool {
    proto_staging::remove(filename)
}

/// Clears every file from the staging area. The registry of already-compiled
/// services is untouched, so in-flight `grpc_call`s are unaffected.
#[pg_extern]
fn grpc_proto_unstage_all() {
    proto_staging::clear();
}

/// Compiles every file previously staged via `grpc_proto_stage`, resolving
/// imports between them (and against the Google Well-Known Types), then
/// registers every service discovered so `grpc_call` can target servers
/// without reflection.
///
/// On success, the staging area is cleared. On failure, staged files are
/// left intact so the caller can fix the offending file and retry.
#[pg_extern]
fn grpc_proto_compile() {
    let staged = proto_staging::snapshot();
    match proto::compile_proto_files(staged) {
        Ok(pool) => {
            for svc in pool.services() {
                proto_registry::insert_proto(svc.full_name(), pool.clone());
            }
            proto_staging::clear();
        }
        Err(e) => pgrx::error!("{}", e),
    }
}

/// Removes a single registered service by its fully-qualified name (e.g.
/// `"pkg.Service"`). Returns `true` if an entry was removed, `false` if no
/// matching service was registered. Does not touch the staging area.
#[pg_extern]
fn grpc_proto_unregister(service_name: &str) -> bool {
    proto_registry::remove(service_name)
}

/// Clears every registered service. Does not touch the staging area.
#[pg_extern]
fn grpc_proto_unregister_all() {
    proto_registry::clear();
}

#[cfg(any(test, feature = "pg_test"))]
mod tests;

/// This module is required by `cargo pgrx test` invocations.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {}

    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        vec![]
    }
}
