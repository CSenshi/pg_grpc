#[pgrx::pg_schema]
mod tests {
    use pgrx::prelude::*;

    include!("call.rs");
    include!("compile.rs");
    include!("registry.rs");
    include!("staging.rs");
}
