//! Model provider capability overrides and configuration transformers.
//!
//! Configures model capabilities, tool invocation modes, streaming parameters, and context limits:
//! - Declares provider features such as remote compaction protocols and live external web access.
//! - Transforms model instructions and context parameters according to user preferences.

use serde::{Deserialize, Serialize};

pub const PERSONALITY_SECTION_HEADER: &str = "# Personality";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RemoteCompactionSupport {
    Unsupported,
    V2,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModelProviderCapabilities {
    pub external_web_access: Option<bool>,
    pub remote_compaction: Option<RemoteCompactionSupport>,
}

#[derive(Debug, Clone, Default)]
pub struct ModelConfigOverrides {
    pub model_context_window: Option<usize>,
    pub model_auto_compact_token_limit: Option<usize>,
    pub tool_output_token_limit: Option<usize>,
    pub strip_personality: bool,
}

/// Applies configuration overrides to model parameters and system instructions.
pub fn apply_model_config_overrides(
    mut base_instructions: String,
    current_context_window: usize,
    overrides: &ModelConfigOverrides,
) -> (String, usize) {
    let effective_window = overrides
        .model_context_window
        .map(|cw| cw.min(current_context_window))
        .unwrap_or(current_context_window);

    if overrides.strip_personality {
        base_instructions = strip_personality_section(&base_instructions);
    }

    (base_instructions, effective_window)
}

/// Removes the `# Personality` section from system instructions if present.
pub fn strip_personality_section(instructions: &str) -> String {
    let mut result = Vec::new();
    let mut in_personality_section = false;

    for line in instructions.lines() {
        if line.trim() == PERSONALITY_SECTION_HEADER {
            in_personality_section = true;
            continue;
        }

        if in_personality_section && line.starts_with("# ") {
            // Next major H1 section starts
            in_personality_section = false;
        }

        if !in_personality_section {
            result.push(line);
        }
    }

    result.join("\n")
}

/// Estimates dollar cost for token usage according to NVIDIA Build published pricing.
pub fn estimate_interaction_cost(
    model_slug: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
) -> Option<f64> {
    crate::models_identity::calculate_token_cost(model_slug, prompt_tokens, completion_tokens)
}

/// Validates whether a given model satisfies prerequisite capability constraints.
pub fn validate_capability_requirement(
    model: &crate::models::ModelInfo,
    requires_tools: bool,
    requires_vision: bool,
    requires_reasoning: bool,
) -> Result<(), String> {
    if requires_tools && !model.supports_tools {
        return Err(format!(
            "Model `{}` does not support tool calling",
            model.slug
        ));
    }
    if requires_vision && !model.supports_vision {
        return Err(format!(
            "Model `{}` does not support multimodal vision/image input",
            model.slug
        ));
    }
    if requires_reasoning && !model.supports_thinking {
        return Err(format!(
            "Model `{}` does not support reasoning/thinking tokens",
            model.slug
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_personality_section() {
        let text = "# Overview\nYou are M31A.\n\n# Personality\nBe warm and concise.\nUse friendly tone.\n\n# Rules\nNever break boundaries.\n";
        let stripped = strip_personality_section(text);
        assert!(stripped.contains("# Overview"));
        assert!(stripped.contains("# Rules"));
        assert!(!stripped.contains("# Personality"));
        assert!(!stripped.contains("Be warm and concise."));
    }

    #[test]
    fn test_apply_model_config_overrides_window_cap() {
        let text = "Base instructions".to_string();
        let overrides = ModelConfigOverrides {
            model_context_window: Some(64000),
            model_auto_compact_token_limit: Some(50000),
            tool_output_token_limit: Some(4000),
            strip_personality: false,
        };
        let (res_text, window) = apply_model_config_overrides(text, 128000, &overrides);
        assert_eq!(res_text, "Base instructions");
        assert_eq!(window, 64000);
    }

    #[test]
    fn test_validate_capability_requirement() {
        let model = crate::models::find_model("nvidia/nemotron-3.5-lightning-30b-a3b").unwrap();
        assert!(validate_capability_requirement(&model, true, false, false).is_ok());

        let vision_model = crate::models::find_model("meta/llama-3.2-11b-vision-instruct").unwrap();
        assert!(validate_capability_requirement(&vision_model, false, true, false).is_ok());

        assert!(validate_capability_requirement(&model, false, true, false).is_err());
    }

    #[test]
    fn test_estimate_interaction_cost() {
        let cost = estimate_interaction_cost("meta/codellama-70b", 500_000, 500_000);
        assert!(cost.is_some());
        assert!((cost.unwrap() - 0.70).abs() < 1e-4);
    }
}
