//! Dedicated integration tests for `m31a-prompts` (src/fragment_registry.rs).
//!
//! Validates:
//! - ModelFamily slug parsing across vendor models
//! - Fragment inclusion condition evaluation (Always, ToolEnabled, FeatureEnabled, ContextUsageAbove)
//! - Priority ordering during fragment resolution
//! - Custom markdown fragment discovery and disk ingestion

use m31a_prompts::{FragmentRegistry, ModelFamily};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_model_family_from_slug() {
    assert_eq!(
        ModelFamily::from_model_slug("nvidia/nemotron-3.5-lightning-30b-a3b"),
        ModelFamily::Nemotron35
    );
    assert_eq!(
        ModelFamily::from_model_slug("deepseek-ai/deepseek-r1"),
        ModelFamily::DeepSeek
    );
    assert_eq!(
        ModelFamily::from_model_slug("mistralai/codestral-22b-v0.1"),
        ModelFamily::CodeStral
    );
    assert_eq!(
        ModelFamily::from_model_slug("nvidia/llama-3.1-nemotron-70b-instruct"),
        ModelFamily::Llama31Nemotron
    );
    assert_eq!(
        ModelFamily::from_model_slug("openai/gpt-4o"),
        ModelFamily::Other("openai/gpt-4o".to_string())
    );
}

#[tokio::test]
async fn test_fragment_conditions_and_priority_resolution() {
    let registry = FragmentRegistry::new(None);

    // 1. Resolve with minimal tools and low context usage
    let resolved_min = registry
        .resolve(
            "deepseek-ai/deepseek-r1",
            &[],
            &[],
            "workspace_write",
            "on_request",
            0.20,
        )
        .await
        .unwrap();

    assert_eq!(resolved_min.model_family, ModelFamily::DeepSeek);
    // Base instructions and sandbox should be included
    assert!(
        resolved_min
            .fragments
            .iter()
            .any(|f| f.id == "base_instructions")
    );
    assert!(resolved_min.fragments.iter().any(|f| f.id == "sandbox"));
    // Condition-gated fragments should NOT be included
    assert!(!resolved_min.fragments.iter().any(|f| f.id == "apply_patch"));
    assert!(
        !resolved_min
            .fragments
            .iter()
            .any(|f| f.id == "token_budget")
    );
    assert!(!resolved_min.fragments.iter().any(|f| f.id == "plan_mode"));

    // 2. Resolve with apply_patch tool and high context usage
    let resolved_active = registry
        .resolve(
            "nvidia/nemotron-3.5-lightning-30b-a3b",
            &[
                "apply_patch".to_string(),
                "get_context_remaining".to_string(),
            ],
            &["plan_mode".to_string()],
            "workspace_write",
            "on_request",
            0.85,
        )
        .await
        .unwrap();

    assert!(
        resolved_active
            .fragments
            .iter()
            .any(|f| f.id == "apply_patch")
    );
    assert!(
        resolved_active
            .fragments
            .iter()
            .any(|f| f.id == "plan_mode")
    );
    assert!(
        resolved_active
            .fragments
            .iter()
            .any(|f| f.id == "token_budget")
    );

    // 3. Verify strict priority descending order
    for i in 1..resolved_active.fragments.len() {
        assert!(
            resolved_active.fragments[i - 1].priority >= resolved_active.fragments[i].priority,
            "Fragments must be sorted by priority descending: {} ({}) vs {} ({})",
            resolved_active.fragments[i - 1].id,
            resolved_active.fragments[i - 1].priority,
            resolved_active.fragments[i].id,
            resolved_active.fragments[i].priority
        );
    }
}

#[tokio::test]
async fn test_custom_markdown_fragment_discovery() {
    let tmp = tempdir().unwrap();
    let custom_dir = tmp.path().to_path_buf();

    // Create a custom markdown fragment file
    let custom_file = custom_dir.join("enterprise_security_guidelines.md");
    fs::write(
        &custom_file,
        "# Enterprise Security Policy\nAll code must undergo static taint analysis.",
    )
    .unwrap();

    let registry = FragmentRegistry::new(Some(custom_dir));
    assert!(registry.custom_dir().is_some());

    let resolved = registry
        .resolve(
            "mistralai/codestral-22b-v0.1",
            &[],
            &[],
            "workspace_write",
            "on_request",
            0.1,
        )
        .await
        .unwrap();

    let custom_fragment = resolved
        .fragments
        .iter()
        .find(|f| f.id == "enterprise_security_guidelines");
    assert!(
        custom_fragment.is_some(),
        "Custom markdown fragment must be loaded and resolved"
    );
    assert!(
        resolved
            .rendered
            .contains("All code must undergo static taint analysis")
    );
}
