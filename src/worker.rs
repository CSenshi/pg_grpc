use crate::{call, guc, queue, shmem};
use pgrx::bgworkers::*;
use pgrx::prelude::*;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[unsafe(no_mangle)]
#[pg_guard]
pub extern "C-unwind" fn grpc_async_worker(_arg: pg_sys::Datum) {
    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    let db = guc::database_name();
    let user = guc::username();
    BackgroundWorker::connect_worker_to_spi(Some(&db), user.as_deref());

    unsafe { shmem::store_latch(pg_sys::MyLatch) };
    shmem::set_status(shmem::STATUS_RUNNING);

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

    shmem::set_status(shmem::STATUS_EXITED);
}
