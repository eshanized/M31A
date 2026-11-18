//! Provider client initialization, discovery, and dynamic model switching.

use crate::error::RuntimeError;
use m31a_provider::models::{ModelInfo, get_nvidia_model_catalog};
use m31a_provider::{ProviderBuilder, ProviderClient, ProviderConfig, ProviderType};

/// Initialize a provider client for the specified model and credentials.
pub fn init_provider_client(
    model_provider: Option<&str>,
    model: &str,
    api_base: &str,
    api_key: Option<&str>,
    reasoning_effort: Option<String>,
) -> Result<ProviderClient, RuntimeError> {
    let provider_type = ProviderType::resolve(model_provider, model, Some(api_base))
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

    let config = ProviderConfig {
        provider_type,
        api_base: Some(api_base.to_string()),
        api_key: api_key.filter(|k| !k.trim().is_empty()).map(String::from),
        model: model.to_string(),
        extra: std::collections::HashMap::new(),
    };

    m31a_provider::validate_provider_config(&config)
        .map_err(|e| RuntimeError::Provider(e.to_string()))?;

    let client =
        ProviderBuilder::build_client(config).map_err(|e| RuntimeError::Provider(e.to_string()))?;

    Ok(client.with_reasoning_effort(reasoning_effort))
}

/// Discover available models combining catalog and cached entries.
pub fn discover_available_models(active_model: &str) -> Vec<ModelInfo> {
    let cached = m31a_provider::load_cached_models();
    let mut models = get_nvidia_model_catalog();
    for m in cached {
        if !models.iter().any(|existing| existing.slug == m.slug) {
            models.push(m);
        }
    }

    if !models.iter().any(|m| m.slug == active_model) {
        let model_info = if let Some(m) = m31a_provider::find_model(active_model) {
            m
        } else if let Some(c) = m31a_provider::find_canonical_model(active_model) {
            ModelInfo {
                slug: c.id.clone(),
                display_name: c.name.clone(),
                description: c.description.clone(),
                context_window: c.context_window.unwrap_or(0),
                max_output_tokens: c.max_output_tokens.unwrap_or(0),
                supports_tools: c.capabilities.tool_calling,
                supports_thinking: c.capabilities.reasoning,
                supports_vision: c.capabilities.vision,
                supports_audio: c.capabilities.audio,
                originator: c.publisher.clone(),
                ..ModelInfo::default()
            }
        } else {
            ModelInfo {
                slug: active_model.to_string(),
                display_name: active_model.to_string(),
                description: format!("Configured custom model: {}", active_model),
                context_window: 0,
                max_output_tokens: 0,
                supports_tools: false,
                supports_thinking: false,
                supports_vision: false,
                supports_audio: false,
                originator: "custom".to_string(),
                ..ModelInfo::default()
            }
        };
        models.insert(0, model_info);
    }

    models
}
