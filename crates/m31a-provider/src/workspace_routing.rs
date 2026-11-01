//! Workspace-Aware Dynamic Model Routing and Repository Complexity Analyzer.
//!
//! Evaluates the active workspace codebase (primary programming languages, project
//! manifests, systems vs application code) and task prompts to select the optimal
//! LLM tier (Deep Thinking / Reasoning, High-Throughput Coder, Vision, or Local Offline).

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Complexity classification of a codebase or programming task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ComplexityTier {
    Light,
    Standard,
    Complex,
    Architectural,
}

/// Profile describing the workspace repository environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceProfile {
    pub primary_language: Option<String>,
    pub is_systems_code: bool,
    pub has_build_manifest: bool,
    pub detected_complexity: ComplexityTier,
}

impl Default for WorkspaceProfile {
    fn default() -> Self {
        Self {
            primary_language: None,
            is_systems_code: false,
            has_build_manifest: false,
            detected_complexity: ComplexityTier::Standard,
        }
    }
}

/// Workspace analyzer inspecting directory contents.
pub struct WorkspaceComplexityAnalyzer;

impl WorkspaceComplexityAnalyzer {
    /// Inspect a workspace directory to determine language profile and complexity.
    pub fn analyze(workspace_dir: &Path) -> WorkspaceProfile {
        let mut profile = WorkspaceProfile::default();

        if workspace_dir.join("Cargo.toml").exists() {
            profile.primary_language = Some("Rust".to_string());
            profile.is_systems_code = true;
            profile.has_build_manifest = true;
            profile.detected_complexity = ComplexityTier::Complex;
            return profile;
        }

        if workspace_dir.join("go.mod").exists() {
            profile.primary_language = Some("Go".to_string());
            profile.is_systems_code = true;
            profile.has_build_manifest = true;
            profile.detected_complexity = ComplexityTier::Complex;
            return profile;
        }

        if workspace_dir.join("CMakeLists.txt").exists() || workspace_dir.join("Makefile").exists()
        {
            profile.primary_language = Some("C/C++".to_string());
            profile.is_systems_code = true;
            profile.has_build_manifest = true;
            profile.detected_complexity = ComplexityTier::Complex;
            return profile;
        }

        if workspace_dir.join("build.zig").exists() {
            profile.primary_language = Some("Zig".to_string());
            profile.is_systems_code = true;
            profile.has_build_manifest = true;
            profile.detected_complexity = ComplexityTier::Complex;
            return profile;
        }

        if workspace_dir.join("package.json").exists() {
            profile.primary_language = Some("TypeScript/JavaScript".to_string());
            profile.is_systems_code = false;
            profile.has_build_manifest = true;
            profile.detected_complexity = ComplexityTier::Standard;
            return profile;
        }

        if workspace_dir.join("pyproject.toml").exists()
            || workspace_dir.join("requirements.txt").exists()
        {
            profile.primary_language = Some("Python".to_string());
            profile.is_systems_code = false;
            profile.has_build_manifest = true;
            profile.detected_complexity = ComplexityTier::Standard;
            return profile;
        }

        if workspace_dir.join("pom.xml").exists() || workspace_dir.join("build.gradle").exists() {
            profile.primary_language = Some("Java/Kotlin".to_string());
            profile.is_systems_code = false;
            profile.has_build_manifest = true;
            profile.detected_complexity = ComplexityTier::Standard;
            return profile;
        }

        if workspace_dir.join("Gemfile").exists() {
            profile.primary_language = Some("Ruby".to_string());
            profile.is_systems_code = false;
            profile.has_build_manifest = true;
            profile.detected_complexity = ComplexityTier::Light;
            return profile;
        }

        profile
    }
}

/// Workspace routing policy for model selection.
#[derive(Debug, Clone)]
pub struct WorkspaceRoutingPolicy {
    pub default_model: String,
    pub reasoning_model: String,
    pub coding_model: String,
    pub vision_model: String,
    pub local_offline_model: String,
}

impl Default for WorkspaceRoutingPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceRoutingPolicy {
    pub fn new() -> Self {
        Self {
            default_model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
            reasoning_model: "deepseek-ai/deepseek-v4.1-flash".to_string(),
            coding_model: "mistralai/codestral-22b-instruct-v0.1".to_string(),
            vision_model: "meta/llama-3.2-11b-vision-instruct".to_string(),
            local_offline_model: "ollama/qwen2.5-coder:32b".to_string(),
        }
    }

    /// Select the optimal model based on workspace profile, prompt content, and offline mode.
    pub fn select_model(
        &self,
        profile: &WorkspaceProfile,
        prompt: &str,
        offline_mode: bool,
    ) -> String {
        if offline_mode {
            return self.local_offline_model.clone();
        }

        let lower = prompt.to_ascii_lowercase();

        // Multimodal / UI inspection requests -> Vision model
        let is_vision_prompt = lower.contains("screenshot")
            || lower.contains("view image")
            || lower.contains("ui mockup")
            || lower.contains("wireframe")
            || lower.contains("diagram");

        if is_vision_prompt {
            return self.vision_model.clone();
        }

        // Architectural, debugging, or complex algorithmic prompts -> Reasoning model
        let is_architectural_prompt = lower.contains("architect")
            || lower.contains("refactor")
            || lower.contains("diagnose")
            || lower.contains("self-heal")
            || lower.contains("algorithm")
            || lower.contains("audit")
            || lower.contains("deadlock");

        if is_architectural_prompt || profile.detected_complexity == ComplexityTier::Architectural {
            return self.reasoning_model.clone();
        }

        // Systems code (Rust/C++/Go/Zig) -> High-fidelity coder model
        if profile.is_systems_code {
            return self.coding_model.clone();
        }

        self.default_model.clone()
    }

    /// Resolves the ModelInfo structure for the selected model.
    pub fn select_model_info(
        &self,
        profile: &WorkspaceProfile,
        prompt: &str,
        offline_mode: bool,
    ) -> Option<crate::models::ModelInfo> {
        let slug = self.select_model(profile, prompt, offline_mode);
        crate::models::find_model(&slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_analyze_rust_workspace() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            b"[package]\nname = \"demo\"\n",
        )
        .unwrap();

        let profile = WorkspaceComplexityAnalyzer::analyze(dir.path());
        assert_eq!(profile.primary_language.as_deref(), Some("Rust"));
        assert!(profile.is_systems_code);
        assert_eq!(profile.detected_complexity, ComplexityTier::Complex);
    }

    #[test]
    fn test_routing_decisions() {
        let policy = WorkspaceRoutingPolicy::new();

        let rust_profile = WorkspaceProfile {
            primary_language: Some("Rust".to_string()),
            is_systems_code: true,
            has_build_manifest: true,
            detected_complexity: ComplexityTier::Complex,
        };

        // Standard Rust task -> Coding model
        let model1 = policy.select_model(&rust_profile, "Implement a vector buffer", false);
        assert_eq!(model1, policy.coding_model);

        // Architectural task -> Reasoning model
        let model2 = policy.select_model(
            &rust_profile,
            "Architect a multi-threaded raft protocol",
            false,
        );
        assert_eq!(model2, policy.reasoning_model);

        // Offline mode -> Local model
        let model3 = policy.select_model(&rust_profile, "Implement a vector buffer", true);
        assert_eq!(model3, policy.local_offline_model);

        // Vision task -> Vision model
        let model4 =
            policy.select_model(&rust_profile, "Inspect screenshot of this ui mockup", false);
        assert_eq!(model4, policy.vision_model);
    }
}
