use pgrx::bgworkers::*;
use pgrx::prelude::*;
use std::time::Duration;

#[unsafe(no_mangle)]
#[pg_guard]
pub extern "C-unwind" fn grpc_async_worker(_arg: pg_sys::Datum) {
    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    let db = crate::guc::database_name();
    let user = crate::guc::username();
    BackgroundWorker::connect_worker_to_spi(Some(&db), user.as_deref());

    unsafe { crate::shmem::store_latch(pg_sys::MyLatch) };
    crate::shmem::set_status(crate::shmem::STATUS_RUNNING);

    while BackgroundWorker::worker_continue() {
        BackgroundWorker::wait_latch(Some(Duration::from_secs(1)));

        if BackgroundWorker::sighup_received() {
            unsafe { pg_sys::ProcessConfigFile(pg_sys::GucContext::PGC_SIGHUP) };
        }
    }

    crate::shmem::set_status(crate::shmem::STATUS_EXITED);
}
