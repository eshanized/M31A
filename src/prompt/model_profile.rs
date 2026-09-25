//! Strongly typed model capability descriptor for PromptOS prompt adaptation (Section 11, Artifacts F & G).
//!
//! # Core Invariant: "The model proposes. The runtime decides."
//!
//! `ModelProfile` represents trusted runtime facts about a model's operational characteristics,
//! physical context boundaries, and tool-calling fidelity.
//!
//! A `ModelProfile` is a capability descriptor, **never** an authority or privilege mechanism.
//! Models cannot self-report, elevate, or negotiate their own profile attributes.

use crate::context::tokenizer::calculate_output_headroom;
use crate::model::router::resolver::{ModelCandidate, ModelTier};
use crate::prompt::error::PromptError;
use crate::prompt::strategy::PromptStrategy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Strongly typed capability rating bounded between 1 (minimal) and 5 (maximal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityRating(u8);

impl CapabilityRating {
    /// Rating level 1: minimal capability.
    pub const ONE: Self = Self(1);
    /// Rating level 2: low / constrained capability.
    pub const TWO: Self = Self(2);
    /// Rating level 3: moderate capability.
    pub const THREE: Self = Self(3);
    /// Rating level 4: high / proficient capability.
    pub const FOUR: Self = Self(4);
    /// Rating level 5: maximal / frontier capability.
    pub const FIVE: Self = Self(5);

    /// Construct a new `CapabilityRating`, validating that the score is within 1..=5.
    pub fn new(val: u8) -> Result<Self, PromptError> {
        if (1..=5).contains(&val) {
            Ok(Self(val))
        } else {
            Err(PromptError::PromptInvalid {
                id: "capability_rating".to_string(),
                version: 1,
                reason: format!("capability rating must be between 1 and 5, found {val}"),
            })
        }
    }

    /// Construct a `CapabilityRating` clamping the score into the valid range 1..=5.
    pub fn clamped(val: u8) -> Self {
        Self(val.clamp(1, 5))
    }

    /// Return the raw integer score in 1..=5.
    pub fn as_u8(&self) -> u8 {
        self.0
    }

    /// Return the raw integer score in 1..=5 (alias for `as_u8`).
    pub fn score(&self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for CapabilityRating {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (1..=5).contains(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "capability rating must be between 1 and 5, found {value}"
            ))
        }
    }
}

impl From<CapabilityRating> for u8 {
    fn from(rating: CapabilityRating) -> Self {
        rating.0
    }
}

impl PartialEq<u8> for CapabilityRating {
    fn eq(&self, other: &u8) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<u8> for CapabilityRating {
    fn partial_cmp(&self, other: &u8) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl std::fmt::Display for CapabilityRating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for CapabilityRating {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for CapabilityRating {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        Self::try_from(val).map_err(serde::de::Error::custom)
    }
}

/// Degree of structured JSON output support provided by the model or provider gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputSupport {
    /// Provider supports native guaranteed JSON mode (e.g. OpenAI json_object / response_format).
    NativeJson,
    /// Provider enforces strict JSON Schema via grammar sampling (e.g. Ollama grammar / OpenAI json_schema).
    JsonSchema,
    /// Model does not support constrained decoding; requires emulated prompt formatting instructions.
    PromptEmulated,
}

impl StructuredOutputSupport {
    /// Return the canonical wire string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NativeJson => "native_json",
            Self::JsonSchema => "json_schema",
            Self::PromptEmulated => "prompt_emulated",
        }
    }

    /// Returns true if prompt-level format guidance must be injected.
    pub fn requires_emulated_guidance(&self) -> bool {
        matches!(self, Self::PromptEmulated)
    }
}

impl std::fmt::Display for StructuredOutputSupport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Strongly typed model capability descriptor for PromptOS (Section 11, Artifacts F & G).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProfile {
    /// Model identifier (e.g. "claude-3-7-sonnet", "llama-3.1-8b-instruct-q8").
    pub model_id: String,
    /// Upstream provider name (e.g. "anthropic", "ollama", "openai").
    pub provider: String,
    /// Human-readable display label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Physical context window capacity in tokens.
    pub context_capacity: usize,
    /// Maximum permitted output generation tokens.
    pub max_output_tokens: usize,
    /// Reasoning and cognitive strength rating (1..=5).
    pub reasoning_strength: CapabilityRating,
    /// Instruction-following fidelity rating (1..=5).
    pub instruction_following: CapabilityRating,
    /// Tool-calling fidelity rating (1..=5).
    pub tool_calling_fidelity: CapabilityRating,
    /// Code synthesis quality rating (1..=5).
    pub code_synthesis_quality: CapabilityRating,
    /// Level of structured output support available.
    pub structured_output_support: StructuredOutputSupport,
    /// Whether the model/provider reliably supports multi-tool or parallel tool dispatch.
    pub supports_parallel_tools: bool,
    /// Default prompt strategy when no override or condition matches.
    pub default_strategy: PromptStrategy,
    /// Cost in micros or points per million input tokens.
    #[serde(default)]
    pub cost_per_m_input: u64,
    /// Cost in micros or points per million output tokens.
    #[serde(default)]
    pub cost_per_m_output: u64,
    /// Arbitrary provider-specific runtime flags or metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl ModelProfile {
    /// Conservative fallback profile used when capabilities are unknown or unconfigured.
    pub fn conservative_default() -> Self {
        Self {
            model_id: "unknown-conservative".to_string(),
            provider: "default".to_string(),
            display_name: Some("Conservative Default Profile".to_string()),
            context_capacity: 16_384,
            max_output_tokens: 2048,
            reasoning_strength: CapabilityRating::TWO,
            instruction_following: CapabilityRating::TWO,
            tool_calling_fidelity: CapabilityRating::TWO,
            code_synthesis_quality: CapabilityRating::TWO,
            structured_output_support: StructuredOutputSupport::PromptEmulated,
            supports_parallel_tools: false,
            default_strategy: PromptStrategy::Constrained,
            cost_per_m_input: 0,
            cost_per_m_output: 0,
            metadata: BTreeMap::new(),
        }
    }

    /// Construct a profile representing a frontier hybrid-reasoning model (Normative Artifact F).
    pub fn strong_reasoning_default() -> Self {
        let mut metadata = BTreeMap::new();
        metadata.insert("recommended_temperature".to_string(), "0.1".to_string());
        metadata.insert(
            "extended_thinking_supported".to_string(),
            "true".to_string(),
        );

        Self {
            model_id: "claude-3-7-sonnet".to_string(),
            provider: "anthropic".to_string(),
            display_name: Some("Claude 3.7 Sonnet (Hybrid Reasoning)".to_string()),
            context_capacity: 200_000,
            max_output_tokens: 16_384,
            reasoning_strength: CapabilityRating::FIVE,
            instruction_following: CapabilityRating::FIVE,
            tool_calling_fidelity: CapabilityRating::FIVE,
            code_synthesis_quality: CapabilityRating::FIVE,
            structured_output_support: StructuredOutputSupport::NativeJson,
            supports_parallel_tools: true,
            default_strategy: PromptStrategy::Standard,
            cost_per_m_input: 3000,
            cost_per_m_output: 15000,
            metadata,
        }
    }

    /// Construct a profile representing a constrained local quantized model (Normative Artifact G).
    pub fn constrained_local_default() -> Self {
        let mut metadata = BTreeMap::new();
        metadata.insert("recommended_temperature".to_string(), "0.0".to_string());
        metadata.insert("enforce_json_grammar".to_string(), "true".to_string());
        metadata.insert("max_task_steps".to_string(), "15".to_string());

        Self {
            model_id: "llama-3.1-8b-instruct-q8".to_string(),
            provider: "ollama".to_string(),
            display_name: Some("Llama 3.1 8B Instruct (Local Quantized)".to_string()),
            context_capacity: 16_384,
            max_output_tokens: 4096,
            reasoning_strength: CapabilityRating::TWO,
            instruction_following: CapabilityRating::TWO,
            tool_calling_fidelity: CapabilityRating::THREE,
            code_synthesis_quality: CapabilityRating::THREE,
            structured_output_support: StructuredOutputSupport::JsonSchema,
            supports_parallel_tools: false,
            default_strategy: PromptStrategy::Constrained,
            cost_per_m_input: 0,
            cost_per_m_output: 0,
            metadata,
        }
    }

    /// Construct a `ModelProfile` by projecting runtime `ModelCandidate` metadata.
    pub fn from_candidate(candidate: &ModelCandidate) -> Self {
        let (reasoning, instruction, tool_fidelity, code_qual, default_strat) = match candidate.tier
        {
            ModelTier::Fast => (
                CapabilityRating::TWO,
                CapabilityRating::THREE,
                if candidate.supports_tools {
                    CapabilityRating::THREE
                } else {
                    CapabilityRating::ONE
                },
                CapabilityRating::THREE,
                PromptStrategy::Minimal,
            ),
            ModelTier::Standard => (
                CapabilityRating::FOUR,
                CapabilityRating::FOUR,
                if candidate.supports_tools {
                    CapabilityRating::FOUR
                } else {
                    CapabilityRating::ONE
                },
                CapabilityRating::FOUR,
                PromptStrategy::Standard,
            ),
            ModelTier::Reasoning => (
                CapabilityRating::FIVE,
                CapabilityRating::FIVE,
                if candidate.supports_tools {
                    CapabilityRating::FIVE
                } else {
                    CapabilityRating::ONE
                },
                CapabilityRating::FIVE,
                PromptStrategy::DeepReasoning,
            ),
        };

        let structured_output = if candidate.supports_structured_output {
            StructuredOutputSupport::NativeJson
        } else {
            StructuredOutputSupport::PromptEmulated
        };

        let max_output_tokens = calculate_output_headroom(candidate.tier);

        Self {
            model_id: candidate.model_id.clone(),
            provider: candidate.provider.clone(),
            display_name: candidate.display_name.clone(),
            context_capacity: candidate.context_capacity,
            max_output_tokens,
            reasoning_strength: reasoning,
            instruction_following: instruction,
            tool_calling_fidelity: tool_fidelity,
            code_synthesis_quality: code_qual,
            structured_output_support: structured_output,
            supports_parallel_tools: candidate.supports_tools && candidate.tier != ModelTier::Fast,
            default_strategy: default_strat,
            cost_per_m_input: candidate.cost_per_million_input,
            cost_per_m_output: candidate.cost_per_million_output,
            metadata: BTreeMap::new(),
        }
    }

    /// Returns true if the model is operating under tight context constraints (<= 16,384 tokens).
    pub fn is_context_constrained(&self) -> bool {
        self.context_capacity <= 16_384
    }

    /// Returns true if the model has constrained instruction-following capability (<= 2).
    pub fn is_instruction_constrained(&self) -> bool {
        self.instruction_following.as_u8() <= 2
    }

    /// Calculate maximum input tokens after reserving output headroom.
    pub fn max_input_tokens(&self) -> usize {
        self.context_capacity.saturating_sub(self.max_output_tokens)
    }

    /// Conservative byte ceiling corresponding to available input tokens (~3.5 bytes/token).
    pub fn max_input_bytes(&self) -> usize {
        self.max_input_tokens().saturating_mul(35) / 10
    }
}

impl Default for ModelProfile {
    fn default() -> Self {
        Self::conservative_default()
    }
}
