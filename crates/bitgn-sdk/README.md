# bitgn-sdk

Unofficial Rust SDK for [BitGN](https://bitgn.com/) Agent Benchmarks & Challenges platform.

Typed Connect-RPC client generated from the official [buf.build/bitgn/api](https://buf.build/bitgn/api) proto definitions using [connect-rust](https://github.com/anthropics/connect-rust) by Anthropic.

## About BitGN

[BitGN](https://bitgn.com/) is a platform for Agent Benchmarks & Challenges, built by Rinat Abdullin and Ksenia Makarova in Vienna. The [PAC1 Challenge](https://bitgn.com/) evaluates personal & trustworthy autonomous agents on CRM tasks with security traps.

This SDK was built for the PAC1 competition (April 2026) and covers the full platform API.

## Services

| Service | Description |
|---------|------------|
| `harness::HarnessServiceClient` | Control plane: list benchmarks, start runs/trials, get scores |
| `vm::pcm::PcmRuntimeClient` | PAC1 agent runtime: read/write files, search, delete, answer |
| `vm::mini::MiniRuntimeClient` | Mini/demo agent runtime (simpler API surface) |

## Quick Start

```toml
[dependencies]
bitgn-sdk = "0.1"
connectrpc = { version = "0.3", features = ["client"] }
tokio = { version = "1", features = ["full"] }
```

```rust
use bitgn_sdk::harness::{HarnessServiceClient, GetBenchmarkRequest};
use connectrpc::client::{HttpClient, ClientConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tls = connectrpc::rustls::ClientConfig::builder()
        .with_native_roots().unwrap()
        .with_no_client_auth();
    let http = HttpClient::with_tls(std::sync::Arc::new(tls));
    let config = ClientConfig::new("https://api.bitgn.com".parse()?);
    let client = HarnessServiceClient::new(http, config);

    let bench = client.get_benchmark(GetBenchmarkRequest {
        benchmark_id: "bitgn/pac1-dev".into(),
        ..Default::default()
    }).await?;
    println!("{} tasks", bench.tasks.len());
    Ok(())
}
```

## How It Works

```
buf.build/bitgn/api (proto source)
        |
  connectrpc-build (build.rs)
        |
  Typed Rust client (21K lines generated)
        |
  Connect-RPC / JSON over HTTP
        |
  BitGN Platform API
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
