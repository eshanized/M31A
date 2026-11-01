//! Tests for ProviderType resolution, credential validation, and ProviderBuilder client bootstrap.

use m31a_core::types::ChatMessage;
use m31a_provider::{
    ProviderBuilder, ProviderConfig, ProviderError, ProviderType, validate_provider_config,
};
use std::collections::HashMap;

#[test]
fn test_provider_type_deterministic_resolution() {
    // 1. Mock model names must NEVER implicitly resolve to Mock provider or Nim
    assert!(ProviderType::resolve(None, "mock-model", None).is_err());
    assert!(ProviderType::resolve(None, "test-agent", None).is_err());

    // 2. Explicit provider selection takes absolute precedence
    assert_eq!(
        ProviderType::resolve(Some("anthropic"), "any-model", None).unwrap(),
        ProviderType::Anthropic
    );
    assert_eq!(
        ProviderType::resolve(Some("openai"), "any-model", None).unwrap(),
        ProviderType::OpenAi
    );
    assert_eq!(
        ProviderType::resolve(Some("gemini"), "any-model", None).unwrap(),
        ProviderType::Gemini
    );
    assert_eq!(
        ProviderType::resolve(Some("groq"), "any-model", None).unwrap(),
        ProviderType::Groq
    );
    assert_eq!(
        ProviderType::resolve(Some("deepseek"), "any-model", None).unwrap(),
        ProviderType::DeepSeek
    );
    assert_eq!(
        ProviderType::resolve(Some("mistral"), "any-model", None).unwrap(),
        ProviderType::Mistral
    );
    assert_eq!(
        ProviderType::resolve(Some("together"), "any-model", None).unwrap(),
        ProviderType::Together
    );
    assert_eq!(
        ProviderType::resolve(Some("ollama"), "any-model", None).unwrap(),
        ProviderType::Ollama
    );
    assert_eq!(
        ProviderType::resolve(Some("lmstudio"), "any-model", None).unwrap(),
        ProviderType::LmStudio
    );
    assert_eq!(
        ProviderType::resolve(Some("custom"), "any-model", None).unwrap(),
        ProviderType::Custom
    );

    // 3. Unknown explicit provider fails explicitly
    let err = ProviderType::resolve(Some("unsupported-vendor"), "model-x", None).unwrap_err();
    assert!(matches!(err, ProviderError::UnknownProvider { .. }));

    // 4. Known endpoint domain mappings
    assert_eq!(
        ProviderType::resolve(None, "custom-claude", Some("https://api.anthropic.com/v1")).unwrap(),
        ProviderType::Anthropic
    );
    assert_eq!(
        ProviderType::resolve(None, "custom-gpt", Some("https://api.openai.com/v1")).unwrap(),
        ProviderType::OpenAi
    );
    assert_eq!(
        ProviderType::resolve(
            None,
            "custom-gemini",
            Some("https://generativelanguage.googleapis.com/v1")
        )
        .unwrap(),
        ProviderType::Gemini
    );
    assert_eq!(
        ProviderType::resolve(None, "custom-groq", Some("https://api.groq.com/openai/v1")).unwrap(),
        ProviderType::Groq
    );
    assert_eq!(
        ProviderType::resolve(None, "custom-deepseek", Some("https://api.deepseek.com/v1"))
            .unwrap(),
        ProviderType::DeepSeek
    );
    assert_eq!(
        ProviderType::resolve(None, "custom-mistral", Some("https://api.mistral.ai/v1")).unwrap(),
        ProviderType::Mistral
    );
    assert_eq!(
        ProviderType::resolve(None, "custom-together", Some("https://api.together.xyz/v1"))
            .unwrap(),
        ProviderType::Together
    );
    assert_eq!(
        ProviderType::resolve(None, "llama3.2", Some("http://localhost:11434/v1")).unwrap(),
        ProviderType::Ollama
    );
    assert_eq!(
        ProviderType::resolve(None, "qwen", Some("http://localhost:1234/v1")).unwrap(),
        ProviderType::LmStudio
    );

    // 5. Canonical catalog models resolve to NIM
    assert_eq!(
        ProviderType::resolve(None, "meta/llama-3.3-70b-instruct", None).unwrap(),
        ProviderType::Nim
    );

    // 6. Canonical prefix migrations
    assert_eq!(
        ProviderType::resolve(None, "claude-3-5-sonnet", None).unwrap(),
        ProviderType::Anthropic
    );
    assert_eq!(
        ProviderType::resolve(None, "gpt-4o", None).unwrap(),
        ProviderType::OpenAi
    );
    assert_eq!(
        ProviderType::resolve(None, "o1-preview", None).unwrap(),
        ProviderType::OpenAi
    );
    assert_eq!(
        ProviderType::resolve(None, "gemini-1.5-pro", None).unwrap(),
        ProviderType::Gemini
    );

    // 7. Unknown model without provider or custom endpoint must fail explicitly
    assert!(ProviderType::resolve(None, "completely-unknown-model", None).is_err());
}

#[test]
fn test_provider_credential_validation_all_providers() {
    let remote_providers = [
        (ProviderType::Nim, "meta/llama-3.3-70b-instruct"),
        (ProviderType::Anthropic, "claude-3-5-sonnet-20241022"),
        (ProviderType::OpenAi, "gpt-4o"),
        (ProviderType::Gemini, "gemini-1.5-pro-latest"),
        (ProviderType::Groq, "llama-3.3-70b-versatile"),
        (ProviderType::DeepSeek, "deepseek-chat"),
        (ProviderType::Mistral, "mistral-large-latest"),
        (
            ProviderType::Together,
            "meta-llama/Llama-3.3-70B-Instruct-Turbo",
        ),
    ];

    // Every remote provider must reject missing API keys
    for (pt, model) in remote_providers {
        let config = ProviderConfig {
            provider_type: pt,
            api_base: None,
            api_key: None,
            model: model.to_string(),
            extra: HashMap::new(),
        };
        let err = validate_provider_config(&config).unwrap_err();
        assert!(
            matches!(err, ProviderError::MissingApiKey { .. }),
            "Expected MissingApiKey for {:?}, got {:?}",
            pt,
            err
        );
        // Never exposes raw keys in error
        assert!(!err.to_string().contains("sk-"));

        // Whitespace-only key must also be rejected
        let config_whitespace = ProviderConfig {
            provider_type: pt,
            api_base: None,
            api_key: Some("   \t\n  ".to_string()),
            model: model.to_string(),
            extra: HashMap::new(),
        };
        let ws_err = validate_provider_config(&config_whitespace).unwrap_err();
        assert!(matches!(ws_err, ProviderError::MissingApiKey { .. }));

        // Valid key succeeds validation
        let config_valid = ProviderConfig {
            provider_type: pt,
            api_base: None,
            api_key: Some("valid-credential".to_string()),
            model: model.to_string(),
            extra: HashMap::new(),
        };
        assert!(validate_provider_config(&config_valid).is_ok());
    }

    // Local providers deliberately permit no-key operation
    for pt in [ProviderType::Ollama, ProviderType::LmStudio] {
        let config = ProviderConfig {
            provider_type: pt,
            api_base: None,
            api_key: None,
            model: "local-model".to_string(),
            extra: HashMap::new(),
        };
        assert!(
            validate_provider_config(&config).is_ok(),
            "Local provider {:?} must permit no-key operation",
            pt
        );
    }

    // Custom endpoint validation
    // 1. Custom with key succeeds
    let custom_with_key = ProviderConfig {
        provider_type: ProviderType::Custom,
        api_base: Some("https://custom.endpoint.com/v1".to_string()),
        api_key: Some("custom-key".to_string()),
        model: "custom-model".to_string(),
        extra: HashMap::new(),
    };
    assert!(validate_provider_config(&custom_with_key).is_ok());

    // 2. Custom without key and without explicit policy fails
    let custom_no_key = ProviderConfig {
        provider_type: ProviderType::Custom,
        api_base: Some("https://custom.endpoint.com/v1".to_string()),
        api_key: None,
        model: "custom-model".to_string(),
        extra: HashMap::new(),
    };
    let err = validate_provider_config(&custom_no_key).unwrap_err();
    assert!(matches!(err, ProviderError::MissingApiKey { .. }));

    // 3. Custom without key but with explicit allow_anonymous succeeds
    let mut extra_anon = HashMap::new();
    extra_anon.insert("allow_anonymous".to_string(), serde_json::Value::Bool(true));
    let custom_anon = ProviderConfig {
        provider_type: ProviderType::Custom,
        api_base: Some("https://custom.endpoint.com/v1".to_string()),
        api_key: None,
        model: "custom-model".to_string(),
        extra: extra_anon,
    };
    assert!(validate_provider_config(&custom_anon).is_ok());

    // 4. Custom without key but with explicit auth_policy = "none" succeeds
    let mut extra_none = HashMap::new();
    extra_none.insert(
        "auth_policy".to_string(),
        serde_json::Value::String("none".to_string()),
    );
    let custom_none = ProviderConfig {
        provider_type: ProviderType::Custom,
        api_base: Some("https://custom.endpoint.com/v1".to_string()),
        api_key: None,
        model: "custom-model".to_string(),
        extra: extra_none,
    };
    assert!(validate_provider_config(&custom_none).is_ok());

    // Malformed endpoint validation
    let malformed_config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some("not-a-url".to_string()),
        api_key: Some("key".to_string()),
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };
    let err = validate_provider_config(&malformed_config).unwrap_err();
    assert!(matches!(err, ProviderError::InvalidEndpoint { .. }));
}

#[tokio::test]
async fn test_provider_builder_mock_bootstrap() {
    let config = ProviderConfig {
        provider_type: ProviderType::Mock,
        api_base: None,
        api_key: None,
        model: "mock-model".to_string(),
        extra: HashMap::new(),
    };

    let client =
        ProviderBuilder::build_client(config).expect("Mock provider client build should succeed");
    assert_eq!(client.model(), "mock-model");

    // Can execute chat through universal ProviderClient
    let (msg, _usage) = client
        .chat(&[ChatMessage::user("ping")], None)
        .await
        .expect("Mock chat should succeed");
    assert!(!msg.content.unwrap_or_default().is_empty());
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
async fn test_empty_api_key_does_not_activate_mock_provider() {
    let config = ProviderConfig {
        provider_type: ProviderType::Nim,
        api_base: Some("https://integrate.api.nvidia.com/v1".to_string()),
        api_key: None,
        model: "meta/llama-3.3-70b-instruct".to_string(),
        extra: HashMap::new(),
    };
    let res = ProviderBuilder::build_client(config);
    assert!(
        res.is_err(),
        "Empty/missing API key must fail honestly, not fallback to MockProvider"
    );
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
        api_key: Some("fake-key".to_string()),
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
