use pgrx::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

::pgrx::pg_module_magic!(name, version);

// True while the current transaction has at least one enqueued call.
static WAKE_CB_ACTIVE: AtomicBool = AtomicBool::new(false);

mod async_schema;
mod call;
mod channel_cache;
mod endpoint;
mod error;
mod guc;
mod options;
mod proto;
mod proto_registry;
mod proto_staging;
mod queue;
mod shmem;
mod tls;
mod worker;

#[pg_guard]
pub extern "C-unwind" fn _PG_init() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    guc::init();
    shmem::init();
    unsafe {
        pg_sys::RegisterXactCallback(Some(wake_worker_on_commit), std::ptr::null_mut());
    }
    use pgrx::bgworkers::*;
    use std::time::Duration;
    BackgroundWorkerBuilder::new("pg_grpc async worker")
        .set_function("grpc_async_worker")
        .set_library("pg_grpc")
        .enable_shmem_access(None)
        .enable_spi_access()
        .set_start_time(BgWorkerStartTime::RecoveryFinished)
        .set_restart_time(Some(Duration::from_secs(1)))
        .load();
}

#[pg_extern]
fn grpc_call_async(
    endpoint: &str,
    method: &str,
    request: pgrx::JsonB,
    metadata: default!(Option<pgrx::JsonB>, "null"),
    options: default!(Option<pgrx::JsonB>, "null"),
) -> i64 {
    // Validate endpoint and method at enqueue time — same rules as the sync path.
    if let Err(e) = endpoint::validate_endpoint(endpoint) {
        pgrx::error!("{}", e);
    }
    if let Err(e) = call::parse_method(method) {
        pgrx::error!("{}", e);
    }

    // Validate options at enqueue time — same rules as the sync path.
    let opts = match &options {
        None => options::OptionsConfig::default(),
        Some(pgrx::JsonB(v)) => match options::OptionsConfig::parse(v) {
            Ok(c) => c,
            Err(e) => pgrx::error!("{}", e),
        },
    };
    let timeout_raw = opts.timeout_ms.unwrap_or(30_000);
    if timeout_raw > i32::MAX as u64 {
        pgrx::error!("options.timeout_ms must be <= {}", i32::MAX);
    }
    let timeout_ms = timeout_raw as i32;

    let id = Spi::connect_mut(|client| {
        client
            .update(
                "INSERT INTO grpc.call_queue
                     (endpoint, method, request, metadata, options, timeout_ms)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 RETURNING id",
                None,
                &[
                    endpoint.into(),
                    method.into(),
                    request.into(),
                    metadata.into(),
                    options.into(),
                    timeout_ms.into(),
                ],
            )
            .unwrap()
            .first()
            .get_one::<i64>()
            .unwrap()
            .unwrap()
    });

    WAKE_CB_ACTIVE.store(true, Ordering::Relaxed);

    id
}

/// Called by PostgreSQL at transaction commit/abort.  Only signals the worker
/// on a real commit so rolled-back enqueues never disturb it.
unsafe extern "C-unwind" fn wake_worker_on_commit(
    event: pg_sys::XactEvent::Type,
    _arg: *mut std::ffi::c_void,
) {
    if event == pg_sys::XactEvent::XACT_EVENT_COMMIT
        || event == pg_sys::XactEvent::XACT_EVENT_PARALLEL_COMMIT
    {
        // swap returns the old value: only the one callback that flips true→false
        // signals the worker.  Accumulated stale nodes from previous transactions
        // see false and do nothing — matching pg_net's inner wake_commit_cb_active guard.
        if WAKE_CB_ACTIVE.swap(false, Ordering::Relaxed) {
            shmem::should_wake().store(true, Ordering::Release);
            shmem::set_latch();
        }
    } else if event == pg_sys::XactEvent::XACT_EVENT_ABORT
        || event == pg_sys::XactEvent::XACT_EVENT_PARALLEL_ABORT
    {
        WAKE_CB_ACTIVE.store(false, Ordering::Relaxed);
    }
}

use crate::error::{GrpcError, GrpcResult};

#[pg_extern]
fn grpc_call_result(
    id: i64,
    r#async: default!(bool, true),
) -> TableIterator<
    'static,
    (
        name!(id, i64),
        name!(status, String),
        name!(message, Option<String>),
        name!(response, Option<pgrx::JsonB>),
    ),
> {
    let result = if !r#async {
        loop {
            let r = queue::lookup(id);
            match r.status {
                queue::LookupStatus::Pending => unsafe {
                    pg_sys::WaitLatch(
                        pg_sys::MyLatch,
                        (pg_sys::WL_LATCH_SET | pg_sys::WL_TIMEOUT | pg_sys::WL_EXIT_ON_PM_DEATH)
                            as i32,
                        50,
                        pg_sys::PG_WAIT_EXTENSION,
                    );
                    pg_sys::ResetLatch(pg_sys::MyLatch);
                    pg_sys::check_for_interrupts!();
                },
                _ => break r,
            }
        }
    } else {
        queue::lookup(id)
    };

    let (status, message, response) = match result.status {
        queue::LookupStatus::Pending => ("PENDING".to_string(), None, None),
        queue::LookupStatus::Success(v) => ("SUCCESS".to_string(), None, Some(pgrx::JsonB(v))),
        queue::LookupStatus::Error(msg) => ("ERROR".to_string(), Some(msg), None),
    };

    TableIterator::new(vec![(result.id, status, message, response)])
}

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
        vec![
            "shared_preload_libraries = 'pg_grpc'",
            "pg_grpc.database_name = 'pgrx_tests'",
        ]
    }
}
