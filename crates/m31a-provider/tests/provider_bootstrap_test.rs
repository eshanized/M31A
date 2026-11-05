//! Tests for ProviderType resolution, credential validation, and ProviderBuilder client bootstrap.
//!
//! Validates actual NVIDIA NIM provider integration in v1 without mocks or synthetic success shortcuts.

use m31a_core::types::ChatMessage;
use m31a_provider::{
    LocalWireServer, ProviderBuilder, ProviderConfig, ProviderError, ProviderType,
    WireServerResponse, validate_provider_config,
};
use std::collections::HashMap;

#[test]
fn test_provider_type_deterministic_resolution() {
    // 1. Explicit provider selection for NVIDIA NIM takes precedence
    assert_eq!(
        ProviderType::resolve(Some("nim"), "meta/llama-3.3-70b-instruct", None).unwrap(),
        ProviderType::Nim
    );
    assert_eq!(
        ProviderType::resolve(Some("nvidia"), "meta/llama-3.3-70b-instruct", None).unwrap(),
        ProviderType::Nim
    );
    assert_eq!(
        ProviderType::resolve(Some("nvidia-nim"), "meta/llama-3.3-70b-instruct", None).unwrap(),
        ProviderType::Nim
    );

    // 2. Unsupported providers fail explicitly in v1
    for unsupported in [
        "openai",
        "anthropic",
        "gemini",
        "groq",
        "deepseek",
        "mistral",
        "together",
        "ollama",
        "lmstudio",
        "custom",
        "mock",
        "unsupported-vendor",
    ] {
        let err = ProviderType::resolve(Some(unsupported), "any-model", None).unwrap_err();
        assert!(
            matches!(err, ProviderError::UnknownProvider { .. }),
            "Expected UnknownProvider for {unsupported}"
        );
    }

    // 3. Known endpoint domain mappings (NVIDIA NIM or local testing endpoints)
    assert_eq!(
        ProviderType::resolve(
            None,
            "custom-nim-model",
            Some("https://integrate.api.nvidia.com/v1")
        )
        .unwrap(),
        ProviderType::Nim
    );
    assert_eq!(
        ProviderType::resolve(None, "local-test-model", Some("http://127.0.0.1:8080/v1")).unwrap(),
        ProviderType::Nim
    );
    assert_eq!(
        ProviderType::resolve(None, "local-test-model", Some("http://localhost:8080/v1")).unwrap(),
        ProviderType::Nim
    );

    // 4. Canonical catalog models resolve to NIM
    assert_eq!(
        ProviderType::resolve(None, "meta/llama-3.3-70b-instruct", None).unwrap(),
        ProviderType::Nim
    );
    assert_eq!(
        ProviderType::resolve(None, "nvidia/nemotron-3-ultra-550b-a55b", None).unwrap(),
        ProviderType::Nim
    );

    // 5. Unknown model without provider or valid endpoint must fail explicitly
    assert!(ProviderType::resolve(None, "completely-unknown-model", None).is_err());
    assert!(ProviderType::resolve(None, "claude-3-5-sonnet", None).is_err());
    assert!(ProviderType::resolve(None, "gpt-4o", None).is_err());
    assert!(ProviderType::resolve(None, "gemini-1.5-pro", None).is_err());
}

#[test]
fn test_provider_credential_validation() {
    // 1. Missing API key rejected
    let config_missing = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: None,
        api_key: None,
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };
    let err = validate_provider_config(&config_missing).unwrap_err();
    assert!(
        matches!(err, ProviderError::MissingApiKey { .. }),
        "Expected MissingApiKey, got {:?}",
        err
    );

    // 2. Whitespace-only API key rejected
    let config_whitespace = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: None,
        api_key: Some("   \t\n  ".to_string()),
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };
    let ws_err = validate_provider_config(&config_whitespace).unwrap_err();
    assert!(matches!(ws_err, ProviderError::MissingApiKey { .. }));

    // 3. Valid key and valid catalog model succeeds
    let config_valid = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: None,
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        extra: HashMap::new(),
    };
    assert!(validate_provider_config(&config_valid).is_ok());

    // 4. Malformed endpoint rejected
    let malformed_config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some("not-a-url".to_string()),
        api_key: Some("nvapi-runtime-integration-token".to_string()),
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        extra: HashMap::new(),
    };
    let err = validate_provider_config(&malformed_config).unwrap_err();
    assert!(matches!(err, ProviderError::InvalidEndpoint { .. }));
}

#[tokio::test]
async fn test_provider_builder_nim_bootstrap() {
    let server = LocalWireServer::start()
        .await
        .expect("Wire server must start on local TCP socket");
    server.enqueue_text("Real response from NVIDIA NIM over TCP transport.");

    let config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some(server.api_base()),
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        extra: HashMap::new(),
    };

    let client = ProviderBuilder::build_client(config)
        .expect("NVIDIA NIM client build should succeed with valid config");
    assert_eq!(client.model(), "nvidia/nemotron-3.5-lightning-30b-a3b");

    // Real chat through universal ProviderClient over actual TCP HTTP
    let (msg, usage) = client
        .chat(&[ChatMessage::user("ping")], None)
        .await
        .expect("NVIDIA chat through universal client should succeed");
    assert_eq!(
        msg.content.as_deref(),
        Some("Real response from NVIDIA NIM over TCP transport.")
    );
    assert!(
        !server.requests().is_empty(),
        "Must verify actual TCP HTTP request arrived at wire server"
    );
    assert!(server.requests()[0].contains("POST /v1/chat/completions"));
    assert!(usage.total_tokens > 0 || usage.prompt_tokens > 0);
}

#[tokio::test]
async fn test_provider_builder_nim_streaming_actual() {
    let server = LocalWireServer::start()
        .await
        .expect("Wire server must start on local TCP socket");
    server.enqueue_response(
        WireServerResponse::text("Streaming test token verification.")
            .with_tool_call("execute_code", r#"{"code":"println!(42);"}"#),
    );

    let config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some(server.api_base()),
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        extra: HashMap::new(),
    };

    let client = ProviderBuilder::build_client(config)
        .expect("NVIDIA NIM client build should succeed with valid config");
    let content_chunks = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cc = content_chunks.clone();

    let (msg, usage) = client
        .stream_chat(
            &[ChatMessage::user("run test")],
            None,
            Some(std::sync::Arc::new(move |_chunk| {
                cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })),
            None,
        )
        .await
        .expect("Streaming chat should succeed");

    assert_eq!(
        msg.content.as_deref(),
        Some("Streaming test token verification.")
    );
    assert!(
        content_chunks.load(std::sync::atomic::Ordering::SeqCst) > 0,
        "Must receive stream chunks via callback"
    );
    assert!(msg.tool_calls.is_some(), "Tool calls must be parsed");
    let calls = msg.tool_calls.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.name, "execute_code");
    assert_eq!(calls[0].function.arguments, r#"{"code":"println!(42);"}"#);
    assert!(usage.prompt_tokens > 0 || usage.total_tokens > 0);
}

#[test]
fn test_provider_builder_nim_missing_key_fails_closed() {
    let config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: None,
        api_key: None, // Missing key!
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };

    let res = ProviderBuilder::build_client(config);
    assert!(res.is_err());
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("Missing API key for provider 'NVIDIA NIM'")
    );
}

#[tokio::test]
async fn test_empty_api_key_fails_closed() {
    let config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some("https://integrate.api.nvidia.com/v1".to_string()),
        api_key: None,
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };
    let res = ProviderBuilder::build_client(config);
    assert!(res.is_err(), "Empty/missing API key must fail honestly");
    let err = res.err().unwrap().to_string();
    assert!(
        err.contains("Missing API key for provider 'NVIDIA NIM'"),
        "Error was: {err}"
    );
}

#[tokio::test]
async fn test_model_availability_unresponsive_endpoint_fails_honestly() {
    let config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some("http://127.0.0.1:1".to_string()), // Dead port
        api_key: Some("nvapi-key".to_string()),
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };
    let client =
        ProviderBuilder::build_client(config).expect("Client should build with explicit config");
    let health_res = client.test_model_availability().await;
    assert!(
        health_res.is_err(),
        "Unresponsive endpoint must fail availability test"
    );
}

#[tokio::test]
async fn test_dynamic_context_window_and_capabilities_resolution() {
    let api_key = m31a_core::resolve_stored_api_key()
        .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string());

    // 1. Llama-3.3-70b (128k context)
    let llama_config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some("https://integrate.api.nvidia.com/v1".to_string()),
        api_key: Some(api_key.clone()),
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };
    let llama_client = ProviderBuilder::build_client(llama_config).unwrap();
    let llama_caps = llama_client.capabilities();
    assert_eq!(llama_caps.max_context_tokens, 131_072);
    assert!(llama_caps.supports_tools);

    // 2. Qwen 2.5 Coder 32B (32k context)
    let qwen_config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some("https://integrate.api.nvidia.com/v1".to_string()),
        api_key: Some(api_key.clone()),
        model: "qwen/qwen2.5-coder-32b-instruct".to_string(),
        extra: HashMap::new(),
    };
    let qwen_client = ProviderBuilder::build_client(qwen_config).unwrap();
    let qwen_caps = qwen_client.capabilities();
    assert_eq!(qwen_caps.max_context_tokens, 32_768);
    assert!(qwen_caps.supports_tools);

    // 3. Nemotron 3 Ultra 550B (1M context)
    let ultra_config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some("https://integrate.api.nvidia.com/v1".to_string()),
        api_key: Some(api_key),
        model: "nvidia/nemotron-3-ultra-550b-a55b".to_string(),
        extra: HashMap::new(),
    };
    let ultra_client = ProviderBuilder::build_client(ultra_config).unwrap();
    let ultra_caps = ultra_client.capabilities();
    assert_eq!(ultra_caps.max_context_tokens, 1_048_576);
    assert!(ultra_caps.supports_reasoning);

    // 4. Client model switching via `.with_model()` dynamically adapts capabilities
    let switched = llama_client.with_model("qwen/qwen2.5-coder-32b-instruct");
    assert_eq!(switched.capabilities().max_context_tokens, 32_768);
}
