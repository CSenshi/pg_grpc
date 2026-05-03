use pgrx::{GucContext, GucFlags, GucRegistry, GucSetting};
use std::ffi::CString;

static DATABASE_NAME: GucSetting<Option<CString>> =
    GucSetting::<Option<CString>>::new(Some(c"postgres"));

static USERNAME: GucSetting<Option<CString>> = GucSetting::<Option<CString>>::new(None);

static TTL: GucSetting<Option<CString>> = GucSetting::<Option<CString>>::new(Some(c"6 hours"));

static BATCH_SIZE: GucSetting<i32> = GucSetting::<i32>::new(200);

pub fn init() {
    GucRegistry::define_string_guc(
        c"pg_grpc.database_name",
        c"Database the async worker connects to",
        c"",
        &DATABASE_NAME,
        GucContext::Postmaster,
        GucFlags::default(),
    );
    GucRegistry::define_string_guc(
        c"pg_grpc.username",
        c"Role the async worker connects as (NULL = bootstrap superuser)",
        c"",
        &USERNAME,
        GucContext::Postmaster,
        GucFlags::default(),
    );
    GucRegistry::define_string_guc(
        c"pg_grpc.ttl",
        c"How long to retain async call results before TTL cleanup",
        c"",
        &TTL,
        GucContext::Sighup,
        GucFlags::default(),
    );
    GucRegistry::define_int_guc(
        c"pg_grpc.batch_size",
        c"Number of async calls to dequeue per worker cycle",
        c"",
        &BATCH_SIZE,
        1,
        i32::MAX,
        GucContext::Sighup,
        GucFlags::default(),
    );
}

pub fn database_name() -> String {
    DATABASE_NAME
        .get()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "postgres".to_string())
}

pub fn username() -> Option<String> {
    USERNAME
        .get()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
}

pub fn ttl() -> String {
    TTL.get()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "6 hours".to_string())
}

pub fn batch_size() -> i32 {
    BATCH_SIZE.get()
}
