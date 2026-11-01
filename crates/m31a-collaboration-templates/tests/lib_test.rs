//! Dedicated integration tests for `m31a-collaboration-templates` (src/lib.rs).
//!
//! Validates:
//! - Builtin template loading and category categorization
//! - Variable interpolation during template application
//! - Custom template serialization to TOML and persistent filesystem discovery
//! - Error boundaries for missing templates

use m31a_collaboration_templates::{
    CollaborationTemplate, TemplateCategory, TemplateConfig, TemplateLibrary, TemplateStep,
};
use std::collections::HashMap;
use tempfile::tempdir;

#[test]
fn test_builtin_templates_inventory_and_steps() {
    let tmp = tempdir().unwrap();
    let library = TemplateLibrary::new(tmp.path().to_path_buf());

    let all_templates = library.list();
    assert!(
        all_templates.len() >= 6,
        "Expected at least 6 builtin templates, found {}",
        all_templates.len()
    );

    let required_ids = [
        "pair-programming",
        "code-review",
        "debugging",
        "architecture-design",
        "feature-development",
        "refactoring",
        "security-audit",
    ];

    for id in required_ids {
        let t = library.get(id);
        assert!(t.is_some(), "Missing builtin template: {}", id);
        let template = t.unwrap();
        assert!(!template.name.is_empty());
        assert!(!template.description.is_empty());
        assert!(!template.steps.is_empty(), "Template {} must have workflow steps", id);
        assert!(template.estimated_duration_minutes.is_some());
    }

    // Verify category filtering
    let reviews = library.list_by_category(TemplateCategory::CodeReview);
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].id, "code-review");
}

#[test]
fn test_template_variable_interpolation() {
    let tmp = tempdir().unwrap();
    let library = TemplateLibrary::new(tmp.path().to_path_buf());

    let mut vars = HashMap::new();
    vars.insert(
        "bug_description".to_string(),
        "SIGSEGV when reading socket buffer".to_string(),
    );

    let applied_config = library.apply_template("debugging", vars).unwrap();
    let initial_prompt = applied_config.initial_prompt.unwrap();

    assert!(
        initial_prompt.contains("SIGSEGV when reading socket buffer"),
        "Variable must be interpolated in prompt: {}",
        initial_prompt
    );
    assert!(
        !initial_prompt.contains("{bug_description}"),
        "Raw placeholder must not remain"
    );

    // Assert tools and policies are preserved
    assert!(!applied_config.enabled_tools.is_empty());
    assert!(applied_config.model.is_some());
}

#[test]
fn test_apply_template_missing_error() {
    let tmp = tempdir().unwrap();
    let library = TemplateLibrary::new(tmp.path().to_path_buf());

    let res = library.apply_template("non-existent-template", HashMap::new());
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Template 'non-existent-template' not found"));
}

#[test]
fn test_custom_template_persistence_and_loading() {
    let tmp = tempdir().unwrap();
    let custom_dir = tmp.path().join("templates");

    let custom_template = CollaborationTemplate {
        id: "incident-response".to_string(),
        name: "Incident Response".to_string(),
        description: "Post-outage root cause mitigation and runbook execution".to_string(),
        category: TemplateCategory::Custom,
        icon: "🚨".to_string(),
        config: TemplateConfig {
            collaboration_mode: None,
            sandbox_policy: None,
            approval_policy: None,
            model: Some("gpt-4o".to_string()),
            reasoning_effort: None,
            environment_vars: HashMap::new(),
            disabled_tools: vec![],
            enabled_tools: vec!["exec_command".to_string(), "read_file".to_string()],
            context_files: vec!["runbook.md".to_string()],
            initial_prompt: Some("Mitigating incident {incident_id}".to_string()),
        },
        steps: vec![TemplateStep {
            order: 1,
            title: "Check Logs".to_string(),
            description: "Review system logs for panic stack traces".to_string(),
            prompt_template: Some("Checking logs for {incident_id}".to_string()),
            expected_tools: vec!["read_file".to_string()],
            validation: None,
            is_optional: false,
        }],
        tags: vec!["incident".to_string(), "prod".to_string()],
        estimated_duration_minutes: Some(20),
    };

    // Save custom template via library
    {
        let library = TemplateLibrary::new(custom_dir.clone());
        library.save_custom(&custom_template).unwrap();
    }

    // Verify .toml file was generated
    let toml_file = custom_dir.join("incident-response.toml");
    assert!(toml_file.exists(), "Custom template TOML file must exist");

    // Create a new TemplateLibrary instance pointing to custom_dir and verify automatic discovery
    {
        let library2 = TemplateLibrary::new(custom_dir);
        let loaded = library2.get("incident-response");
        assert!(loaded.is_some(), "Custom template must be loaded on startup");
        let t = loaded.unwrap();
        assert_eq!(t.name, "Incident Response");
        assert_eq!(t.steps.len(), 1);

        // Apply template
        let mut vars = HashMap::new();
        vars.insert("incident_id".to_string(), "INC-4040".to_string());
        let applied = library2.apply_template("incident-response", vars).unwrap();
        assert_eq!(applied.initial_prompt.unwrap(), "Mitigating incident INC-4040");
    }
}
