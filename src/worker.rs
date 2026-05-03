use crate::{call, guc, queue, shmem};
use pgrx::bgworkers::*;
use pgrx::prelude::*;
use std::sync::atomic::Ordering;
use std::time::Duration;

// Registered via on_proc_exit so it runs even on panic/ereport(ERROR), clearing the
// latch pointer before this process's PGPROC slot is recycled.
unsafe extern "C-unwind" fn on_worker_exit(_code: std::os::raw::c_int, _arg: pg_sys::Datum) {
    // Mark wake pending so the restarted worker drains any rows that were in-flight
    // when this process died, rather than waiting up to 1 second for the next timeout.
    shmem::should_wake().store(true, std::sync::atomic::Ordering::Release);
    shmem::clear_latch();
}

#[unsafe(no_mangle)]
#[pg_guard]
pub extern "C-unwind" fn grpc_async_worker(_arg: pg_sys::Datum) {
    // Zero any stale pointer left by a previous incarnation (handles SIGKILL where
    // on_proc_exit did not run).  Do this before storing MyLatch so callers that race
    // the startup window see 0 and no-op rather than dereferencing a dead pointer.
    shmem::clear_latch();

    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    let db = guc::database_name();
    let user = guc::username();
    BackgroundWorker::connect_worker_to_spi(Some(&db), user.as_deref());

    // Register the exit hook before publishing the latch so any subsequent
    // crash or clean exit always zeros the pointer.
    unsafe { pg_sys::on_proc_exit(Some(on_worker_exit), pg_sys::Datum::null()) };

    unsafe { shmem::store_latch(pg_sys::MyLatch) };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime for async worker");

    while BackgroundWorker::wait_latch(Some(Duration::from_secs(1))) {
        if BackgroundWorker::sighup_received() {
            unsafe { pg_sys::ProcessConfigFile(pg_sys::GucContext::PGC_SIGHUP) };
        }

        // CAS should_wake true→false; skip batch if nobody signaled since last cycle.
        if shmem::should_wake()
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            continue;
        }

        // Transaction 1: dequeue up to batch_size pending rows.
        let batch_size = guc::batch_size();
        let rows = queue::dequeue(batch_size);
        if rows.is_empty() {
            continue;
        }

        // Execute all gRPC calls concurrently on the single-threaded runtime.
        let results: Vec<queue::CallResult> = rt.block_on(async {
            let mut set = tokio::task::JoinSet::new();
            for row in rows {
                set.spawn(call::call_async_row(row));
            }
            let mut out = Vec::new();
            while let Some(res) = set.join_next().await {
                if let Ok(r) = res {
                    out.push(r);
                }
            }
            out
        });

        // Transaction 2: persist results and remove processed rows; also TTL cleanup.
        queue::insert_results(results);
        queue::ttl_cleanup(&guc::ttl());
    }
}
