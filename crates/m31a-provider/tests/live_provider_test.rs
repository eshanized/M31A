//! Live integration tests for NVIDIA NIM API using real model inference.
//!
//! Target:
//! - Model: nvidia/nemotron-3-ultra-550b-a55b
//! - Provider: NVIDIA NIM
//! - Base URL: https://integrate.api.nvidia.com/v1
//! - Endpoint: /chat/completions
//!
//! Gated by:
//! - Cargo feature: `live-nvidia-tests`
//! - Environment variable: `M31A_RUN_LIVE_NVIDIA_TESTS=1`

use m31a_core::types::{ChatMessage, FunctionDefinition, ToolDefinition};
use m31a_provider::live_test_support::{
    LiveTestConfig, NVIDIA_NEMOTRON_LIGHTNING, NVIDIA_NIM_BASE_URL, ensure_live_test_opt_in,
};
use m31a_provider::{NvidiaNimClient, ProviderBuilder, ProviderConfig, ProviderType};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
async fn test_real_live_provider_chat_streaming() {
    let cfg = LiveTestConfig::load();
    let client = NvidiaNimClient::new(cfg.api_base, &cfg.api_key, cfg.model)
        .with_max_tokens(Some(cfg.short_output_budget))
        .with_reasoning_budget(Some(cfg.medium_reasoning_budget))
        .with_reasoning_effort(Some(cfg.reasoning_effort.to_string()))
        .with_temperature(Some(cfg.temperature))
        .with_top_p(Some(cfg.top_p));

    assert_eq!(client.model(), cfg.model);

    let messages = vec![ChatMessage::user(
        "What is 13 * 13? Answer with just the numeric result.",
    )];

    let content_chunks = Arc::new(AtomicUsize::new(0));
    let reasoning_chunks = Arc::new(AtomicUsize::new(0));

    let cc = Arc::clone(&content_chunks);
    let rc = Arc::clone(&reasoning_chunks);

    let start = std::time::Instant::now();
    let (response, usage) = client
        .stream_chat(
            &messages,
            None,
            Some(Arc::new(move |_| {
                cc.fetch_add(1, Ordering::SeqCst);
            })),
            Some(Arc::new(move |_| {
                rc.fetch_add(1, Ordering::SeqCst);
            })),
        )
        .await
        .expect("stream_chat must succeed against real NVIDIA NIM API");

    let elapsed = start.elapsed();
    let text = response.content.as_deref().unwrap_or_default();
    println!(
        "Live Nemotron Ultra response [{} ms]: {}",
        elapsed.as_millis(),
        text
    );

    // Record safe metadata only — never print raw reasoning traces to logs
    let has_reasoning = response.reasoning_content.is_some();
    let reasoning_len = response
        .reasoning_content
        .as_ref()
        .map(|r| r.len())
        .unwrap_or(0);
    let reasoning_count = reasoning_chunks.load(Ordering::SeqCst);
    println!(
        "Live reasoning safe metadata: present={}, byte_len={}, chunk_count={}",
        has_reasoning, reasoning_len, reasoning_count
    );
    if has_reasoning && reasoning_len > 0 {
        assert!(
            reasoning_count > 0,
            "Must observe reasoning chunk callbacks when reasoning content is returned"
        );
    }

    println!(
        "Usage: prompt={}, completion={}, total={}",
        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
    );

    // Verify final content independently (do not concatenate reasoning)
    assert!(
        text.contains("169"),
        "Expected response content containing 169, got: {}",
        text
    );
    assert!(
        content_chunks.load(Ordering::SeqCst) > 0,
        "Must receive streamed content chunks over SSE"
    );
    assert!(
        usage.total_tokens > 0 && usage.prompt_tokens > 0,
        "Must record non-zero token usage from NIM endpoint"
    );
}

#[tokio::test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
async fn test_real_live_provider_tool_calling() {
    let cfg = LiveTestConfig::load();
    let client = NvidiaNimClient::new(cfg.api_base, &cfg.api_key, cfg.model)
        .with_max_tokens(Some(cfg.short_output_budget))
        .with_reasoning_budget(Some(cfg.medium_reasoning_budget))
        .with_reasoning_effort(Some(cfg.reasoning_effort.to_string()))
        .with_temperature(Some(cfg.temperature))
        .with_top_p(Some(cfg.top_p))
        .with_tool_choice(Some("required".to_string()));

    let tools = vec![ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: "get_weather".to_string(),
            description: "Get weather for a city".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }),
        },
    }];

    let messages = vec![ChatMessage::user(
        "What is the weather in Tokyo right now? You must invoke the get_weather tool.",
    )];

    let start = std::time::Instant::now();
    let (response, usage) = client
        .stream_chat(&messages, Some(&tools), None, None)
        .await
        .expect("stream_chat with tools must succeed");

    let elapsed = start.elapsed();
    println!(
        "Live tool call response received in {} ms (prompt={}, completion={}, total={})",
        elapsed.as_millis(),
        usage.prompt_tokens,
        usage.completion_tokens,
        usage.total_tokens
    );

    assert!(
        response.tool_calls.is_some(),
        "Model must return tool_calls: {:?}",
        response
    );
    let calls = response.tool_calls.unwrap();
    assert!(!calls.is_empty(), "Must have at least one tool call");
    assert_eq!(calls[0].function.name, "get_weather");

    let args: serde_json::Value = serde_json::from_str(&calls[0].function.arguments)
        .expect("Tool call arguments must be valid JSON");
    let city = args
        .get("city")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    assert!(
        city.contains("tokyo"),
        "Expected city argument 'tokyo', got: {:?}",
        args
    );
}

#[tokio::test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
async fn test_real_live_provider_abstraction_pipeline() {
    let cfg = LiveTestConfig::load();

    // 1. Explicit provider resolution
    let resolved = ProviderType::resolve(None, cfg.model, Some(cfg.api_base))
        .expect("Provider resolution for Nemotron Ultra must succeed");
    assert_eq!(resolved, ProviderType::Nim);

    // 2. Client construction via ProviderBuilder
    let provider_config = ProviderConfig {
        provider_type: resolved,
        api_base: Some(cfg.api_base.to_string()),
        api_key: Some(cfg.api_key.clone()),
        model: cfg.model.to_string(),
        extra: HashMap::new(),
    };
    let client = ProviderBuilder::build_client(provider_config)
        .expect("ProviderBuilder must construct ProviderClient")
        .with_max_tokens(Some(cfg.short_output_budget))
        .with_reasoning_budget(Some(cfg.medium_reasoning_budget))
        .with_reasoning_effort(Some(cfg.reasoning_effort.to_string()))
        .with_temperature(Some(cfg.temperature));

    assert_eq!(client.model(), cfg.model);

    // 3. Application-level inference traversal
    let messages = vec![ChatMessage::user(
        "Compute 7 * 9. Answer with only the number.",
    )];

    let start = std::time::Instant::now();
    let (response, usage) = client
        .stream_chat(&messages, None, None, None)
        .await
        .expect("Inference through ProviderClient abstraction must succeed");

    let elapsed = start.elapsed();
    let text = response.content.as_deref().unwrap_or_default();
    println!(
        "Live ProviderClient response [{} ms]: {}",
        elapsed.as_millis(),
        text
    );
    assert!(text.contains("63"), "Expected 63, got: {}", text);
    assert!(usage.total_tokens > 0 || usage.prompt_tokens > 0);
}

#[tokio::test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
async fn test_real_live_provider_invalid_key_error_propagation() {
    ensure_live_test_opt_in();
    // Deliberately verify failure without mock fallback
    let client = NvidiaNimClient::new(
        NVIDIA_NIM_BASE_URL,
        "nvapi-invalid-key-testing-honest-error-propagation",
        NVIDIA_NEMOTRON_LIGHTNING,
    );

    let messages = vec![ChatMessage::user("Ping")];
    let result = client.stream_chat(&messages, None, None, None).await;

    assert!(result.is_err(), "Invalid API key must return an error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("401") || err.contains("Unauthorized") || err.contains("API Error"),
        "Error message must indicate honest auth failure without mock fallback: {err}"
    );
}

#[tokio::test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
async fn test_real_live_provider_client_model_cloning_and_inference() {
    let cfg = LiveTestConfig::load();
    // Start with a base client configuration
    let base_client = NvidiaNimClient::new(
        cfg.api_base,
        &cfg.api_key,
        "nvidia/nemotron-3.5-lightning-30b-a3b",
    )
    .with_max_tokens(Some(cfg.short_output_budget))
    .with_reasoning_budget(Some(cfg.medium_reasoning_budget))
    .with_reasoning_effort(Some(cfg.reasoning_effort.to_string()))
    .with_temperature(Some(cfg.temperature))
    .with_top_p(Some(cfg.top_p));

    assert_eq!(base_client.model(), "nvidia/nemotron-3.5-lightning-30b-a3b");

    // Clone client targeting NVIDIA Nemotron 3.5 Bolt
    let switched_client = base_client.with_model("nvidia/nemotron-3.5-bolt-e4b");
    assert_eq!(switched_client.model(), "nvidia/nemotron-3.5-bolt-e4b");
    assert_ne!(switched_client.model(), base_client.model());

    // Execute real inference on cloned Nemotron 3.5 Bolt client
    let (resp, _) = switched_client
        .stream_chat(
            &[ChatMessage::user("Say the exact word 'STELLAR' only.")],
            None,
            None,
            None,
        )
        .await
        .expect("Inference on cloned Nemotron Bolt client must succeed");

    let text = resp.content.as_deref().unwrap_or("");
    assert!(
        text.to_uppercase().contains("STELLAR"),
        "Expected user-facing content containing 'STELLAR', got: {}",
        text
    );
}

#[tokio::test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
async fn test_real_live_remote_models_discovery_and_caching() {
    let cfg = LiveTestConfig::load();

    // 1. Fetch live models from NVIDIA NIM using real API key
    let models = m31a_provider::fetch_remote_models(cfg.api_base, &cfg.api_key)
        .await
        .expect("Live fetch_remote_models against NVIDIA NIM must succeed");

    assert!(
        models.len() >= 50,
        "Expected at least 50 models on NVIDIA NIM, discovered: {}",
        models.len()
    );

    // 2. Verify key models are present with correct context windows
    let nemotron = models
        .iter()
        .find(|m| m.slug == "nvidia/nemotron-3-ultra-550b-a55b")
        .expect("nemotron-3-ultra must be present in live discovered models");
    assert_eq!(nemotron.context_window, 1_048_576);

    let lightning = models
        .iter()
        .find(|m| m.slug == "nvidia/nemotron-3.5-lightning-30b-a3b")
        .expect("nemotron-3.5-lightning must be present");
    assert!(lightning.context_window > 0);

    // 3. Test caching envelope roundtrip
    m31a_provider::save_cached_models(&models);
    let loaded = m31a_provider::load_cached_models();
    assert!(loaded.len() >= models.len());
    assert!(!m31a_provider::is_models_cache_stale(
        m31a_provider::DEFAULT_MODELS_CACHE_TTL_SECS
    ));
}
