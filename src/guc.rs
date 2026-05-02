use pgrx::prelude::*;
use pgrx::{GucContext, GucFlags, GucRegistry, GucSetting};
use std::ffi::CString;

static DATABASE_NAME: GucSetting<Option<CString>> =
    GucSetting::<Option<CString>>::new(Some(c"postgres"));

static USERNAME: GucSetting<Option<CString>> = GucSetting::<Option<CString>>::new(None);

pub fn init() {
    GucRegistry::define_string_guc(
        c"pg_grpc.database_name",
        c"Database the async worker connects to",
        c"",
        &DATABASE_NAME,
        GucContext::Suset,
        GucFlags::default(),
    );
    GucRegistry::define_string_guc(
        c"pg_grpc.username",
        c"Role the async worker connects as (NULL = bootstrap superuser)",
        c"",
        &USERNAME,
        GucContext::Suset,
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
