//! Two-stage deterministic model router with eligibility filtering and tier ranking (D-05, MDL-02).
//!
//! Stage 1: Derives model requirements from AgentProfile/RoutingRequest and applies hard
//! capability filters (tool calling, context capacity, circuit health, budget bounds).
//!
//! Stage 2: Deterministically ranks eligible candidates by tier suitability, operational health,
//! cost preference, and stable model ID tie-breaking.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::model::router::health::{CircuitBreakerRegistry, CircuitState};
use crate::model::types::ProviderCapabilityStatus;
use crate::state_machine::agent::AgentRole;
use thiserror::Error;

/// Fully resolved immutable model configuration ready for execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedModelSelection {
    pub provider: String,
    pub model_name: String,
    pub temperature: f32,
    pub max_tokens: usize,
}

/// Errors occurring during deterministic model resolution.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ModelResolutionError {
    #[error("no available model satisfies hard requirements: {0}")]
    NoEligibleModel(String),
}

fn default_candidate_availability() -> ProviderCapabilityStatus {
    ProviderCapabilityStatus::Available
}

fn default_candidate_source() -> String {
    "discovery".to_string()
}

/// Model capability and latency/cost tier (D-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    /// Low-latency, high-throughput models for simple tasks or fast iterations (e.g. 8B models).
    Fast,
    /// Balanced models suitable for coding, tool use, and general implementation (e.g. 70B models).
    Standard,
    /// Deep reasoning models with chain-of-thought capabilities (e.g. DeepSeek-R1, Nemotron).
    Reasoning,
}

impl std::fmt::Display for ModelTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fast => write!(f, "fast"),
            Self::Standard => write!(f, "standard"),
            Self::Reasoning => write!(f, "reasoning"),
        }
    }
}

/// Explicit state of a model capability (Section 13).
///
/// Distinguishes between confirmed support, confirmed lack of support, and unverified/unknown state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySupport {
    Supported,
    Unsupported,
    #[default]
    Unknown,
}

impl CapabilitySupport {
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported)
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

impl From<bool> for CapabilitySupport {
    fn from(b: bool) -> Self {
        if b {
            Self::Supported
        } else {
            Self::Unsupported
        }
    }
}

impl std::fmt::Display for CapabilitySupport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Supported => write!(f, "supported"),
            Self::Unsupported => write!(f, "unsupported"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Normalized classification of model kind / primary modality (Section 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    #[default]
    Unknown,
    TextGeneration,
    Chat,
    CodeGeneration,
    Reasoning,
    Embedding,
    Reranker,
    ImageGeneration,
    VisualLanguage,
    SpeechRecognition,
    SpeechSynthesis,
    GuardSafety,
    Reward,
}

impl ModelKind {
    pub fn is_embedding(&self) -> bool {
        matches!(self, Self::Embedding | Self::Reranker)
    }

    pub fn is_image_generation(&self) -> bool {
        matches!(self, Self::ImageGeneration)
    }

    pub fn is_safety_guard(&self) -> bool {
        matches!(self, Self::GuardSafety | Self::Reward)
    }

    pub fn is_text_or_code_generation(&self) -> bool {
        matches!(
            self,
            Self::TextGeneration
                | Self::Chat
                | Self::CodeGeneration
                | Self::Reasoning
                | Self::VisualLanguage
        )
    }
}

impl std::fmt::Display for ModelKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::TextGeneration => write!(f, "text_generation"),
            Self::Chat => write!(f, "chat"),
            Self::CodeGeneration => write!(f, "code_generation"),
            Self::Reasoning => write!(f, "reasoning"),
            Self::Embedding => write!(f, "embedding"),
            Self::Reranker => write!(f, "reranker"),
            Self::ImageGeneration => write!(f, "image_generation"),
            Self::VisualLanguage => write!(f, "visual_language"),
            Self::SpeechRecognition => write!(f, "speech_recognition"),
            Self::SpeechSynthesis => write!(f, "speech_synthesis"),
            Self::GuardSafety => write!(f, "guard_safety"),
            Self::Reward => write!(f, "reward"),
        }
    }
}

/// Context capacity breakdown distinguishing advertised, deployed, and effective limits (Section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContextLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advertised_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed_tokens: Option<usize>,
    pub effective_tokens: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<usize>,
}

/// Candidate model descriptor evaluated during model routing (D-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCandidate {
    pub model_id: String,
    pub provider: String,
    pub tier: ModelTier,
    pub context_capacity: usize,
    pub supports_tools: bool,
    pub supports_structured_output: bool,
    pub cost_per_million_input: u64,
    pub cost_per_million_output: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default = "default_candidate_availability")]
    pub availability: ProviderCapabilityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovered_at: Option<u64>,
    #[serde(default = "default_candidate_source")]
    pub source: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub model_kind: ModelKind,
    #[serde(default)]
    pub tool_support: CapabilitySupport,
    #[serde(default)]
    pub structured_output_support: CapabilitySupport,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modalities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_limits: Option<ContextLimits>,
}

impl ModelCandidate {
    /// Construct a candidate with default full capability support.
    pub fn new(
        model_id: impl Into<String>,
        provider: impl Into<String>,
        tier: ModelTier,
        context_capacity: usize,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            provider: provider.into(),
            tier,
            context_capacity,
            supports_tools: true,
            supports_structured_output: true,
            cost_per_million_input: 0,
            cost_per_million_output: 0,
            display_name: None,
            availability: ProviderCapabilityStatus::Available,
            discovered_at: None,
            source: "discovery".to_string(),
            metadata: HashMap::new(),
            model_kind: ModelKind::TextGeneration,
            tool_support: CapabilitySupport::Supported,
            structured_output_support: CapabilitySupport::Supported,
            modalities: vec!["text".to_string()],
            context_limits: if context_capacity > 0 {
                Some(ContextLimits {
                    advertised_tokens: Some(context_capacity),
                    deployed_tokens: None,
                    effective_tokens: context_capacity,
                    max_output_tokens: None,
                })
            } else {
                None
            },
        }
    }

    /// Builder to configure display name.
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// Builder to configure availability state.
    pub fn with_availability(mut self, status: ProviderCapabilityStatus) -> Self {
        self.availability = status;
        self
    }

    /// Builder to configure discovery timestamp.
    pub fn with_discovered_at(mut self, timestamp: u64) -> Self {
        self.discovered_at = Some(timestamp);
        self
    }

    /// Builder to configure provenance source.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Builder to configure additional metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Builder to configure tool calling support.
    pub fn with_tool_support(mut self, supports_tools: bool) -> Self {
        self.supports_tools = supports_tools;
        self.tool_support = if supports_tools {
            CapabilitySupport::Supported
        } else {
            CapabilitySupport::Unsupported
        };
        self
    }

    /// Builder to configure tool calling capability explicitly.
    pub fn with_tool_capability(mut self, cap: CapabilitySupport) -> Self {
        self.tool_support = cap;
        self.supports_tools = cap.is_supported();
        self
    }

    /// Builder to configure structured JSON output support.
    pub fn with_structured_output(mut self, supports_structured_output: bool) -> Self {
        self.supports_structured_output = supports_structured_output;
        self.structured_output_support = if supports_structured_output {
            CapabilitySupport::Supported
        } else {
            CapabilitySupport::Unsupported
        };
        self
    }

    /// Builder to configure structured output capability explicitly.
    pub fn with_structured_output_capability(mut self, cap: CapabilitySupport) -> Self {
        self.structured_output_support = cap;
        self.supports_structured_output = cap.is_supported();
        self
    }

    /// Builder to configure normalized model kind.
    pub fn with_model_kind(mut self, kind: ModelKind) -> Self {
        self.model_kind = kind;
        self
    }

    /// Builder to configure supported modalities.
    pub fn with_modalities(mut self, modalities: Vec<String>) -> Self {
        self.modalities = modalities;
        self
    }

    /// Builder to configure context limits.
    pub fn with_context_limits(mut self, limits: ContextLimits) -> Self {
        self.context_capacity = limits.effective_tokens;
        self.context_limits = Some(limits);
        self
    }

    /// Builder to configure pricing in micro-dollars ($ / 1M tokens).
    pub fn with_costs(mut self, cost_input: u64, cost_output: u64) -> Self {
        self.cost_per_million_input = cost_input;
        self.cost_per_million_output = cost_output;
        self
    }

    /// Builder to configure context provenance metadata.
    pub fn with_context_provenance(mut self, provenance: impl Into<String>) -> Self {
        self.metadata
            .insert("context_provenance".to_string(), provenance.into());
        self
    }

    /// Mutate context provenance metadata in place.
    pub fn set_context_provenance(&mut self, provenance: impl Into<String>) {
        self.metadata
            .insert("context_provenance".to_string(), provenance.into());
    }

    /// Read context capacity provenance metadata, if recorded.
    pub fn context_provenance(&self) -> Option<&str> {
        self.metadata.get("context_provenance").map(|s| s.as_str())
    }

    /// Check if context capacity was explicitly discovered from provider metadata.
    pub fn is_context_known(&self) -> bool {
        self.context_capacity > 0 && self.context_provenance() != Some("unknown")
    }

    /// Check if model is an embedding or retrieval reranking model.
    pub fn is_embedding(&self) -> bool {
        if self.model_kind.is_embedding() {
            return true;
        }
        let id_lower = self.model_id.to_lowercase();
        id_lower.contains("embed") || id_lower.contains("rerank")
    }

    /// Check if model is an image generation model.
    pub fn is_image_generation(&self) -> bool {
        if self.model_kind.is_image_generation() {
            return true;
        }
        let id_lower = self.model_id.to_lowercase();
        id_lower.contains("diffusion") || id_lower.contains("stable-diffusion")
    }

    /// Check if model is a safety guard or reward model.
    pub fn is_safety_guard(&self) -> bool {
        if self.model_kind.is_safety_guard() {
            return true;
        }
        let id_lower = self.model_id.to_lowercase();
        (id_lower.contains("guard") && !id_lower.contains("instruct"))
            || id_lower.contains("reward")
    }

    /// Read advertised context limit, if available.
    pub fn advertised_context(&self) -> Option<usize> {
        self.context_limits.and_then(|l| l.advertised_tokens)
    }

    /// Read deployed context limit, if available.
    pub fn deployed_context(&self) -> Option<usize> {
        self.context_limits.and_then(|l| l.deployed_tokens)
    }

    /// Read effective context limit used for routing and budgeting.
    pub fn effective_context(&self) -> usize {
        self.context_capacity
    }
}

/// Routing request specifying agent profile, capability, context, and budget requirements (D-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingRequest {
    pub agent_role: AgentRole,
    pub preferred_tier: ModelTier,
    pub required_context_tokens: usize,
    pub requires_tool_calling: bool,
    pub budget_token_limit: usize,
}

impl RoutingRequest {
    /// Create a standard routing request for a role and preferred tier.
    pub fn new(agent_role: AgentRole, preferred_tier: ModelTier) -> Self {
        Self {
            agent_role,
            preferred_tier,
            required_context_tokens: 4096,
            requires_tool_calling: true,
            budget_token_limit: 128_000,
        }
    }

    pub fn with_context_tokens(mut self, tokens: usize) -> Self {
        self.required_context_tokens = tokens;
        self
    }

    pub fn with_tool_calling(mut self, requires: bool) -> Self {
        self.requires_tool_calling = requires;
        self
    }

    pub fn with_budget_token_limit(mut self, limit: usize) -> Self {
        self.budget_token_limit = limit;
        self
    }
}

/// Detailed resolution metadata produced by the two-stage router for telemetry and audit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelResolutionDetails {
    pub selection: ResolvedModelSelection,
    pub routing_reason: String,
    pub evaluated_count: usize,
    pub eligible_count: usize,
    pub score: i64,
}

/// Role-specific temperature defaults.
///
/// Single authority for per-role temperature is the role registry
/// (`RoleRegistry::temperature_for`). This function is a thin adapter, not a
/// second table — no role may gain a divergent literal here.
pub fn role_default_temperature(role: AgentRole) -> f32 {
    crate::agent::registry::RoleRegistry::global()
        .read()
        .map(|guard| guard.temperature_for(&role))
        .unwrap_or(crate::agent::registry::DEFAULT_ROLE_TEMPERATURE_MILLICELSIUS as f32 / 1000.0)
}

/// Two-stage deterministic Model Router (D-05, MDL-02).
#[derive(Debug, Clone, Default)]
pub struct ModelRouter;

impl ModelRouter {
    /// Create a new ModelRouter instance.
    pub fn new() -> Self {
        Self
    }

    /// Resolve a model selection deterministically against candidate pool and live circuit health.
    pub fn resolve_model(
        &self,
        request: &RoutingRequest,
        candidates: &[ModelCandidate],
        health_registry: &CircuitBreakerRegistry,
    ) -> Result<ResolvedModelSelection, ModelResolutionError> {
        let details = self.resolve_model_with_details(request, candidates, health_registry)?;
        Ok(details.selection)
    }

    /// Resolve a model selection and produce rich audit/telemetry details.
    pub fn resolve_model_with_details(
        &self,
        request: &RoutingRequest,
        candidates: &[ModelCandidate],
        health_registry: &CircuitBreakerRegistry,
    ) -> Result<ModelResolutionDetails, ModelResolutionError> {
        if candidates.is_empty() {
            return Err(ModelResolutionError::NoEligibleModel(
                "no model candidates configured in candidate pool".to_string(),
            ));
        }

        // =====================================================================
        // STAGE 1: Hard Eligibility Filter
        // =====================================================================
        let mut eligible: Vec<(&ModelCandidate, CircuitState)> = Vec::new();
        let mut disqualifications: Vec<String> = Vec::new();

        for candidate in candidates {
            // Check availability status (WS-I §1, Dynamic Discovery §8)
            if candidate.availability != ProviderCapabilityStatus::Available {
                disqualifications.push(format!(
                    "{} rejected: status is {}",
                    candidate.model_id, candidate.availability
                ));
                continue;
            }

            // Check non-coding model kinds (embeddings, image-generation, safety-guards)
            if candidate.is_embedding() {
                disqualifications.push(format!(
                    "{} rejected: embedding models cannot execute reasoning/agent tasks",
                    candidate.model_id
                ));
                continue;
            }
            if candidate.is_image_generation() {
                disqualifications.push(format!(
                    "{} rejected: image generation models cannot execute reasoning/agent tasks",
                    candidate.model_id
                ));
                continue;
            }
            if candidate.is_safety_guard() {
                disqualifications.push(format!(
                    "{} rejected: safety guard/reward models cannot execute reasoning/agent tasks",
                    candidate.model_id
                ));
                continue;
            }

            // Check tool calling support requirement (fails closed on unknown or unsupported)
            if request.requires_tool_calling {
                if candidate.tool_support == CapabilitySupport::Unknown {
                    disqualifications.push(format!(
                        "{} rejected: tool calling capability is unknown/unverified",
                        candidate.model_id
                    ));
                    continue;
                }
                if !candidate.supports_tools
                    || candidate.tool_support == CapabilitySupport::Unsupported
                {
                    disqualifications.push(format!(
                        "{} rejected: lacks required tool calling support",
                        candidate.model_id
                    ));
                    continue;
                }
            }

            // Check context capacity requirement
            if candidate.context_capacity < request.required_context_tokens {
                disqualifications.push(format!(
                    "{} rejected: context capacity {} < required {}",
                    candidate.model_id, candidate.context_capacity, request.required_context_tokens
                ));
                continue;
            }

            // Check live circuit breaker health (Open circuits are ineligible).
            // Admission-aware query: an expired cooldown transitions to HalfOpen
            // here instead of disqualifying forever on a stale Open snapshot.
            let health = {
                let state_by_endpoint =
                    health_registry.admission_state(&candidate.provider, &candidate.model_id);
                if state_by_endpoint != CircuitState::Healthy {
                    state_by_endpoint
                } else {
                    health_registry.admission_state_for_model(&candidate.model_id)
                }
            };

            if health == CircuitState::Open {
                disqualifications.push(format!(
                    "{} rejected: circuit breaker is Open (cooling down)",
                    candidate.model_id
                ));
                continue;
            }

            eligible.push((candidate, health));
        }

        if eligible.is_empty() {
            return Err(ModelResolutionError::NoEligibleModel(format!(
                "all {} candidates disqualified: {}",
                candidates.len(),
                disqualifications.join("; ")
            )));
        }

        // =====================================================================
        // STAGE 2: Deterministic Ranking
        // =====================================================================
        // Scoring formula:
        // Tier Match:
        //   - Exact preferred tier match: +1000
        //   - Reasoning preferred: Standard (+500), Fast (+100)
        //   - Standard preferred: Fast (+600), Reasoning (+400)
        //   - Fast preferred: Standard (+500), Reasoning (+200)
        // Health Status:
        //   - Healthy: +200
        //   - Degraded: +50
        //   - HalfOpen: +20
        // Cost: lower cost is preferred (cost score subtracted or ranked)
        // Tie-breaker: stable lexicographical ordering on model_id
        let mut scored_candidates: Vec<_> = eligible
            .into_iter()
            .map(|(candidate, health)| {
                let tier_score = if request.preferred_tier == candidate.tier {
                    1000
                } else {
                    match (request.preferred_tier, candidate.tier) {
                        (ModelTier::Reasoning, ModelTier::Standard) => 500,
                        (ModelTier::Reasoning, ModelTier::Fast) => 100,
                        (ModelTier::Standard, ModelTier::Fast) => 600,
                        (ModelTier::Standard, ModelTier::Reasoning) => 400,
                        (ModelTier::Fast, ModelTier::Standard) => 500,
                        (ModelTier::Fast, ModelTier::Reasoning) => 200,
                        _ => 0,
                    }
                };

                let health_score = match health {
                    CircuitState::Healthy => 200,
                    CircuitState::Degraded => 50,
                    CircuitState::HalfOpen => 20,
                    CircuitState::Open => 0,
                };

                let total_score = tier_score + health_score;
                let total_cost =
                    candidate.cost_per_million_input + candidate.cost_per_million_output;

                // Sort key:
                // (-total_score, total_cost, model_id)
                // Minimizing this tuple maximizes score, minimizes cost, and breaks ties lexicographically.
                (
                    (-total_score, total_cost, candidate.model_id.as_str()),
                    candidate,
                    health,
                    total_score,
                )
            })
            .collect();

        scored_candidates.sort_by(|a, b| a.0.cmp(&b.0));

        let (_, winner, winner_health, final_score) = scored_candidates
            .first()
            .expect("eligible candidates verified non-empty");

        let temperature = role_default_temperature(request.agent_role.clone());
        let max_tokens = winner.context_capacity;

        let routing_reason = format!(
            "tier={:?} (preferred={:?}), health={:?}, cost=${}/1M, role={:?}, eligible={}/{}",
            winner.tier,
            request.preferred_tier,
            winner_health,
            winner.cost_per_million_input + winner.cost_per_million_output,
            request.agent_role,
            scored_candidates.len(),
            candidates.len()
        );

        let selection = ResolvedModelSelection {
            provider: winner.provider.clone(),
            model_name: winner.model_id.clone(),
            temperature,
            max_tokens,
        };

        Ok(ModelResolutionDetails {
            selection,
            routing_reason,
            evaluated_count: candidates.len(),
            eligible_count: scored_candidates.len(),
            score: *final_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::ModelError;

    fn sample_candidates() -> Vec<ModelCandidate> {
        vec![
            ModelCandidate::new(
                "meta/llama-3.1-8b-instruct",
                "nvidia",
                ModelTier::Fast,
                8192,
            )
            .with_costs(200, 200),
            ModelCandidate::new(
                "meta/llama-3.3-70b-instruct",
                "nvidia",
                ModelTier::Standard,
                32768,
            )
            .with_costs(800, 800),
            ModelCandidate::new(
                "deepseek-ai/deepseek-r1",
                "nvidia",
                ModelTier::Reasoning,
                65536,
            )
            .with_costs(2000, 2000),
        ]
    }

    #[test]
    fn test_hard_filter_eliminates_insufficient_context() {
        let router = ModelRouter::new();
        let registry = CircuitBreakerRegistry::new();
        let candidates = sample_candidates();

        // Request 16384 context tokens; 8b has only 8192, so it must be eliminated
        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Fast)
            .with_context_tokens(16384);

        let details = router
            .resolve_model_with_details(&request, &candidates, &registry)
            .unwrap();

        // Winner cannot be 8b because 8192 < 16384
        assert_ne!(details.selection.model_name, "meta/llama-3.1-8b-instruct");
        // Standard (70b) satisfies 16384 context and is ranked highest among eligible
        assert_eq!(details.selection.model_name, "meta/llama-3.3-70b-instruct");
        assert_eq!(details.eligible_count, 2);
    }

    #[test]
    fn test_hard_filter_eliminates_missing_tool_support() {
        let router = ModelRouter::new();
        let registry = CircuitBreakerRegistry::new();
        let mut candidates = sample_candidates();
        // Disallow tool support for the 70B model
        candidates[1].supports_tools = false;

        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Standard)
            .with_tool_calling(true);

        let selection = router
            .resolve_model(&request, &candidates, &registry)
            .unwrap();

        // 70B disqualified for lacking tools; router falls back to Fast (8b) or Reasoning
        assert_ne!(selection.model_name, "meta/llama-3.3-70b-instruct");
    }

    #[test]
    fn test_circuit_breaker_open_rejected_in_stage_1() {
        let router = ModelRouter::new();
        let registry = CircuitBreakerRegistry::new();
        let candidates = sample_candidates();

        // Trip circuit breaker for the 8B model into Open
        registry.record_failure(
            "nvidia",
            "meta/llama-3.1-8b-instruct",
            &ModelError::RateLimited { cooldown_secs: 60 },
        );
        assert_eq!(
            registry.get_state_for_model("meta/llama-3.1-8b-instruct"),
            CircuitState::Open
        );

        // Request preferred tier Fast
        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Fast);

        let details = router
            .resolve_model_with_details(&request, &candidates, &registry)
            .unwrap();

        // 8B is Open, so it must be rejected even though it was the preferred tier
        assert_ne!(details.selection.model_name, "meta/llama-3.1-8b-instruct");
        assert_eq!(details.eligible_count, 2);
    }

    /// Verifies that an expired cooldown re-admits the model through resolution
    /// (HalfOpen probe) instead of disqualifying forever on a stale Open snapshot.
    #[test]
    fn test_expired_cooldown_readmits_model_in_stage_1() {
        let router = ModelRouter::new();
        let registry = CircuitBreakerRegistry::new();
        let candidates = sample_candidates();

        // Trip the 8B breaker 20s ago with a 10s cooldown: expired.
        let past = chrono::Utc::now() - chrono::Duration::seconds(20);
        registry.record_failure_at(
            "nvidia",
            "meta/llama-3.1-8b-instruct",
            &ModelError::RateLimited { cooldown_secs: 10 },
            past,
        );

        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Fast);
        let details = router
            .resolve_model_with_details(&request, &candidates, &registry)
            .unwrap();

        // 8B is HalfOpen-eligible again and wins its preferred tier.
        assert_eq!(details.selection.model_name, "meta/llama-3.1-8b-instruct");
        assert_eq!(details.eligible_count, 3);
    }

    #[test]
    fn test_role_temperature_follows_profile_registry() {
        // The router must not keep a shadow temperature table.
        // Values below mirror AgentProfile::built_in temperatures.
        assert_eq!(role_default_temperature(AgentRole::planner()), 0.2);
        assert_eq!(role_default_temperature(AgentRole::researcher()), 0.2);
        assert_eq!(role_default_temperature(AgentRole::verifier()), 0.05);
        assert_eq!(
            role_default_temperature(AgentRole::discovery_analyst()),
            0.15
        );
        assert_eq!(role_default_temperature(AgentRole::implementer()), 0.1);
    }

    #[test]
    fn test_tier_matching_ranks_preferred_tier_first() {
        let router = ModelRouter::new();
        let registry = CircuitBreakerRegistry::new();
        let candidates = sample_candidates();

        // Request Reasoning tier
        let request_reasoning = RoutingRequest::new(AgentRole::architect(), ModelTier::Reasoning);
        let selection_reasoning = router
            .resolve_model(&request_reasoning, &candidates, &registry)
            .unwrap();
        assert_eq!(selection_reasoning.model_name, "deepseek-ai/deepseek-r1");
        // Single temperature authority: the router converts the
        // profile registry value (Architect: 100 mC) instead of keeping a
        // shadow table.
        assert_eq!(selection_reasoning.temperature, 0.1);

        // Request Fast tier
        let request_fast = RoutingRequest::new(AgentRole::implementer(), ModelTier::Fast);
        let selection_fast = router
            .resolve_model(&request_fast, &candidates, &registry)
            .unwrap();
        assert_eq!(selection_fast.model_name, "meta/llama-3.1-8b-instruct");
        assert_eq!(selection_fast.temperature, 0.1);
    }

    #[test]
    fn test_tie_breaking_is_completely_deterministic() {
        let router = ModelRouter::new();
        let registry = CircuitBreakerRegistry::new();

        // Two candidates with identical tier, context, capability, and cost
        let candidates = vec![
            ModelCandidate::new("model-zebra", "nvidia", ModelTier::Standard, 32768),
            ModelCandidate::new("model-alpha", "nvidia", ModelTier::Standard, 32768),
        ];

        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Standard);

        // Run 50 iterations to ensure absolute determinism
        for _ in 0..50 {
            let selection = router
                .resolve_model(&request, &candidates, &registry)
                .unwrap();
            // Lexicographical ordering dictates model-alpha must always win
            assert_eq!(selection.model_name, "model-alpha");
        }
    }

    #[test]
    fn test_fails_closed_when_all_candidates_disqualified() {
        let router = ModelRouter::new();
        let registry = CircuitBreakerRegistry::new();
        let candidates = vec![
            ModelCandidate::new("tiny-model", "nvidia", ModelTier::Fast, 2048)
                .with_tool_support(false),
        ];

        // Request requiring 4096 context and tools
        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Fast)
            .with_context_tokens(4096)
            .with_tool_calling(true);

        let err = router
            .resolve_model(&request, &candidates, &registry)
            .unwrap_err();
        assert!(matches!(err, ModelResolutionError::NoEligibleModel(_)));
    }
}
