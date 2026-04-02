//! Rust SDK for [BitGN](https://bitgn.com/) Agent Benchmarks & Challenges.
//!
//! Typed Connect-RPC client generated from [buf.build/bitgn/api](https://buf.build/bitgn/api)
//! using [connect-rust](https://github.com/anthropics/connect-rust) by Anthropic.
//!
//! # Services
//!
//! - [`harness::HarnessServiceClient`] — Control plane: benchmarks, runs, trials
//! - [`vm::pcm::PcmRuntimeClient`] — PAC1 agent runtime: file ops, search, answer
//! - [`vm::mini::MiniRuntimeClient`] — Mini/demo agent runtime
//!
//! # Example
//!
//! ```rust,ignore
//! use bitgn_sdk::{make_client_config, make_http_client};
//! use bitgn_sdk::harness::{HarnessServiceClient, GetBenchmarkRequest};
//!
//! let http = make_http_client("https://api.bitgn.com");
//! let config = make_client_config("https://api.bitgn.com", None);
//! let client = HarnessServiceClient::new(http, config);
//! ```

// Generated proto code
include!(concat!(env!("OUT_DIR"), "/_all.rs"));

// Re-export for convenience
pub use bitgn::harness;
pub use bitgn::vm;
pub use connectrpc;

/// Create an HTTP client with automatic TLS detection.
/// Uses `plaintext` for http://, `with_tls` (ring + webpki-roots) for https://.
pub fn make_http_client(url: &str) -> connectrpc::client::HttpClient {
    if url.starts_with("https://") {
        let _ = connectrpc::rustls::crypto::ring::default_provider().install_default();
        let roots = connectrpc::rustls::RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned()
        );
        let tls = connectrpc::rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        connectrpc::client::HttpClient::with_tls(std::sync::Arc::new(tls))
    } else {
        connectrpc::client::HttpClient::plaintext()
    }
}

/// Create a ClientConfig with optional Bearer auth header.
pub fn make_client_config(url: &str, api_key: Option<&str>) -> connectrpc::client::ClientConfig {
    let parsed = url.trim_end_matches('/').parse().expect("invalid URL");
    let mut config = connectrpc::client::ClientConfig::new(parsed);
    if let Some(key) = api_key {
        config = config.default_header(
            http::header::AUTHORIZATION,
            format!("Bearer {}", key).parse::<http::header::HeaderValue>().unwrap(),
        );
    }
    config
}
