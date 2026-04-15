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
/// without reflection. Each registered service is stored alongside the
/// source text of the `.proto` file that originally defined it, so
/// `grpc_proto_list_registered` can echo the source back to callers.
///
/// On success, the staging area is cleared. On failure, staged files are
/// left intact so the caller can fix the offending file and retry.
#[pg_extern]
fn grpc_proto_compile() {
    let staged = proto_staging::snapshot();
    match proto::compile_proto_files(staged.clone()) {
        Ok(pool) => {
            for svc in pool.services() {
                // Recover the filename + source of the file that defined
                // this service. protox uses our filenames verbatim, so the
                // parent file name is a key into the pre-compile snapshot.
                let filename = svc.parent_file().name().to_owned();
                let source = staged.get(&filename).cloned().unwrap_or_default();
                proto_registry::insert_proto(svc.full_name(), pool.clone(), filename, source);
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

/// Lists every currently-staged `.proto` file along with its source text.
/// Order is unspecified.
///
/// ```sql
/// SELECT filename FROM grpc_proto_list_staged();
/// SELECT * FROM grpc_proto_list_staged() WHERE filename = 'common.proto';
/// ```
#[pg_extern]
fn grpc_proto_list_staged(
) -> TableIterator<'static, (name!(filename, String), name!(source, String))> {
    TableIterator::new(proto_staging::list().into_iter())
}

/// Lists every registered service together with the `.proto` filename and
/// source that defined it. One row per service — a file with multiple
/// services produces multiple rows sharing the same filename/source. Files
/// that contribute only messages/types via `import` do not appear. Order
/// is unspecified.
///
/// ```sql
/// -- Everything that can currently be called
/// SELECT service_name FROM grpc_proto_list_registered();
///
/// -- Unique file inventory (staged-style view)
/// SELECT DISTINCT filename, source FROM grpc_proto_list_registered();
///
/// -- Services defined by a specific file
/// SELECT service_name FROM grpc_proto_list_registered() WHERE filename = 'auth.proto';
/// ```
#[pg_extern]
fn grpc_proto_list_registered() -> TableIterator<
    'static,
    (
        name!(service_name, String),
        name!(filename, String),
        name!(source, String),
    ),
> {
    TableIterator::new(proto_registry::list().into_iter())
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
