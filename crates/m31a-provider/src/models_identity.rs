//! Sovereign Model Identity Registry, Taxonomy, and Capabilities Catalog.
//!
//! Normalizes model identifiers across NVIDIA NIM, Ollama, LM Studio, DeepSeek,
//! Anthropic, and OpenAI, classifying model families, parameter tiers, maximum context
//! windows, output limits, tool support, and reasoning/thinking token semantics.

use serde::{Deserialize, Serialize};

/// Originating provider or vendor of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelVendor {
    NvidiaNim,
    OllamaLocal,
    LmStudioLocal,
    DeepSeek,
    Anthropic,
    OpenAI,
    Meta,
    Google,
    Ibm,
    Mistral,
    Microsoft,
    Qwen,
    ZeroOneAi,
    Adept,
    Ai21Labs,
    AiSingapore,
    BigCode,
    Databricks,
    MinimaxAi,
    MoonshotAi,
    Poolside,
    Snowflake,
    StepfunAi,
    Writer,
    ZAi,
    Zyphra,
    Custom,
}

impl ModelVendor {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::NvidiaNim => "NVIDIA NIM",
            Self::OllamaLocal => "Ollama (Local)",
            Self::LmStudioLocal => "LM Studio (Local)",
            Self::DeepSeek => "DeepSeek",
            Self::Anthropic => "Anthropic",
            Self::OpenAI => "OpenAI",
            Self::Meta => "Meta",
            Self::Google => "Google",
            Self::Ibm => "IBM",
            Self::Mistral => "Mistral AI",
            Self::Microsoft => "Microsoft",
            Self::Qwen => "Qwen",
            Self::ZeroOneAi => "01-ai",
            Self::Adept => "Adept",
            Self::Ai21Labs => "AI21 Labs",
            Self::AiSingapore => "AI Singapore",
            Self::BigCode => "BigCode",
            Self::Databricks => "Databricks",
            Self::MinimaxAi => "MiniMax",
            Self::MoonshotAi => "Moonshot AI",
            Self::Poolside => "Poolside",
            Self::Snowflake => "Snowflake",
            Self::StepfunAi => "StepFun",
            Self::Writer => "Writer",
            Self::ZAi => "Z-AI (GLM)",
            Self::Zyphra => "Zyphra",
            Self::Custom => "Custom Endpoint",
        }
    }
}

/// Architectural model family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelFamily {
    Llama,
    Llama3,
    DeepSeek,
    DeepSeekR1,
    DeepSeekV3,
    Qwen,
    Qwen2_5Coder,
    Claude,
    Claude3_5,
    Gpt,
    Gpt4o,
    Mistral,
    Gemma,
    Granite,
    Nemotron,
    Phi,
    Yi,
    Jamba,
    StarCoder,
    Dbrx,
    Zamba,
    Palmyra,
    Fuyu,
    Kosmos,
    Kimi,
    Generic,
}

impl ModelFamily {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Llama | Self::Llama3 => "Llama",
            Self::DeepSeek | Self::DeepSeekR1 | Self::DeepSeekV3 => "DeepSeek",
            Self::Qwen | Self::Qwen2_5Coder => "Qwen",
            Self::Claude | Self::Claude3_5 => "Claude",
            Self::Gpt | Self::Gpt4o => "GPT",
            Self::Mistral => "Mistral",
            Self::Gemma => "Gemma",
            Self::Granite => "Granite",
            Self::Nemotron => "Nemotron",
            Self::Phi => "Phi",
            Self::Yi => "Yi",
            Self::Jamba => "Jamba",
            Self::StarCoder => "StarCoder",
            Self::Dbrx => "DBRX",
            Self::Zamba => "Zamba",
            Self::Palmyra => "Palmyra",
            Self::Fuyu => "Fuyu",
            Self::Kosmos => "Kosmos",
            Self::Kimi => "Kimi",
            Self::Generic => "Generic",
        }
    }
}

/// Identity and capability specifications for an LLM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelIdentityInfo {
    pub id: String,
    pub canonical_name: String,
    pub vendor: ModelVendor,
    pub family: ModelFamily,
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub is_reasoning_model: bool,
}

impl ModelIdentityInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        canonical_name: impl Into<String>,
        vendor: ModelVendor,
        family: ModelFamily,
        context_window: usize,
        max_output_tokens: usize,
        supports_tools: bool,
        supports_vision: bool,
        is_reasoning_model: bool,
    ) -> Self {
        Self {
            id: id.into(),
            canonical_name: canonical_name.into(),
            vendor,
            family,
            context_window,
            max_output_tokens,
            supports_tools,
            supports_vision,
            is_reasoning_model,
        }
    }
}

/// Resolves an arbitrary model ID into a normalized `ModelIdentityInfo` descriptor.
pub fn resolve_model_identity(raw_id: &str) -> ModelIdentityInfo {
    let lower = raw_id.to_ascii_lowercase();

    // 0. Explicit local provider prefixes take precedence
    if lower.starts_with("ollama/") {
        let name = raw_id.strip_prefix("ollama/").unwrap_or(raw_id);
        return ModelIdentityInfo::new(
            raw_id,
            format!("Ollama Local ({})", name),
            ModelVendor::OllamaLocal,
            ModelFamily::Generic,
            32_768,
            4096,
            true,
            false,
            lower.contains("r1"),
        );
    }

    if lower.starts_with("lmstudio/") {
        let name = raw_id.strip_prefix("lmstudio/").unwrap_or(raw_id);
        return ModelIdentityInfo::new(
            raw_id,
            format!("LM Studio Local ({})", name),
            ModelVendor::LmStudioLocal,
            ModelFamily::Generic,
            32_768,
            4096,
            true,
            false,
            lower.contains("r1"),
        );
    }

    // 1. Exact catalog matches for well-known models (satisfying existing contracts)
    if lower == "meta/llama-3.3-70b-instruct" || lower == "llama-3.3-70b-instruct" {
        return ModelIdentityInfo::new(
            "meta/llama-3.3-70b-instruct",
            "Llama 3.3 70B Instruct",
            ModelVendor::NvidiaNim,
            ModelFamily::Llama3,
            131_072,
            4096,
            true,
            false,
            false,
        );
    }

    if lower.contains("deepseek-r1") || lower.contains("deepseek/r1") {
        return ModelIdentityInfo::new(
            "deepseek-ai/deepseek-r1",
            "DeepSeek R1 Reasoning Model",
            ModelVendor::DeepSeek,
            ModelFamily::DeepSeekR1,
            131_072,
            8192,
            true,
            false,
            true,
        );
    }

    if lower.contains("deepseek-v3") || lower.contains("deepseek/v3") {
        return ModelIdentityInfo::new(
            "deepseek-ai/deepseek-v3",
            "DeepSeek V3 Sovereign Coder",
            ModelVendor::DeepSeek,
            ModelFamily::DeepSeekV3,
            131_072,
            8192,
            true,
            false,
            false,
        );
    }

    if lower.contains("qwen2.5-coder") || lower.contains("qwen-2.5-coder") {
        return ModelIdentityInfo::new(
            "qwen/qwen2.5-coder-32b-instruct",
            "Qwen 2.5 Coder 32B Instruct",
            ModelVendor::Qwen,
            ModelFamily::Qwen2_5Coder,
            131_072,
            8192,
            true,
            false,
            false,
        );
    }

    if lower.contains("claude-3-5-sonnet") || lower.contains("claude-3.5-sonnet") {
        return ModelIdentityInfo::new(
            "anthropic/claude-3-5-sonnet-20241022",
            "Claude 3.5 Sonnet",
            ModelVendor::Anthropic,
            ModelFamily::Claude3_5,
            200_000,
            8192,
            true,
            true,
            false,
        );
    }

    if lower.contains("gpt-4o") {
        return ModelIdentityInfo::new(
            "openai/gpt-4o",
            "GPT-4o Omnimodal",
            ModelVendor::OpenAI,
            ModelFamily::Gpt4o,
            128_000,
            4096,
            true,
            true,
            false,
        );
    }

    // 2. Canonical catalog matching (covers all 85 models!)
    if let Some(canonical) = crate::models::find_canonical_model(raw_id) {
        let vendor = match canonical.publisher.as_str() {
            "nvidia" => ModelVendor::NvidiaNim,
            "meta" => ModelVendor::Meta,
            "google" => ModelVendor::Google,
            "deepseek-ai" => ModelVendor::DeepSeek,
            "mistralai" => ModelVendor::Mistral,
            "ibm" => ModelVendor::Ibm,
            "microsoft" => ModelVendor::Microsoft,
            "01-ai" => ModelVendor::ZeroOneAi,
            "adept" => ModelVendor::Adept,
            "ai21labs" => ModelVendor::Ai21Labs,
            "aisingapore" => ModelVendor::AiSingapore,
            "bigcode" => ModelVendor::BigCode,
            "databricks" => ModelVendor::Databricks,
            "minimaxai" => ModelVendor::MinimaxAi,
            "moonshotai" => ModelVendor::MoonshotAi,
            "openai" => ModelVendor::OpenAI,
            "poolside" => ModelVendor::Poolside,
            "snowflake" => ModelVendor::Snowflake,
            "stepfun-ai" => ModelVendor::StepfunAi,
            "writer" => ModelVendor::Writer,
            "z-ai" => ModelVendor::ZAi,
            "zyphra" => ModelVendor::Zyphra,
            _ => ModelVendor::Custom,
        };

        let family = if lower.contains("llama-3") || lower.contains("llama3") {
            ModelFamily::Llama3
        } else if lower.contains("llama") {
            ModelFamily::Llama
        } else if lower.contains("deepseek-r1") || lower.contains("r1") {
            ModelFamily::DeepSeekR1
        } else if lower.contains("deepseek-v3") || lower.contains("v3") {
            ModelFamily::DeepSeekV3
        } else if lower.contains("deepseek") {
            ModelFamily::DeepSeek
        } else if lower.contains("gemma") || lower.contains("codegemma") {
            ModelFamily::Gemma
        } else if lower.contains("granite") {
            ModelFamily::Granite
        } else if lower.contains("nemotron") {
            ModelFamily::Nemotron
        } else if lower.contains("phi") {
            ModelFamily::Phi
        } else if lower.contains("yi") {
            ModelFamily::Yi
        } else if lower.contains("jamba") {
            ModelFamily::Jamba
        } else if lower.contains("starcoder") {
            ModelFamily::StarCoder
        } else if lower.contains("dbrx") {
            ModelFamily::Dbrx
        } else if lower.contains("zamba") {
            ModelFamily::Zamba
        } else if lower.contains("palmyra") {
            ModelFamily::Palmyra
        } else if lower.contains("fuyu") {
            ModelFamily::Fuyu
        } else if lower.contains("kosmos") {
            ModelFamily::Kosmos
        } else if lower.contains("kimi") {
            ModelFamily::Kimi
        } else if lower.contains("mistral")
            || lower.contains("mixtral")
            || lower.contains("codestral")
        {
            ModelFamily::Mistral
        } else {
            ModelFamily::Generic
        };

        return ModelIdentityInfo::new(
            &canonical.id,
            &canonical.name,
            vendor,
            family,
            canonical.context_window.unwrap_or(65536) as usize,
            canonical.max_output_tokens.unwrap_or(4096) as usize,
            canonical.capabilities.tool_calling,
            canonical.capabilities.vision,
            canonical.capabilities.reasoning,
        );
    }

    // 3. Generic fallback for custom/unknown endpoints
    ModelIdentityInfo::new(
        raw_id,
        raw_id,
        ModelVendor::Custom,
        ModelFamily::Generic,
        32_768,
        4096,
        true,
        false,
        lower.contains("reason") || lower.contains("r1") || lower.contains("o1"),
    )
}

/// Checks whether a model emits reasoning/thinking tokens (e.g. DeepSeek R1, OpenAI o1/o3).
pub fn is_reasoning_model(model_id: &str) -> bool {
    resolve_model_identity(model_id).is_reasoning_model
}

/// Checks whether a model supports multimodal vision input.
pub fn is_vision_model(model_id: &str) -> bool {
    resolve_model_identity(model_id).supports_vision
}

/// Checks whether a model is specialized for code generation and analysis.
pub fn is_code_specialized(model_id: &str) -> bool {
    let lower = model_id.to_ascii_lowercase();
    lower.contains("code")
        || lower.contains("coder")
        || lower.contains("codestral")
        || lower.contains("starcoder")
}

/// Checks whether a model is an embedding model.
pub fn is_embedding_model(model_id: &str) -> bool {
    let lower = model_id.to_ascii_lowercase();
    lower.contains("embed") || lower.contains("retriever")
}

/// Checks whether a model is a guardrail or content safety model.
pub fn is_guardrail_model(model_id: &str) -> bool {
    let lower = model_id.to_ascii_lowercase();
    lower.contains("guard") || lower.contains("safety")
}

/// Looks up the official published token pricing for a given model.
pub fn get_model_pricing(model_id: &str) -> Option<crate::models::CanonicalPricing> {
    crate::models::find_canonical_model(model_id).map(|m| m.pricing.clone())
}

/// Calculates token cost in USD based on published NVIDIA Build pricing rates.
pub fn calculate_token_cost(
    model_id: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
) -> Option<f64> {
    let pricing = get_model_pricing(model_id)?;
    if pricing.status == "free_tier" {
        return Some(0.0);
    }
    let in_price = pricing.input_per_million?;
    let out_price = pricing.output_per_million?;

    let prompt_cost = (prompt_tokens as f64 / 1_000_000.0) * in_price;
    let completion_cost = (completion_tokens as f64 / 1_000_000.0) * out_price;

    Some(prompt_cost + completion_cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_well_known_models() {
        let llama = resolve_model_identity("meta/llama-3.3-70b-instruct");
        assert_eq!(llama.vendor, ModelVendor::NvidiaNim);
        assert_eq!(llama.family, ModelFamily::Llama3);
        assert_eq!(llama.context_window, 131_072);
        assert!(!llama.is_reasoning_model);

        let r1 = resolve_model_identity("deepseek-ai/deepseek-r1");
        assert_eq!(r1.vendor, ModelVendor::DeepSeek);
        assert_eq!(r1.family, ModelFamily::DeepSeekR1);
        assert!(r1.is_reasoning_model);

        let qwen = resolve_model_identity("qwen/qwen2.5-coder-32b-instruct");
        assert_eq!(qwen.vendor, ModelVendor::Qwen);
        assert_eq!(qwen.family, ModelFamily::Qwen2_5Coder);
        assert_eq!(qwen.max_output_tokens, 8192);

        let sonnet = resolve_model_identity("claude-3-5-sonnet-20241022");
        assert_eq!(sonnet.vendor, ModelVendor::Anthropic);
        assert!(sonnet.supports_vision);
        assert_eq!(sonnet.context_window, 200_000);
    }

    #[test]
    fn test_resolve_local_ollama_and_lmstudio() {
        let o = resolve_model_identity("ollama/deepseek-r1:14b");
        assert_eq!(o.vendor, ModelVendor::OllamaLocal);
        assert!(o.is_reasoning_model);

        let lm = resolve_model_identity("lmstudio/qwen2.5-coder-7b");
        assert_eq!(lm.vendor, ModelVendor::LmStudioLocal);
        assert_eq!(lm.context_window, 32_768);
    }

    #[test]
    fn test_resolve_canonical_catalog_models() {
        let gemma = resolve_model_identity("google/gemma-3-12b-it");
        assert_eq!(gemma.vendor, ModelVendor::Google);
        assert_eq!(gemma.family, ModelFamily::Gemma);

        let vision_model = resolve_model_identity("meta/llama-3.2-11b-vision-instruct");
        assert!(vision_model.supports_vision);

        let codellama = resolve_model_identity("meta/codellama-70b");
        assert_eq!(codellama.vendor, ModelVendor::Meta);
        assert_eq!(codellama.family, ModelFamily::Llama);
        assert_eq!(codellama.context_window, 131_072);

        let granite = resolve_model_identity("ibm/granite-34b-code-instruct");
        assert_eq!(granite.vendor, ModelVendor::Ibm);
        assert_eq!(granite.family, ModelFamily::Granite);
    }

    #[test]
    fn test_pricing_and_cost_calculation() {
        let pricing = get_model_pricing("meta/codellama-70b");
        assert!(pricing.is_some());
        let p = pricing.unwrap();
        assert_eq!(p.input_per_million, Some(0.70));
        assert_eq!(p.output_per_million, Some(0.70));

        let cost = calculate_token_cost("meta/codellama-70b", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        assert!((cost.unwrap() - 1.40).abs() < 1e-4);
    }
}
