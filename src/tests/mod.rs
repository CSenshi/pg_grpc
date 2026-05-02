#[pgrx::pg_schema]
mod tests {
    use pgrx::prelude::*;

    // Tests hit a real gRPC server that speaks the grpcbin dialect.
    // Defaults to the public grpcb.in instance for local runs; CI points this
    // at a containerized moul/grpcbin via GRPCBIN_ENDPOINT so the suite stays
    // hermetic and tolerant of grpcb.in downtime.
    fn grpcbin_endpoint() -> String {
        std::env::var("GRPCBIN_ENDPOINT").unwrap_or_else(|_| "grpcb.in:9000".to_string())
    }

    // TLS sibling of grpcbin_endpoint. grpcb.in exposes :9001 as the TLS port.
    // Allow override for CI where the containerized grpcbin may terminate TLS
    // on a different host/port.
    fn grpcbin_tls_endpoint() -> String {
        std::env::var("GRPCBIN_TLS_ENDPOINT").unwrap_or_else(|_| "grpcb.in:9001".to_string())
    }

    include!("call.rs");
    include!("channel_cache.rs");
    include!("compile.rs");
    include!("endpoint.rs");
    include!("list.rs");
    include!("options.rs");
    include!("registry.rs");
    include!("staging.rs");
    include!("tls.rs");
    include!("queue.rs");
    include!("worker.rs");
    include!("call_async.rs");
}
