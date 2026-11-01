# `m31a-provider`

Dedicated NVIDIA NIM LLM abstraction layer, Server-Sent Events (SSE) streaming, reasoning trace parser, and model router for **M31A**.

---

## Overview

`m31a-provider` is the inference communication engine of M31A. It connects to **NVIDIA NIM** endpoints (`https://integrate.api.nvidia.com/v1`), dynamically queries `/v1/models`, streams chat completions via SSE, parses DeepSeek R1 / Nemotron thinking paths in real time, handles rate limits with exponential backoff retries, and extracts function/tool calls.

---

## User-Facing Capabilities

- **Frontier Intelligence**: Connects natively to high-performance models hosted on [build.nvidia.com](https://build.nvidia.com/):
  - `nvidia/nemotron-3.5-lightning-30b-a3b`
  - `nvidia/nemotron-3.5-bolt-e4b`
  - `deepseek-ai/deepseek-v4.1-flash`
  - `mistralai/codestral-22b-instruct-v0.1`
  - `nvidia/llama-3.1-nemotron-70b-instruct`
- **Dynamic Model Discovery (`m31a model list`)**: Queries the live NIM endpoint to discover available models, context window limits, and capabilities.
- **Thinking Stream Visualization**: Separates raw reasoning thought tokens from user-facing markdown text.

---

## Developer Guide & Architecture

### Key Modules

- [`client`](src/client.rs): High-throughput asynchronous HTTP/SSE streaming client.
- [`models`](src/models.rs): Canonical model definitions, context window limits, and capability tags (`supports_tools`, `supports_reasoning`).
- [`retry`](src/retry.rs): Robust exponential backoff with jitter for HTTP 429 and transient 5xx errors.
- [`router`](src/router.rs): Dynamically routes requests based on task type (e.g. routing fast vibe edits to `bolt-e4b` and deep architectural planning to `deepseek-v4.1-flash`).

### Programmatic Usage

```rust
use m31a_provider::{ProviderBuilder, ProviderConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ProviderConfig {
        api_key: std::env::var("NVIDIA_API_KEY")?,
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".into(),
        ..Default::default()
    };
    let provider = ProviderBuilder::new(config).build()?;
    println!("Provider initialized: {}", provider.model_name());
    Ok(())
}
```

---

## Testing

```bash
# Standard tests
cargo test -p m31a-provider

# Live NVIDIA NIM integration tests (requires NVIDIA_API_KEY)
export NVIDIA_API_KEY="nvapi-..."
cargo test -p m31a-provider --features live-nvidia-tests --test live_provider_test -- --ignored
```
