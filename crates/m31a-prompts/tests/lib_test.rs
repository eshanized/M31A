//! Dedicated integration tests for `m31a-prompts` (src/lib.rs).
//!
//! Validates:
//! - Builtin prompt templates loading and variable structure
//! - Template variable substitution, default value fallback, and error handling for missing required variables
//! - Template search and category filtering
//! - Custom template creation, TOML file persistence, retrieval, and deletion safety

use m31a_prompts::{
    PromptCategory, PromptLibrary, PromptTemplate, PromptVariable, VariableType,
};
use std::collections::HashMap;
use tempfile::tempdir;

#[tokio::test]
async fn test_builtin_prompt_templates_catalog() {
    let tmp = tempdir().unwrap();
    let library = PromptLibrary::new(tmp.path().to_path_buf());

    let all_templates = library.list().await;
    assert!(
        all_templates.len() >= 6,
        "Expected at least 6 builtin templates, found {}",
        all_templates.len()
    );

    let required_ids = [
        "code-explanation",
        "code-review",
        "debugging-assistant",
        "adr-template",
        "tdd-test-generation",
        "refactoring-plan",
        "threat-modeling",
    ];

    for id in required_ids {
        let t = library.get(id).await;
        assert!(t.is_some(), "Missing builtin prompt template: {}", id);
        let template = t.unwrap();
        assert!(template.is_builtin);
        assert!(!template.template.is_empty());
        assert!(!template.variables.is_empty());
    }

    // Category filter
    let review_templates = library.list_by_category(PromptCategory::CodeReview).await;
    assert_eq!(review_templates.len(), 1);
    assert_eq!(review_templates[0].id, "code-review");
}

#[tokio::test]
async fn test_prompt_render_substitution_and_defaults() {
    let tmp = tempdir().unwrap();
    let library = PromptLibrary::new(tmp.path().to_path_buf());

    let mut vars = HashMap::new();
    vars.insert(
        "code_snippet".to_string(),
        "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
    );
    // Omit 'language' and 'audience' to test default value resolution (python, intermediate)
    let rendered = library.render("code-explanation", vars).await.unwrap();

    assert!(
        rendered.rendered.contains("fn add(a: i32, b: i32)"),
        "Substituted snippet must appear in rendered output"
    );
    assert!(
        rendered.rendered.contains("python"),
        "Default language 'python' should be applied"
    );
    assert!(
        rendered.rendered.contains("intermediate"),
        "Default audience 'intermediate' should be applied"
    );
    assert_eq!(rendered.variables_used.get("language").unwrap(), "python");
}

#[tokio::test]
async fn test_render_missing_required_variable_fails() {
    let tmp = tempdir().unwrap();
    let library = PromptLibrary::new(tmp.path().to_path_buf());

    // code-explanation requires 'code_snippet'
    let empty_vars = HashMap::new();
    let res = library.render("code-explanation", empty_vars).await;
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Required variable 'code_snippet' not provided"));
}

#[tokio::test]
async fn test_custom_template_lifecycle_and_deletion_safety() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path().join("prompts_lib");
    let library = PromptLibrary::new(dir.clone());

    let custom_template = PromptTemplate {
        id: "git-commit-helper".to_string(),
        name: "Conventional Git Commit Helper".to_string(),
        description: "Generate structured conventional commits".to_string(),
        category: PromptCategory::Custom,
        tags: vec!["git".to_string(), "vcs".to_string()],
        version: "1.0.0".to_string(),
        author: "DevOps Team".to_string(),
        created_at: 1700000000,
        updated_at: 1700000000,
        template: "feat({{scope}}): {{summary}}".to_string(),
        variables: vec![
            PromptVariable {
                name: "scope".to_string(),
                description: "Component name".to_string(),
                var_type: VariableType::String,
                required: true,
                default: None,
                options: None,
            },
            PromptVariable {
                name: "summary".to_string(),
                description: "Commit title".to_string(),
                var_type: VariableType::String,
                required: true,
                default: None,
                options: None,
            },
        ],
        examples: vec![],
        is_builtin: false,
    };

    // Save custom template
    library.save(custom_template).await.unwrap();

    // Verify .toml file was written
    let toml_file = dir.join("git-commit-helper.toml");
    assert!(toml_file.exists());

    // Search matches
    let found = library.search("conventional").await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "git-commit-helper");

    // Cannot delete builtin template
    let del_builtin = library.delete("code-review").await;
    assert!(del_builtin.is_err());
    assert!(del_builtin.unwrap_err().to_string().contains("Cannot delete builtin template"));

    // Successfully delete custom template
    library.delete("git-commit-helper").await.unwrap();
    assert!(library.get("git-commit-helper").await.is_none());
    assert!(!toml_file.exists());
}
