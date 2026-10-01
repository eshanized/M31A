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
fn test_valid_manifest_from_toml_str() {
    let toml_str = r#"
manifest_version = 1

[workflow]
id = "test_wf"
name = "Test Workflow"
description = "A valid test workflow"
version = 1
default_recovery_strategy = { type = "fail" }

[[steps]]
key = "step_a"
name = "First Step"
role = "researcher"
prompt = "genesis.discovery.v1"
timeout_secs = 600
allows_parallelism = false
depends_on = []

[[steps.outputs]]
artifact_name = "A.md"
relative_path = "docs/A.md"
schema_type = "markdown"

[[steps]]
key = "step_b"
name = "Second Step"
role = "architect"
prompt = "genesis.architecture.v1"
timeout_secs = 300
allows_parallelism = true
depends_on = ["step_a"]

[[steps.inputs]]
parameter_name = "input_a"
source_step_key = "step_a"
artifact_name = "A.md"
is_optional = false

[[steps.outputs]]
artifact_name = "B.md"
relative_path = "docs/B.md"
schema_type = "markdown"
"#;

    let manifest = WorkflowManifest::from_toml_str(toml_str).expect("parse valid manifest");
    assert_eq!(manifest.manifest_version, CURRENT_MANIFEST_VERSION);
    assert_eq!(manifest.workflow.id, "test_wf");
    assert_eq!(manifest.steps.len(), 2);
    assert_eq!(manifest.steps[0].role, AgentRole::researcher());
    assert_eq!(manifest.steps[1].role, AgentRole::architect());
    manifest.validate(None).expect("validation should succeed");
}

#[test]
fn test_example_greenfield_manifest_file() {
    let manifest_path = Path::new("examples/workflows/greenfield.toml");
    assert!(manifest_path.exists(), "example greenfield.toml must exist");

    let manifest = WorkflowManifest::from_file(manifest_path, None)
        .expect("load and validate greenfield manifest");
    assert_eq!(manifest.workflow.id, "greenfield");
    assert_eq!(manifest.steps.len(), 2);
    assert_eq!(manifest.steps[0].key, "discovery");
    assert_eq!(manifest.steps[1].key, "research_stack");
}

#[test]
fn test_manifest_rejects_malformed_toml() {
    let bad_toml = "workflow = { id = unquoted_bad_syntax";
    let err = WorkflowManifest::from_toml_str(bad_toml).unwrap_err();
    assert!(matches!(err, WorkflowError::ManifestParse(_)));
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
    let toml_str = r#"
manifest_version = 1

[workflow]
id = "dup_test"
name = "Dup Test"
version = 1

[[steps]]
key = "alpha"
name = "First Alpha"
role = "researcher"
prompt = "test.v1"

[[steps]]
key = "alpha"
name = "Second Alpha"
role = "implementer"
prompt = "test.v1"
"#;

    let manifest = WorkflowManifest::from_toml_str(toml_str).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::ManifestValidation { field: Some(f), reason, .. } if f == "key" && reason.contains("duplicate"))
    );
}

#[test]
fn test_manifest_rejects_unknown_and_self_dependencies() {
    // Self-dependency
    let toml_self = r#"
manifest_version = 1
[workflow]
id = "self_dep"
name = "Self Dep"
version = 1

[[steps]]
key = "alpha"
name = "Alpha"
role = "researcher"
prompt = "test.v1"
depends_on = ["alpha"]
"#;
    let manifest = WorkflowManifest::from_toml_str(toml_self).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::ManifestValidation { field: Some(f), reason, .. } if f == "depends_on" && reason.contains("itself"))
    );

    // Missing dependency
    let toml_missing = r#"
manifest_version = 1
[workflow]
id = "missing_dep"
name = "Missing Dep"
version = 1

[[steps]]
key = "alpha"
name = "Alpha"
role = "researcher"
prompt = "test.v1"
depends_on = ["non_existent"]
"#;
    let manifest2 = WorkflowManifest::from_toml_str(toml_missing).unwrap();
    let err2 = manifest2.validate(None).unwrap_err();
    assert!(
        matches!(err2, WorkflowError::ManifestValidation { field: Some(f), reason, .. } if f == "depends_on" && reason.contains("not found"))
    );
}

#[test]
fn test_manifest_rejects_dependency_cycle() {
    let toml_cycle = r#"
manifest_version = 1
[workflow]
id = "cycle_wf"
name = "Cycle Test"
version = 1

[[steps]]
key = "alpha"
name = "Alpha"
role = "researcher"
prompt = "test.v1"
depends_on = ["gamma"]

[[steps]]
key = "beta"
name = "Beta"
role = "researcher"
prompt = "test.v1"
depends_on = ["alpha"]

[[steps]]
key = "gamma"
name = "Gamma"
role = "researcher"
prompt = "test.v1"
depends_on = ["beta"]
"#;

    let manifest = WorkflowManifest::from_toml_str(toml_cycle).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(matches!(err, WorkflowError::CycleDetected { cycle } if cycle.len() == 3));
}

#[test]
fn test_manifest_rejects_invalid_timeout() {
    let toml_zero = r#"
manifest_version = 1
[workflow]
id = "timeout_wf"
name = "Timeout Test"
version = 1

[[steps]]
key = "alpha"
name = "Alpha"
role = "researcher"
prompt = "test.v1"
timeout_secs = 0
"#;
    let manifest = WorkflowManifest::from_toml_str(toml_zero).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::ManifestValidation { field: Some(f), .. } if f == "timeout_secs")
    );
}

#[test]
fn test_manifest_rejects_invalid_artifact_paths() {
    // Absolute path
    let toml_abs = r#"
manifest_version = 1
[workflow]
id = "abs_path"
name = "Abs Path"
version = 1

[[steps]]
key = "alpha"
name = "Alpha"
role = "researcher"
prompt = "test.v1"

[[steps.outputs]]
artifact_name = "bad.md"
relative_path = "/etc/passwd"
schema_type = "markdown"
"#;
    let manifest = WorkflowManifest::from_toml_str(toml_abs).unwrap();
    let err = manifest.validate(None).unwrap_err();
    assert!(matches!(err, WorkflowError::InvalidArtifactPath { .. }));

    // Traversal ..
    let toml_trav = r#"
manifest_version = 1
[workflow]
id = "trav_path"
name = "Traversal Path"
version = 1

[[steps]]
key = "alpha"
name = "Alpha"
role = "researcher"
prompt = "test.v1"

[[steps.outputs]]
artifact_name = "bad.md"
relative_path = "docs/../../secret.txt"
schema_type = "markdown"
"#;
    let manifest2 = WorkflowManifest::from_toml_str(toml_trav).unwrap();
    let err2 = manifest2.validate(None).unwrap_err();
    assert!(matches!(err2, WorkflowError::InvalidArtifactPath { .. }));
}

#[test]
fn test_manifest_file_security_checks() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // 1. Refuse path with ..
    let bad_path = root.join("../escape.toml");
    let err = WorkflowManifest::from_file(&bad_path, Some(root)).unwrap_err();
    assert!(matches!(err, WorkflowError::PathViolation { .. }));

    // 2. Refuse path inside .git
    let git_dir = root.join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    let git_file = git_dir.join("workflow.toml");
    std::fs::write(&git_file, "content").unwrap();
    let err2 = WorkflowManifest::from_file(&git_file, Some(root)).unwrap_err();
    assert!(matches!(err2, WorkflowError::PathViolation { .. }));

    // 3. Refuse path inside .m31a
    let m31a_dir = root.join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let m31a_file = m31a_dir.join("workflow.toml");
    std::fs::write(&m31a_file, "content").unwrap();
    let err3 = WorkflowManifest::from_file(&m31a_file, Some(root)).unwrap_err();
    assert!(matches!(err3, WorkflowError::PathViolation { .. }));

    // 4. Refuse giant file (> 1 MB)
    let giant_file = root.join("giant.toml");
    let oversized_data = vec![b' '; 1_048_577];
    std::fs::write(&giant_file, oversized_data).unwrap();
    let err4 = WorkflowManifest::from_file(&giant_file, Some(root)).unwrap_err();
    assert!(
        matches!(err4, WorkflowError::ManifestValidation { reason, .. } if reason.contains("exceeds maximum allowed limit"))
    );

    // 5. Refuse invalid UTF-8
    let invalid_utf8_file = root.join("invalid_utf8.toml");
    std::fs::write(&invalid_utf8_file, [0xFF, 0xFE, 0xFD]).unwrap();
    let err5 = WorkflowManifest::from_file(&invalid_utf8_file, Some(root)).unwrap_err();
    assert!(matches!(err5, WorkflowError::ManifestParse(msg) if msg.contains("not valid UTF-8")));
}
