#[pgrx::pg_schema]
mod tests {
    use pgrx::prelude::*;

    include!("call.rs");
    include!("staging.rs");
    include!("registry.rs");
}
