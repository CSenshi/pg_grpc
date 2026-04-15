#[pgrx::pg_schema]
mod tests {
    use pgrx::prelude::*;

    include!("call.rs");
    include!("compile.rs");
    include!("list.rs");
    include!("registry.rs");
    include!("staging.rs");
}
