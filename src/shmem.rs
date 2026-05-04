use pgrx::prelude::*;
use pgrx::{pg_shmem_init, PgAtomic, PgLwLock};
use std::sync::atomic::AtomicBool;

static SHOULD_WAKE: PgAtomic<AtomicBool> = unsafe { PgAtomic::new(c"pg_grpc_should_wake") };
// Latch pointer stored as u64 (raw pointer cast)
static WORKER_LATCH: PgLwLock<u64> = unsafe { PgLwLock::new(c"pg_grpc_worker_latch") };

pub fn init() {
    pg_shmem_init!(SHOULD_WAKE);
    pg_shmem_init!(WORKER_LATCH);
}

pub fn should_wake() -> &'static AtomicBool {
    SHOULD_WAKE.get()
}

pub fn store_latch(latch: *mut pg_sys::Latch) {
    *WORKER_LATCH.exclusive() = latch as u64;
}

pub fn clear_latch() {
    *WORKER_LATCH.exclusive() = 0;
}

pub fn set_latch() {
    let ptr = *WORKER_LATCH.share();
    if ptr != 0 {
        unsafe { pg_sys::SetLatch(ptr as *mut pg_sys::Latch) };
    }
}
