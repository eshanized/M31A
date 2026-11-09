# `m31a-provider`

> Dedicated NVIDIA NIM frontier intelligence client, Server-Sent Events (SSE) streaming engine, DeepSeek R1 reasoning trace parser, and dynamic model router for M31A.

---

## Why Does This Crate Exist?

At the heart of any AI coding assistant is the inference pipeline. In M31A, we chose to anchor our intelligence on **NVIDIA NIM** microservices (`https://integrate.api.nvidia.com/v1`) because they deliver unmatched throughput, low latency, and open-weights sovereignty.

However, communicating with frontier models in an autonomous agent requires far more than simple `POST /chat/completions` requests:
- DeepSeek R1 and reasoning models output internal `<think>` reasoning traces that need to be parsed in real time without polluting user-facing markdown text.
- Tool calls arrive incrementally in fragmented JSON chunks across Server-Sent Events (SSE) streams and must be reconstructed into valid schema invocations.
- Rate limits (HTTP 429) and transient network hiccups must be handled with randomized exponential backoff and jitter.
- The system must dynamically discover available models via `/v1/models` so users can swap model slugs without recompiling M31A.

`m31a-provider` encapsulates this entire pipeline behind clean, async Rust traits.

---

## Core Concepts & How It Works

### 1. Dynamic `/v1/models` Discovery (`models.rs`)
Rather than hardcoding static model lists, `m31a-provider` queries the live NVIDIA NIM endpoint. It discovers active model slugs, context window limits, and capability tags (`supports_tools`, `supports_reasoning`, `context_window_size`).

Pre-configured flagship models include:
- `nvidia/nemotron-3.5-lightning-30b-a3b`: Default workhorse model with high tool precision and reasoning.
- `nvidia/nemotron-3.5-bolt-e4b`: Ultra-low latency, optimized for rapid vibe coding auto-healing.
- `deepseek-ai/deepseek-v4.1-flash`: Frontier reasoning with thinking stream support.
- `mistralai/codestral-22b-instruct-v0.1`: Specialized for multi-file patch synthesis and refactoring.
- `nvidia/llama-3.1-nemotron-70b-instruct`: Benchmark-leading precision for structured function calling.

### 2. Dual-Channel SSE Streaming (`client.rs`)
When streaming completions, `m31a-provider` inspects chunks on the fly. It separates content into two distinct event streams:
1. **Reasoning Stream**: Emits internal `<think>` chunks directly to the TUI collapsible viewport (`Ctrl+T`) and SDK callbacks (`on_reasoning`).
2. **Content Stream**: Emits user-facing markdown text and tool call JSON payloads.

### 3. Task-Specific Routing (`router.rs`)
`ModelRouter` can intelligently route requests based on operational context:
- Routing quick build-error repairs in `m31a vibe` to ultra-fast models (`bolt-e4b`).
- Routing deep speculative design analysis in `m31a speculate` to frontier reasoning models (`deepseek-v4.1-flash`).

### 4. Resilient Backoff & Jitter (`retry.rs`)
Handles network drops and HTTP 429 rate limits using full jitter exponential backoff (`decorrelated_jitter`).

---

## Key Modules & Types

| Module | Key Structs / Traits | Purpose |
| :--- | :--- | :--- |
| [`src/client.rs`](src/client.rs) | `NvidiaNimClient`, `StreamChunk` | Async HTTP/SSE streaming client connecting to NVIDIA NIM endpoints. |
| [`src/provider_trait.rs`](src/provider_trait.rs) | `ModelProvider`, `ChatRequest`, `ChatResponse` | Core abstraction implemented by all intelligence providers. |
| [`src/models.rs`](src/models.rs) | `ModelCatalog`, `ModelDefinition`, `ModelCapabilities` | Dynamic `/v1/models` discovery, context window tracking, and capabilities. |
| [`src/router.rs`](src/router.rs) | `ModelRouter`, `RoutingRule` | Intelligently routes tasks to specialized models (speed, reasoning, code). |
| [`src/retry.rs`](src/retry.rs) | `RetryPolicy`, `BackoffConfig` | Exponential backoff with decorrelated jitter for rate-limit resilience. |

---

## Practical Usage Example

```rust
use m31a_core::types::{ChatMessage, MessageRole};
use m31a_provider::{ChatRequest, NvidiaNimClient, ProviderConfig};
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize NVIDIA NIM client
    let config = ProviderConfig {
        api_key: std::env::var("NVIDIA_API_KEY")?,
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".into(),
        api_base: "https://integrate.api.nvidia.com/v1".into(),
        ..Default::default()
    };
    let client = NvidiaNimClient::new(config)?;

    // 2. Prepare chat request with reasoning enabled
    let request = ChatRequest {
        messages: vec![
            ChatMessage {
                role: MessageRole::System,
                content: "You are an expert systems engineer.".into(),
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Explain how Linux namespaces enable container isolation.".into(),
            },
        ],
        temperature: Some(0.2),
        max_tokens: Some(1024),
        tools: vec![],
    };

    // 3. Stream response with dual-channel reasoning separation
    let mut stream = client.stream_chat(request).await?;
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        if let Some(reasoning) = chunk.reasoning_content {
            print!("[Thinking] {}", reasoning);
        }
        if let Some(text) = chunk.content {
            print!("{}", text);
        }
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
# Run unit and offline mock tests
cargo test -p m31a-provider

# Run live provider tests against build.nvidia.com (requires NVIDIA_API_KEY)
NVIDIA_API_KEY="nvapi-..." cargo test -p m31a-provider -- --ignored
```
