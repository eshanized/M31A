//! Comprehensive integration and security test suite for workflow manifests (Package 2).

use m31a::state_machine::agent::AgentRole;
use m31a::workflow::definition::RecoveryStrategy;
use m31a::workflow::error::WorkflowError;
use m31a::workflow::manifest::{
    CURRENT_MANIFEST_VERSION, WorkflowHeaderManifest, WorkflowManifest, WorkflowStepManifest,
};
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_valid_manifest_from_markdown_str() {
    let markdown = r#"---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  workflow:
    id: test_wf
    name: Test Workflow
    description: A valid test workflow
    version: 1
    default_recovery_strategy:
      type: fail
  steps:
    - key: step_a
      name: First Step
      role: researcher
      prompt: genesis.discovery.v1
      timeout_secs: 600
      allows_parallelism: false
      depends_on: []
      expected_outputs:
        - artifact_name: A.md
          relative_path: docs/A.md
          schema_type: markdown
    - key: step_b
      name: Second Step
      role: architect
      prompt: genesis.architecture.v1
      timeout_secs: 300
      allows_parallelism: true
      depends_on: [step_a]
      required_inputs:
        - parameter_name: input_a
          source_step_key: step_a
          artifact_name: A.md
          is_optional: false
      expected_outputs:
        - artifact_name: B.md
          relative_path: docs/B.md
          schema_type: markdown
---

# Test Workflow

This body must remain separate from the structured manifest.
"#;

    let manifest = WorkflowManifest::from_markdown_str(markdown).expect("parse valid Markdown manifest");
    assert_eq!(manifest.manifest_version, CURRENT_MANIFEST_VERSION);
    assert_eq!(manifest.workflow.id, "test_wf");
    assert_eq!(manifest.steps.len(), 2);
    assert_eq!(manifest.steps[0].role, AgentRole::researcher());
    assert_eq!(manifest.steps[1].role, AgentRole::architect());
    manifest.validate(None).expect("validation should succeed");
}

#[test]
fn test_legacy_workflow_fixture_imports_to_markdown() {
    let legacy_path = Path::new("tests/fixtures/legacy_toml/greenfield.toml");
    let legacy_toml = std::fs::read_to_string(legacy_path).unwrap();
    let markdown = m31a::config::migrate::convert_workflow_toml_to_markdown(&legacy_toml)
        .expect("explicit importer should convert legacy workflow TOML");
    let manifest = WorkflowManifest::from_markdown_str(&markdown)
        .expect("canonical parser should load converted Markdown");
    assert_eq!(manifest.workflow.id, "greenfield");
    assert!(!manifest.steps.is_empty());
}

#[test]
fn test_manifest_rejects_malformed_legacy_toml() {
    let bad_toml = "workflow = { id = unquoted_bad_syntax";
    let err = m31a::config::migrate::convert_workflow_toml_to_markdown(bad_toml).unwrap_err();
    assert!(err.contains("Invalid workflow TOML"));
}

#[test]
fn test_manifest_rejects_empty_id_or_name() {
    let mut manifest = WorkflowManifest {
        manifest_version: 1,
        workflow: WorkflowHeaderManifest {
            id: "".to_string(),
            name: "Name".to_string(),
            description: "".to_string(),
            version: 1,
            default_recovery_strategy: RecoveryStrategy::Fail,
        },
        steps: vec![WorkflowStepManifest {
            key: "step1".to_string(),
            name: "Step 1".to_string(),
            role: AgentRole::researcher(),
            prompt: "test.v1".to_string(),
            prompt_version: None,
            required_inputs: vec![],
            expected_outputs: vec![],
            required_capabilities: vec![],
            quality_gate: None,
            depends_on: vec![],
            timeout_secs: 60,
            allows_parallelism: false,
            recovery_strategy: None,
        }],
    };

    let err = manifest.validate(None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::ManifestValidation { field: Some(f), .. } if f == "workflow.id")
    );

    manifest.workflow.id = "valid-id".to_string();
    manifest.workflow.name = "   ".to_string();
    let err2 = manifest.validate(None).unwrap_err();
    assert!(
        matches!(err2, WorkflowError::ManifestValidation { field: Some(f), .. } if f == "workflow.name")
    );
}

#[test]
fn test_manifest_rejects_empty_steps() {
    let manifest = WorkflowManifest {
        manifest_version: 1,
        workflow: WorkflowHeaderManifest {
            id: "wf".to_string(),
            name: "Name".to_string(),
            description: "".to_string(),
            version: 1,
            default_recovery_strategy: RecoveryStrategy::Fail,
        },
        steps: vec![],
    };

    let err = manifest.validate(None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::ManifestValidation { field: Some(f), .. } if f == "steps")
    );
}

#[test]
fn test_manifest_rejects_duplicate_step_keys() {
    let md_str = r#"---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  workflow:
    id: dup_test
    name: Dup Test
    version: 1
  steps:
    - key: alpha
      name: First Alpha
      role: researcher
      prompt: test.v1
    - key: alpha
      name: Second Alpha
      role: implementer
      prompt: test.v1
---
# Dup Test
"#;

    let manifest = WorkflowManifest::from_markdown_str(md_str).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::ManifestValidation { field: Some(f), reason, .. } if f == "key" && reason.contains("duplicate"))
    );
}

#[test]
fn test_manifest_rejects_unknown_and_self_dependencies() {
    // Self-dependency
    let md_self = r#"---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  workflow:
    id: self_dep
    name: Self Dep
    version: 1
  steps:
    - key: alpha
      name: Alpha
      role: researcher
      prompt: test.v1
      depends_on: ["alpha"]
---
# Self Dep
"#;
    let manifest = WorkflowManifest::from_markdown_str(md_self).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::ManifestValidation { field: Some(f), reason, .. } if f == "depends_on" && reason.contains("itself"))
    );

    // Missing dependency
    let md_missing = r#"---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  workflow:
    id: missing_dep
    name: Missing Dep
    version: 1
  steps:
    - key: alpha
      name: Alpha
      role: researcher
      prompt: test.v1
      depends_on: ["non_existent"]
---
# Missing Dep
"#;
    let manifest2 = WorkflowManifest::from_markdown_str(md_missing).unwrap();
    let err2 = manifest2.validate(None).unwrap_err();
    assert!(
        matches!(err2, WorkflowError::ManifestValidation { field: Some(f), reason, .. } if f == "depends_on" && reason.contains("not found"))
    );
}

#[test]
fn test_manifest_rejects_dependency_cycle() {
    let md_cycle = r#"---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  workflow:
    id: cycle_wf
    name: Cycle Test
    version: 1
  steps:
    - key: alpha
      name: Alpha
      role: researcher
      prompt: test.v1
      depends_on: ["gamma"]
    - key: beta
      name: Beta
      role: researcher
      prompt: test.v1
      depends_on: ["alpha"]
    - key: gamma
      name: Gamma
      role: researcher
      prompt: test.v1
      depends_on: ["beta"]
---
# Cycle Test
"#;

    let manifest = WorkflowManifest::from_markdown_str(md_cycle).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(matches!(err, WorkflowError::CycleDetected { cycle } if cycle.len() == 3));
}

#[test]
fn test_manifest_rejects_invalid_timeout() {
    let md_zero = r#"---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  workflow:
    id: timeout_wf
    name: Timeout Test
    version: 1
  steps:
    - key: alpha
      name: Alpha
      role: researcher
      prompt: test.v1
      timeout_secs: 0
---
# Timeout Test
"#;
    let manifest = WorkflowManifest::from_markdown_str(md_zero).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::ManifestValidation { field: Some(f), .. } if f == "timeout_secs")
    );
}

#[test]
fn test_manifest_rejects_invalid_artifact_paths() {
    // Absolute path
    let md_abs = r#"---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  workflow:
    id: abs_path
    name: Abs Path
    version: 1
  steps:
    - key: alpha
      name: Alpha
      role: researcher
      prompt: test.v1
      outputs:
        - artifact_name: bad.md
          relative_path: /etc/passwd
          schema_type: markdown
---
# Abs Path
"#;
    let manifest = WorkflowManifest::from_markdown_str(md_abs).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(matches!(err, WorkflowError::InvalidArtifactPath { .. }));

    // Traversal ..
    let md_trav = r#"---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  workflow:
    id: trav_path
    name: Traversal Path
    version: 1
  steps:
    - key: alpha
      name: Alpha
      role: researcher
      prompt: test.v1
      outputs:
        - artifact_name: bad.md
          relative_path: "docs/../../secret.txt"
          schema_type: markdown
---
# Traversal Path
"#;
    let manifest2 = WorkflowManifest::from_markdown_str(md_trav).unwrap();
    let err2 = manifest2.validate(None).unwrap_err();
    assert!(matches!(err2, WorkflowError::InvalidArtifactPath { .. }));
}

#[test]
fn test_manifest_file_security_checks() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // 1. Refuse path with ..
    let bad_path = root.join("../escape.md");
    let err = WorkflowManifest::from_file(&bad_path, Some(root)).unwrap_err();
    assert!(matches!(err, WorkflowError::PathViolation { .. }));

    // 2. Refuse path inside .git
    let git_dir = root.join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    let git_file = git_dir.join("workflow.md");
    std::fs::write(&git_file, "content").unwrap();
    let err2 = WorkflowManifest::from_file(&git_file, Some(root)).unwrap_err();
    assert!(matches!(err2, WorkflowError::PathViolation { .. }));

    // 3. Canonical workflow documents are allowed in protected M31A storage.
    let m31a_dir = root.join(".m31a/project/workflows");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let m31a_file = m31a_dir.join("workflow.md");
    std::fs::write(
        &m31a_file,
        r#"---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  workflow:
    id: stored-workflow
    name: Stored Workflow
    version: 1
    default_recovery_strategy:
      type: fail
  steps:
    - key: step
      name: Step
      role: researcher
      prompt: test.v1
      timeout_secs: 60
---
# Stored workflow
"#,
    )
    .unwrap();
    let stored = WorkflowManifest::from_file(&m31a_file, Some(root))
        .expect("canonical Markdown workflow under .m31a must load");
    assert_eq!(stored.workflow.id, "stored-workflow");

    // 4. Refuse non-Markdown workflow files.
    let toml_file = root.join("workflow.toml");
    std::fs::write(&toml_file, "workflow = {}").unwrap();
    let err4 = WorkflowManifest::from_file(&toml_file, Some(root)).unwrap_err();
    assert!(matches!(err4, WorkflowError::PathViolation { .. }));

    // 5. Refuse giant file (> 1 MB).
    let giant_file = root.join("giant.md");
    let oversized_data = vec![b' '; 1_048_577];
    std::fs::write(&giant_file, oversized_data).unwrap();
    let err5 = WorkflowManifest::from_file(&giant_file, Some(root)).unwrap_err();
    assert!(
        matches!(err5, WorkflowError::ManifestValidation { reason, .. } if reason.contains("exceeds maximum allowed limit"))
    );

    // 6. Refuse invalid UTF-8 in a canonical Markdown path.
    let invalid_utf8_file = root.join("invalid_utf8.md");
    std::fs::write(&invalid_utf8_file, [0xFF, 0xFE, 0xFD]).unwrap();
    let err6 = WorkflowManifest::from_file(&invalid_utf8_file, Some(root)).unwrap_err();
    assert!(matches!(err6, WorkflowError::ManifestParse(msg) if msg.contains("not valid UTF-8")));
}
