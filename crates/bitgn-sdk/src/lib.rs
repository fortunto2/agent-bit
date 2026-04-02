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
//! use bitgn_sdk::harness::{HarnessServiceClient, GetBenchmarkRequest};
//! use connectrpc::client::{HttpClient, ClientConfig};
//!
//! let http = HttpClient::plaintext();
//! let config = ClientConfig::new("https://api.bitgn.com".parse().unwrap());
//! let client = HarnessServiceClient::new(http, config);
//! let bench = client.get_benchmark(GetBenchmarkRequest {
//!     benchmark_id: "bitgn/pac1-dev".into(),
//!     ..Default::default()
//! }).await.unwrap();
//! ```

// Generated proto code
include!(concat!(env!("OUT_DIR"), "/_all.rs"));

// Re-export for convenience
pub use bitgn::harness;
pub use bitgn::vm;

/// Re-export connectrpc client types.
pub mod client {
    pub use connectrpc::client::{ClientConfig, HttpClient};
}
