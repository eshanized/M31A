//! Multi-model dynamic hybrid router and task complexity classifier.
//!
//! Elevates M31A beyond single-vendor lock-in:
//! - Classifies tasks into Fast, Balanced, Coding, DeepReasoning, Vision, Guardrail, and Embedding tiers.
//! - Routes tasks dynamically to NVIDIA NIM, local Ollama/vLLM/LM Studio, or custom providers.
//! - Automatic graceful failover ladders to backup models on rate limits or service unavailability.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ModelTier {
    /// Low-latency, lightweight models for symbol navigation, outlines, and quick status checks.
    Fast,
    /// High-throughput general-purpose models for standard agent orchestration and turn execution.
    Balanced,
    /// Specialized high-accuracy code synthesis, refactoring, and patch generation models.
    Coding,
    /// Deep-reasoning models for complex architectural design, hard debugging, and self-healing.
    DeepReasoning,
    /// Multimodal vision models for UI inspection, screenshot analysis, and visual design verification.
    Vision,
    /// Content safety and security guardrail models.
    Guardrail,
    /// Vector embedding models for retrieval and memory search.
    Embedding,
}

impl ModelTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::Coding => "coding",
            Self::DeepReasoning => "deep_reasoning",
            Self::Vision => "vision",
            Self::Guardrail => "guardrail",
            Self::Embedding => "embedding",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RouteModelConfig {
    pub model_id: String,
    pub provider_label: String,
    pub max_tokens: usize,
    pub context_window: usize,
}

#[derive(Debug, Clone)]
pub struct HybridModelRouter {
    tier_mapping: HashMap<ModelTier, RouteModelConfig>,
    fallback_models: Vec<RouteModelConfig>,
    tier_fallbacks: HashMap<ModelTier, Vec<RouteModelConfig>>,
}

impl Default for HybridModelRouter {
    fn default() -> Self {
        let mut tier_mapping = HashMap::new();
        tier_mapping.insert(
            ModelTier::Fast,
            RouteModelConfig {
                model_id: "meta/llama-3.1-8b-instruct".to_string(),
                provider_label: "nvidia_nim".to_string(),
                max_tokens: 4096,
                context_window: 128000,
            },
        );
        tier_mapping.insert(
            ModelTier::Balanced,
            RouteModelConfig {
                model_id: "meta/llama-3.3-70b-instruct".to_string(),
                provider_label: "nvidia_nim".to_string(),
                max_tokens: 8192,
                context_window: 128000,
            },
        );
        tier_mapping.insert(
            ModelTier::Coding,
            RouteModelConfig {
                model_id: "mistralai/codestral-22b-instruct-v0.1".to_string(),
                provider_label: "nvidia_nim".to_string(),
                max_tokens: 8192,
                context_window: 65536,
            },
        );
        tier_mapping.insert(
            ModelTier::DeepReasoning,
            RouteModelConfig {
                model_id: "deepseek-ai/deepseek-r1".to_string(),
                provider_label: "nvidia_nim".to_string(),
                max_tokens: 16384,
                context_window: 128000,
            },
        );
        tier_mapping.insert(
            ModelTier::Vision,
            RouteModelConfig {
                model_id: "meta/llama-3.2-11b-vision-instruct".to_string(),
                provider_label: "nvidia_nim".to_string(),
                max_tokens: 4096,
                context_window: 65536,
            },
        );
        tier_mapping.insert(
            ModelTier::Guardrail,
            RouteModelConfig {
                model_id: "nvidia/llama-3.1-nemoguard-8b-content-safety".to_string(),
                provider_label: "nvidia_nim".to_string(),
                max_tokens: 4096,
                context_window: 131072,
            },
        );
        tier_mapping.insert(
            ModelTier::Embedding,
            RouteModelConfig {
                model_id: "nvidia/nv-embedqa-mistral-7b-v2".to_string(),
                provider_label: "nvidia_nim".to_string(),
                max_tokens: 4096,
                context_window: 65536,
            },
        );

        let fallback_models = vec![
            RouteModelConfig {
                model_id: "nvidia/llama-3.1-nemotron-70b-instruct".to_string(),
                provider_label: "nvidia_nim".to_string(),
                max_tokens: 8192,
                context_window: 128000,
            },
            RouteModelConfig {
                model_id: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
                provider_label: "nvidia_nim".to_string(),
                max_tokens: 4096,
                context_window: 131072,
            },
            RouteModelConfig {
                model_id: "qwen2.5-coder:32b".to_string(),
                provider_label: "ollama_local".to_string(),
                max_tokens: 8192,
                context_window: 32768,
            },
        ];

        let mut tier_fallbacks = HashMap::new();
        tier_fallbacks.insert(
            ModelTier::Coding,
            vec![
                RouteModelConfig {
                    model_id: "meta/codellama-70b".to_string(),
                    provider_label: "nvidia_nim".to_string(),
                    max_tokens: 4096,
                    context_window: 131072,
                },
                RouteModelConfig {
                    model_id: "ibm/granite-34b-code-instruct".to_string(),
                    provider_label: "nvidia_nim".to_string(),
                    max_tokens: 4096,
                    context_window: 65536,
                },
            ],
        );
        tier_fallbacks.insert(
            ModelTier::DeepReasoning,
            vec![
                RouteModelConfig {
                    model_id: "deepseek-ai/deepseek-v4.1-flash".to_string(),
                    provider_label: "nvidia_nim".to_string(),
                    max_tokens: 8192,
                    context_window: 1048576,
                },
                RouteModelConfig {
                    model_id: "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning".to_string(),
                    provider_label: "nvidia_nim".to_string(),
                    max_tokens: 4096,
                    context_window: 131072,
                },
            ],
        );
        tier_fallbacks.insert(
            ModelTier::Vision,
            vec![
                RouteModelConfig {
                    model_id: "meta/llama-3.2-90b-vision-instruct".to_string(),
                    provider_label: "nvidia_nim".to_string(),
                    max_tokens: 4096,
                    context_window: 65536,
                },
                RouteModelConfig {
                    model_id: "microsoft/phi-3-vision-128k-instruct".to_string(),
                    provider_label: "nvidia_nim".to_string(),
                    max_tokens: 4096,
                    context_window: 131072,
                },
            ],
        );

        Self {
            tier_mapping,
            fallback_models,
            tier_fallbacks,
        }
    }
}

impl HybridModelRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_tier_model(&mut self, tier: ModelTier, config: RouteModelConfig) {
        self.tier_mapping.insert(tier, config);
    }

    /// Selects the optimal model config for a given task tier.
    pub fn route_for_tier(&self, tier: ModelTier) -> RouteModelConfig {
        self.tier_mapping
            .get(&tier)
            .cloned()
            .unwrap_or_else(|| self.tier_mapping[&ModelTier::Balanced].clone())
    }

    /// Automatically classifies task complexity and routes to the appropriate model.
    pub fn route_task(
        &self,
        task_prompt: &str,
        tool_call_context: Option<&str>,
    ) -> RouteModelConfig {
        let tier = classify_task_tier(task_prompt, tool_call_context);
        self.route_for_tier(tier)
    }

    /// Dynamically constructs router configured for a specific provider.
    pub fn with_provider(provider_label: impl Into<String>) -> Self {
        let label = provider_label.into();
        let mut router = Self::default();
        for cfg in router.tier_mapping.values_mut() {
            cfg.provider_label = label.clone();
        }
        for cfg in router.fallback_models.iter_mut() {
            if cfg.provider_label != "ollama_local" {
                cfg.provider_label = label.clone();
            }
        }
        for ladder in router.tier_fallbacks.values_mut() {
            for cfg in ladder.iter_mut() {
                cfg.provider_label = label.clone();
            }
        }
        router
    }

    /// Finds which tier a model is mapped to (if any).
    pub fn tier_for_model(&self, model_id: &str) -> Option<ModelTier> {
        self.tier_mapping
            .iter()
            .find(|(_, cfg)| cfg.model_id == model_id)
            .map(|(tier, _)| *tier)
    }

    /// Resolves fallback model when the primary model returns rate-limits or fails.
    pub fn fallback_for_failure(&self, failed_model_id: &str) -> Option<RouteModelConfig> {
        self.fallback_for_failure_tier(failed_model_id, None)
    }

    /// Resolves fallback model for a specific failed model and optional request tier.
    /// Follows a deterministic capability-bound fallback ladder.
    pub fn fallback_for_failure_tier(
        &self,
        failed_model_id: &str,
        tier: Option<ModelTier>,
    ) -> Option<RouteModelConfig> {
        let target_tier = tier.or_else(|| self.tier_for_model(failed_model_id));
        if let Some(tier_fallbacks) = target_tier.and_then(|t| self.tier_fallbacks.get(&t)) {
            for m in tier_fallbacks {
                if m.model_id != failed_model_id {
                    return Some(m.clone());
                }
            }
        }
        self.fallback_models
            .iter()
            .find(|m| m.model_id != failed_model_id)
            .cloned()
    }

    /// Returns the complete fallback ladder for a specific model tier.
    pub fn fallback_ladder_for_tier(&self, tier: ModelTier) -> Vec<RouteModelConfig> {
        let mut ladder = Vec::new();
        if let Some(primary) = self.tier_mapping.get(&tier) {
            ladder.push(primary.clone());
        }
        if let Some(tier_specific) = self.tier_fallbacks.get(&tier) {
            ladder.extend(tier_specific.clone());
        }
        ladder.extend(self.fallback_models.clone());
        ladder
    }
}

/// Heuristic classifier determining whether a task requires Fast, Balanced, Coding, DeepReasoning, or Vision.
pub fn classify_task_tier(prompt: &str, tool_name: Option<&str>) -> ModelTier {
    if let Some(tool) = tool_name {
        match tool {
            "outline_file" | "find_definition" | "find_references" | "git_status"
            | "current_time" => {
                return ModelTier::Fast;
            }
            "diagnose_compiler_errors" => {
                return ModelTier::DeepReasoning;
            }
            "view_image" | "inspect_image" | "render_screenshot" => {
                return ModelTier::Vision;
            }
            "guardian_review" | "security_scan" => {
                return ModelTier::Guardrail;
            }
            "apply_patch" => {
                return ModelTier::Coding;
            }
            _ => {}
        }
    }

    let lower = prompt.to_lowercase();

    // 1. Deep reasoning triggers
    let deep_reasoning_keywords = [
        "debug",
        "deadlock",
        "race condition",
        "memory leak",
        "architect",
        "redesign",
        "panic",
        "segfault",
        "compiler error",
        "self-healing",
        "solve",
        "mathematical",
        "formal proof",
    ];

    for kw in deep_reasoning_keywords {
        if lower.contains(kw) {
            return ModelTier::DeepReasoning;
        }
    }

    // 2. Vision triggers
    let vision_keywords = [
        "screenshot",
        "view image",
        "inspect image",
        "diagram",
        "visual ui",
        "wireframe",
        "mockup",
        "look at the image",
        "look at this image",
        "picture",
    ];

    for kw in vision_keywords {
        if lower.contains(kw) {
            return ModelTier::Vision;
        }
    }

    // 3. Fast triggers
    let fast_keywords = [
        "what time",
        "list files",
        "status",
        "format",
        "rename",
        "typo",
        "check line",
    ];

    for kw in fast_keywords {
        if lower.contains(kw) {
            return ModelTier::Fast;
        }
    }

    // 4. Coding triggers (explicit coding requests)
    let coding_keywords = [
        "refactor",
        "unit test",
        "write tests",
        "generate code",
        "implement function",
    ];

    for kw in coding_keywords {
        if lower.contains(kw) {
            return ModelTier::Coding;
        }
    }

    ModelTier::Balanced
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_tier_classification() {
        assert_eq!(
            classify_task_tier(
                "Please debug this race condition in the async channel",
                None
            ),
            ModelTier::DeepReasoning
        );
        assert_eq!(
            classify_task_tier("Format this function and fix typo in comment", None),
            ModelTier::Fast
        );
        assert_eq!(
            classify_task_tier("Implement user authentication route with JWT tokens", None),
            ModelTier::Balanced
        );
        assert_eq!(
            classify_task_tier("Run check", Some("outline_file")),
            ModelTier::Fast
        );
        assert_eq!(
            classify_task_tier("Check errors", Some("diagnose_compiler_errors")),
            ModelTier::DeepReasoning
        );
        assert_eq!(
            classify_task_tier("View UI mockup", Some("view_image")),
            ModelTier::Vision
        );
        assert_eq!(
            classify_task_tier("Please refactor this method and add unit tests", None),
            ModelTier::Coding
        );
    }

    #[test]
    fn test_hybrid_router_routing_and_fallback() {
        let router = HybridModelRouter::new();
        let fast_model = router.route_for_tier(ModelTier::Fast);
        assert!(fast_model.model_id.contains("8b"));

        let deep_model = router.route_for_tier(ModelTier::DeepReasoning);
        assert!(deep_model.model_id.contains("deepseek-r1"));

        let coding_model = router.route_for_tier(ModelTier::Coding);
        assert!(coding_model.model_id.contains("codestral"));

        let vision_model = router.route_for_tier(ModelTier::Vision);
        assert!(vision_model.model_id.contains("vision"));

        let fallback = router
            .fallback_for_failure("meta/llama-3.3-70b-instruct")
            .unwrap();
        assert_ne!(fallback.model_id, "meta/llama-3.3-70b-instruct");
    }

    #[test]
    fn test_fallback_ladder() {
        let router = HybridModelRouter::new();
        let ladder = router.fallback_ladder_for_tier(ModelTier::Coding);
        assert!(ladder.len() >= 3);
        assert_eq!(ladder[0].model_id, "mistralai/codestral-22b-instruct-v0.1");
    }
}
