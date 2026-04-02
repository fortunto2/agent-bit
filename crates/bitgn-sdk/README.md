# bitgn-sdk

[![Crates.io](https://img.shields.io/crates/v/bitgn-sdk.svg)](https://crates.io/crates/bitgn-sdk)
[![docs.rs](https://docs.rs/bitgn-sdk/badge.svg)](https://docs.rs/bitgn-sdk)

Unofficial Rust SDK for [BitGN](https://bitgn.com/) Agent Benchmarks & Challenges platform.

Typed Connect-RPC client generated from the official [buf.build/bitgn/api](https://buf.build/bitgn/api) proto definitions using [connect-rust](https://github.com/anthropics/connect-rust) by Anthropic.

## About BitGN

[BitGN](https://bitgn.com/) is a platform for Agent Benchmarks & Challenges, built by Rinat Abdullin and Ksenia Makarova in Vienna. The [PAC1 Challenge](https://bitgn.com/) evaluates personal & trustworthy autonomous agents on CRM tasks with security traps.

This SDK was built for the PAC1 competition (April 2026) and covers the full platform API.

## Install

```bash
cargo add bitgn-sdk
```

Or add to `Cargo.toml`:

```toml
[dependencies]
bitgn-sdk = "0.1"
tokio = { version = "1", features = ["full"] }
```

TLS is included by default (BitGN API is https-only). No extra features needed.

## Services

| Service | Description |
|---------|------------|
| `harness::HarnessServiceClient` | Control plane: list benchmarks, start runs/trials, get scores |
| `vm::pcm::PcmRuntimeClient` | PAC1 agent runtime: read/write files, search, delete, answer |
| `vm::mini::MiniRuntimeClient` | Mini/demo agent runtime (simpler API surface) |

## Quick Start

Add dependencies:

```toml
[dependencies]
bitgn-sdk = "0.1"
webpki-roots = "0.26"
tokio = { version = "1", features = ["full"] }
```

Create a TLS client and call the API:

```rust
use bitgn_sdk::harness::{HarnessServiceClient, GetBenchmarkRequest};
use connectrpc::client::{HttpClient, ClientConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Install crypto provider (required by rustls 0.23)
    let _ = connectrpc::rustls::crypto::ring::default_provider().install_default();

    // Build TLS client with root certificates
    let roots = connectrpc::rustls::RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned()
    );
    let tls = connectrpc::rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let http = HttpClient::with_tls(Arc::new(tls));

    // Connect to BitGN API
    let config = ClientConfig::new("https://api.bitgn.com".parse()?);
    let client = HarnessServiceClient::new(http, config);

    // List benchmark tasks
    let bench = client.get_benchmark(GetBenchmarkRequest {
        benchmark_id: "bitgn/pac1-dev".into(),
        ..Default::default()
    }).await?;
    println!("{} tasks", bench.view().tasks.len());
    Ok(())
}
```

## PAC1 Agent Example

```rust
use bitgn_sdk::vm::pcm::*;

// After starting a trial, use the harness_url for PCM runtime
let pcm = PcmRuntimeClient::new(http, pcm_config);

// Read a file
let file = pcm.read(ReadRequest {
    path: "contacts/john.json".into(),
    ..Default::default()
}).await?;
println!("{}", file.view().content);

// Search with regex
let results = pcm.search(SearchRequest {
    pattern: "Smith".into(),
    root: "contacts".into(),
    ..Default::default()
}).await?;
for m in &results.view().matches {
    println!("{}:{}: {}", m.path, m.line, m.line_text);
}

// Submit answer
pcm.answer(AnswerRequest {
    message: "Found contact John Smith".into(),
    outcome: Outcome::OUTCOME_OK.into(),
    refs: vec!["contacts/john.json".into()],
    ..Default::default()
}).await?;
```

## How It Works

Proto files from [buf.build/bitgn/api](https://buf.build/bitgn/api) are compiled at build time via `build.rs` using `connectrpc-build`. This generates typed Rust structs and async client methods for all RPC services.

```
buf.build/bitgn/api (proto source)
        |
  connectrpc-build (build.rs)
        |
  Typed Rust client (21K lines)
        |
  Connect-RPC / JSON over HTTPS
        |
  BitGN Platform API
```

## Wire Protocol

Default is **Connect protocol with JSON** encoding — human-readable, easy to debug, works with `curl`. The BitGN server supports three wire formats via the same proto definitions:

| Protocol | Format | When to use |
|----------|--------|-------------|
| Connect (default) | JSON | Development, debugging |
| Connect | Protobuf | Production (~30% smaller payload) |
| gRPC | Protobuf | If you need gRPC ecosystem tooling |

Switch encoding:
```rust
// JSON (default — human-readable, good for debugging)
let config = ClientConfig::new(uri);

// Protobuf binary (smaller, faster)
let config = ClientConfig::new(uri).proto();

// gRPC protocol (HTTP/2 + protobuf)
use connectrpc::client::Protocol;
let config = ClientConfig::new(uri).protocol(Protocol::Grpc).proto();
```

Pre-generated code is available in [`src/generated/`](src/generated/) for browsing without building.

## Regenerating from Proto

If the BitGN API updates, refresh the proto files and rebuild:

```bash
# Download latest proto definitions
curl -sL "https://buf.build/bitgn/api/archive/main.tar.gz" | tar xz -C proto/

# Rebuild (build.rs runs connectrpc-build automatically)
cargo build

# Generated code appears in target/debug/build/bitgn-sdk-*/out/
# Copy to src/generated/ for browsing:
cp target/debug/build/bitgn-sdk-*/out/*.rs src/generated/
```

## Related Crates

Part of a Rust agent stack for the PAC1 challenge:

- [sgr-agent](https://github.com/fortunto2/rust-code/tree/master/crates/sgr-agent) -- LLM agent framework with structured output, function calling, and agent loop
- [openai-oxide](https://crates.io/crates/openai-oxide) -- Rust OpenAI client with Responses API and structured output
- **bitgn-sdk** (this crate) -- typed BitGN platform client

## Proto Source

Official definitions from [buf.build/bitgn/api](https://buf.build/bitgn/api). Generated SDKs for other languages at [buf.build/bitgn/api/sdks](https://buf.build/bitgn/api/sdks). This is the first Rust SDK.

## License

MIT
