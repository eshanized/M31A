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
        api_key: Some("nvapi-valid-test-key".to_string()),
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };
    assert!(validate_provider_config(&config_valid).is_ok());

    // 4. Malformed endpoint rejected
    let malformed_config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some("not-a-url".to_string()),
        api_key: Some("nvapi-key".to_string()),
        model: "meta/llama-3.3-70b-instruct".to_string(),
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
        api_key: Some("nvapi-test-key".to_string()),
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };

    let client = ProviderBuilder::build_client(config)
        .expect("NVIDIA NIM client build should succeed with valid config");
    assert_eq!(client.model(), "meta/llama-3.3-70b-instruct");

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
        api_key: Some("nvapi-test-key".to_string()),
        model: "meta/llama-3.3-70b-instruct".to_string(),
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
