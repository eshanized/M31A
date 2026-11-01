//! System Prompt Fragment Registry
//!
//! Resolves model-specific prompt fragments for injection into the system prompt.
//! Resolves tailored prompt fragments and message roles across supported model families.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// A prompt fragment with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fragment {
    pub id: String,
    pub name: String,
    pub content: String,
    pub priority: i32,                      // Higher = earlier in prompt
    pub model_families: Vec<String>,        // Which model families this applies to
    pub conditions: Vec<FragmentCondition>, // When to include
}

/// Conditions for fragment inclusion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FragmentCondition {
    Always,
    ToolEnabled(String),    // e.g., "apply_patch"
    FeatureEnabled(String), // e.g., "multi_agent"
    SandboxPolicy(String),  // e.g., "workspace_write"
    ApprovalPolicy(String), // e.g., "approve_for_me"
    ContextUsageAbove(f32), // e.g., 0.65
}

/// Resolved fragment set for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedFragments {
    pub model_id: String,
    pub model_family: ModelFamily,
    pub fragments: Vec<Fragment>,
    pub rendered: String,
}

/// Supported model families (maps to NIM model slugs)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelFamily {
    Nemotron35,      // nvidia/nemotron-3.5-*
    DeepSeek,        // deepseek-ai/deepseek-*
    CodeStral,       // mistralai/codestral-*
    Llama31Nemotron, // nvidia/llama-3.1-nemotron-*
    Other(String),   // Custom/unknown
}

impl ModelFamily {
    /// Detect family from model slug
    pub fn from_model_slug(slug: &str) -> Self {
        let slug = slug.to_lowercase();
        if slug.contains("nemotron-3.5") || slug.contains("nemotron-3-5") {
            ModelFamily::Nemotron35
        } else if slug.contains("deepseek") {
            ModelFamily::DeepSeek
        } else if slug.contains("codestral") {
            ModelFamily::CodeStral
        } else if slug.contains("llama-3.1-nemotron") || slug.contains("llama-3-1-nemotron") {
            ModelFamily::Llama31Nemotron
        } else {
            ModelFamily::Other(slug)
        }
    }

    /// Human-readable name
    pub fn display_name(&self) -> &str {
        match self {
            ModelFamily::Nemotron35 => "Nemotron 3.5",
            ModelFamily::DeepSeek => "DeepSeek",
            ModelFamily::CodeStral => "CodeStral",
            ModelFamily::Llama31Nemotron => "Llama 3.1 Nemotron",
            ModelFamily::Other(s) => s,
        }
    }
}

/// Fragment registry with built-in and custom fragments
pub struct FragmentRegistry {
    fragments: Arc<RwLock<HashMap<String, Fragment>>>,
    custom_dir: Option<PathBuf>,
}

impl FragmentRegistry {
    /// Create new registry with built-in fragments
    pub fn new(custom_dir: Option<PathBuf>) -> Self {
        let mut registry = Self {
            fragments: Arc::new(RwLock::new(HashMap::new())),
            custom_dir: custom_dir.clone(),
        };
        registry.register_builtin_fragments();
        if let Some(dir) = custom_dir {
            registry.load_custom_fragments(&dir).ok();
        }
        registry
    }

    /// Get the custom fragments directory, if configured
    pub fn custom_dir(&self) -> Option<&Path> {
        self.custom_dir.as_deref()
    }

    /// Register all built-in fragments
    fn register_builtin_fragments(&mut self) {
        let builtins = vec![
            // Base instructions - always included, highest priority
            Fragment {
                id: "base_instructions".to_string(),
                name: "Base Instructions".to_string(),
                content: include_str!("fragments/base_instructions.md").to_string(),
                priority: 100,
                model_families: vec!["*".to_string()],
                conditions: vec![FragmentCondition::Always],
            },
            // Plan Mode protocol - when plan_mode feature is active
            Fragment {
                id: "plan_mode".to_string(),
                name: "Conversational Plan Mode Protocol".to_string(),
                content: include_str!("fragments/plan_mode.md").to_string(),
                priority: 95,
                model_families: vec!["*".to_string()],
                conditions: vec![FragmentCondition::FeatureEnabled("plan_mode".to_string())],
            },
            // Apply patch guidance - when apply_patch tool is available
            Fragment {
                id: "apply_patch".to_string(),
                name: "Apply Patch Guidance".to_string(),
                content: include_str!("fragments/apply_patch.md").to_string(),
                priority: 90,
                model_families: vec!["*".to_string()],
                conditions: vec![FragmentCondition::ToolEnabled("apply_patch".to_string())],
            },
            // Sandbox policy - always relevant
            Fragment {
                id: "sandbox".to_string(),
                name: "Sandbox Policy".to_string(),
                content: include_str!("fragments/sandbox.md").to_string(),
                priority: 85,
                model_families: vec!["*".to_string()],
                conditions: vec![FragmentCondition::Always],
            },
            // Plan tool - when update_plan is enabled
            Fragment {
                id: "plan_tool".to_string(),
                name: "Plan Tool Usage".to_string(),
                content: include_str!("fragments/plan_tool.md").to_string(),
                priority: 80,
                model_families: vec!["*".to_string()],
                conditions: vec![FragmentCondition::ToolEnabled("update_plan".to_string())],
            },
            // Approval mode - always relevant
            Fragment {
                id: "approval_mode".to_string(),
                name: "Approval Mode Behavior".to_string(),
                content: include_str!("fragments/approval_mode.md").to_string(),
                priority: 75,
                model_families: vec!["*".to_string()],
                conditions: vec![FragmentCondition::Always],
            },
            // Token budget - when context tools available AND context usage high
            Fragment {
                id: "token_budget".to_string(),
                name: "Token Budget Management".to_string(),
                content: include_str!("fragments/token_budget.md").to_string(),
                priority: 70,
                model_families: vec!["*".to_string()],
                conditions: vec![
                    FragmentCondition::ToolEnabled("get_context_remaining".to_string()),
                    FragmentCondition::ContextUsageAbove(0.65),
                ],
            },
            // Multi-agent - when multi-agent tools enabled
            Fragment {
                id: "multi_agent".to_string(),
                name: "Multi-Agent Collaboration".to_string(),
                content: include_str!("fragments/multi_agent.md").to_string(),
                priority: 65,
                model_families: vec!["*".to_string()],
                conditions: vec![FragmentCondition::FeatureEnabled("multi_agent".to_string())],
            },
        ];

        for fragment in builtins {
            self.fragments
                .try_write()
                .expect("uncontended")
                .insert(fragment.id.clone(), fragment);
        }
    }

    /// Load custom fragments from directory
    fn load_custom_fragments(&mut self, dir: &PathBuf) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                if id.is_empty() {
                    continue;
                }
                let content = std::fs::read_to_string(&path)?;
                let fragment = Fragment {
                    id: id.clone(),
                    name: id.replace(['-', '_'], " "),
                    content,
                    priority: 50, // Custom fragments lower priority
                    model_families: vec!["*".to_string()],
                    conditions: vec![FragmentCondition::Always],
                };
                self.fragments
                    .try_write()
                    .expect("uncontended")
                    .insert(id, fragment);
            }
        }
        Ok(())
    }

    /// Resolve fragments for a specific model and tool set
    pub async fn resolve(
        &self,
        model_slug: &str,
        enabled_tools: &[String],
        enabled_features: &[String],
        sandbox_policy: &str,
        approval_policy: &str,
        context_usage: f32,
    ) -> Result<ResolvedFragments> {
        let family = ModelFamily::from_model_slug(model_slug);
        let fragments = self.fragments.read().await;

        let mut selected: Vec<Fragment> = fragments
            .values()
            .filter(|f| {
                self.should_include(
                    f,
                    &family,
                    enabled_tools,
                    enabled_features,
                    sandbox_policy,
                    approval_policy,
                    context_usage,
                )
            })
            .cloned()
            .collect();

        // Sort by priority (highest first)
        selected.sort_by_key(|a| std::cmp::Reverse(a.priority));

        // Render combined
        let rendered = selected
            .iter()
            .map(|f| f.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        Ok(ResolvedFragments {
            model_id: model_slug.to_string(),
            model_family: family,
            fragments: selected,
            rendered,
        })
    }

    /// Check if fragment should be included
    #[allow(clippy::too_many_arguments)]
    fn should_include(
        &self,
        fragment: &Fragment,
        family: &ModelFamily,
        enabled_tools: &[String],
        enabled_features: &[String],
        sandbox_policy: &str,
        approval_policy: &str,
        context_usage: f32,
    ) -> bool {
        // Check model family match
        let family_match = fragment
            .model_families
            .iter()
            .any(|m| m == "*" || *m == family.display_name().to_lowercase().replace(' ', "-"));
        if !family_match {
            return false;
        }

        // Check all conditions
        for condition in &fragment.conditions {
            match condition {
                FragmentCondition::Always => {}
                FragmentCondition::ToolEnabled(tool) => {
                    if !enabled_tools.iter().any(|t| t == tool) {
                        return false;
                    }
                }
                FragmentCondition::FeatureEnabled(feature) => {
                    if !enabled_features.iter().any(|f| f == feature) {
                        return false;
                    }
                }
                FragmentCondition::SandboxPolicy(policy) => {
                    if sandbox_policy != policy {
                        return false;
                    }
                }
                FragmentCondition::ApprovalPolicy(policy) => {
                    if approval_policy != policy {
                        return false;
                    }
                }
                FragmentCondition::ContextUsageAbove(threshold) => {
                    if context_usage <= *threshold {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Get all registered fragments (for debugging)
    pub async fn list_all(&self) -> Vec<Fragment> {
        self.fragments.read().await.values().cloned().collect()
    }

    /// Add a custom fragment at runtime
    pub async fn add_fragment(&self, fragment: Fragment) {
        self.fragments
            .write()
            .await
            .insert(fragment.id.clone(), fragment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_family_detection() {
        assert_eq!(
            ModelFamily::from_model_slug("nvidia/nemotron-3.5-lightning-30b-a3b"),
            ModelFamily::Nemotron35
        );
        assert_eq!(
            ModelFamily::from_model_slug("deepseek-ai/deepseek-v4.1-flash"),
            ModelFamily::DeepSeek
        );
        assert_eq!(
            ModelFamily::from_model_slug("mistralai/codestral-22b-instruct-v0.1"),
            ModelFamily::CodeStral
        );
        assert_eq!(
            ModelFamily::from_model_slug("nvidia/llama-3.1-nemotron-70b-instruct"),
            ModelFamily::Llama31Nemotron
        );
    }

    #[tokio::test]
    async fn test_resolve_for_nemotron() {
        let registry = FragmentRegistry::new(None);
        let resolved = registry
            .resolve(
                "nvidia/nemotron-3.5-lightning-30b-a3b",
                &[
                    "apply_patch".to_string(),
                    "update_plan".to_string(),
                    "get_context_remaining".to_string(),
                    "spawn_agent".to_string(),
                ],
                &["multi_agent".to_string()],
                "workspace_write",
                "on_request",
                0.8, // High context usage to trigger token_budget fragment
            )
            .await
            .unwrap();

        assert_eq!(resolved.model_family, ModelFamily::Nemotron35);
        assert!(
            resolved
                .fragments
                .iter()
                .any(|f| f.id == "base_instructions")
        );
        assert!(resolved.fragments.iter().any(|f| f.id == "apply_patch"));
        assert!(resolved.fragments.iter().any(|f| f.id == "plan_tool"));
        assert!(resolved.fragments.iter().any(|f| f.id == "token_budget"));
        assert!(resolved.fragments.iter().any(|f| f.id == "multi_agent"));
        assert!(resolved.fragments.iter().any(|f| f.id == "sandbox"));
        assert!(resolved.fragments.iter().any(|f| f.id == "approval_mode"));
    }

    #[tokio::test]
    async fn test_resolve_excludes_unavailable_tools() {
        let registry = FragmentRegistry::new(None);
        let resolved = registry
            .resolve(
                "nvidia/nemotron-3.5-lightning-30b-a3b",
                &["read_file".to_string()], // Only basic tools
                &[],
                "readonly",
                "never",
                0.3,
            )
            .await
            .unwrap();

        // Should have base, sandbox, approval_mode
        // Should NOT have apply_patch, plan_tool, token_budget, multi_agent
        assert!(
            resolved
                .fragments
                .iter()
                .any(|f| f.id == "base_instructions")
        );
        assert!(resolved.fragments.iter().any(|f| f.id == "sandbox"));
        assert!(resolved.fragments.iter().any(|f| f.id == "approval_mode"));
        assert!(!resolved.fragments.iter().any(|f| f.id == "apply_patch"));
        assert!(!resolved.fragments.iter().any(|f| f.id == "plan_tool"));
        assert!(!resolved.fragments.iter().any(|f| f.id == "token_budget"));
        assert!(!resolved.fragments.iter().any(|f| f.id == "multi_agent"));
    }

    #[tokio::test]
    async fn test_context_usage_condition() {
        let registry = FragmentRegistry::new(None);

        // Low usage - token_budget should be excluded (threshold 0.65)
        let resolved = registry
            .resolve(
                "nvidia/nemotron-3.5-lightning-30b-a3b",
                &["get_context_remaining".to_string()],
                &[],
                "workspace_write",
                "on_request",
                0.3,
            )
            .await
            .unwrap();
        assert!(
            !resolved.fragments.iter().any(|f| f.id == "token_budget"),
            "Should exclude at 30% usage"
        );

        // High usage - token_budget should be included
        let resolved = registry
            .resolve(
                "nvidia/nemotron-3.5-lightning-30b-a3b",
                &["get_context_remaining".to_string()],
                &[],
                "workspace_write",
                "on_request",
                0.8,
            )
            .await
            .unwrap();
        assert!(
            resolved.fragments.iter().any(|f| f.id == "token_budget"),
            "Should include at 80% usage"
        );
    }
}
