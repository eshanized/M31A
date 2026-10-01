//! Integration test suite for WorkflowCompiler and provenance anchoring (Package 2).

use m31a::prompt::InMemoryPromptCatalog;
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::compiler::WorkflowCompiler;
use m31a::workflow::error::WorkflowError;
use m31a::workflow::manifest::WorkflowManifest;
use std::path::Path;

#[test]
fn test_compiler_success_with_builtin_prompts() {
    let toml_str = r#"
manifest_version = 1

[workflow]
id = "genesis_sample"
name = "Genesis Sample Workflow"
description = "Sample workflow"
version = 1
default_recovery_strategy = { type = "fail" }

[[steps]]
key = "discovery"
name = "Project Discovery"
role = "researcher"
prompt = "genesis.discovery.v1"
timeout_secs = 1800
allows_parallelism = false
depends_on = []

[steps.quality_gate]
schema = "discovery.v1"
required_artifacts = ["DISCOVERY.md"]
require_human_approval = false

[[steps.outputs]]
artifact_name = "DISCOVERY.md"
relative_path = ".planning/DISCOVERY.md"
schema_type = "markdown.discovery.v1"

[[steps]]
key = "architecture"
name = "System Architecture"
role = "architect"
prompt = "genesis.architecture.v1"
timeout_secs = 1200
allows_parallelism = false
depends_on = ["discovery"]

[[steps.inputs]]
parameter_name = "requirements_doc"
source_step_key = "discovery"
artifact_name = "DISCOVERY.md"
is_optional = false

[steps.quality_gate]
schema = "architecture.v1"
required_artifacts = ["ARCHITECTURE.md"]
require_human_approval = true

[[steps.outputs]]
artifact_name = "ARCHITECTURE.md"
relative_path = ".planning/ARCHITECTURE.md"
schema_type = "markdown.architecture.v1"
"#;

    let manifest = WorkflowManifest::from_toml_str(toml_str).unwrap();
    let catalog = InMemoryPromptCatalog::with_builtins();
    let compiler = WorkflowCompiler::new(&catalog);

    let compiled = compiler
        .compile(&manifest, Some("workflows/sample.toml"), None)
        .expect("compilation must succeed");

    // 1. Definition verification
    assert_eq!(compiled.definition.id, "genesis_sample");
    assert_eq!(compiled.definition.steps.len(), 2);
    assert_eq!(
        compiled.topological_order,
        vec!["discovery", "architecture"]
    );

    // 2. Provenance verification
    let prov = &compiled.provenance;
    assert_eq!(prov.manifest_id, "genesis_sample");
    assert_eq!(prov.manifest_version, 1);
    assert_eq!(prov.source_path, Some("workflows/sample.toml".to_string()));
    assert_eq!(prov.step_provenance.len(), 2);

    let disc_prov = prov.step_provenance.get("discovery").unwrap();
    assert_eq!(disc_prov.prompt_id, "genesis.discovery");
    assert_eq!(disc_prov.prompt_version, 1);
    assert_eq!(disc_prov.role, AgentRole::researcher());
    assert!(!disc_prov.prompt_content_hash.is_empty());

    let arch_prov = prov.step_provenance.get("architecture").unwrap();
    assert_eq!(arch_prov.prompt_id, "genesis.architecture");
    assert_eq!(arch_prov.prompt_version, 1);
    assert_eq!(arch_prov.role, AgentRole::architect());

    assert_eq!(prov.composite_hash.len(), 64);
}

#[test]
fn test_compiler_compiles_example_greenfield_manifest() {
    let manifest_path = Path::new("examples/workflows/greenfield.toml");
    let manifest = WorkflowManifest::from_file(manifest_path, None).unwrap();

    let catalog = InMemoryPromptCatalog::with_builtins();
    let compiler = WorkflowCompiler::new(&catalog);

    let compiled = compiler
        .compile(&manifest, Some("examples/workflows/greenfield.toml"), None)
        .expect("greenfield compilation must succeed");

    assert_eq!(compiled.definition.id, "greenfield");
    assert_eq!(
        compiled.topological_order,
        vec!["discovery", "research_stack"]
    );
}

#[test]
fn test_compiler_fails_closed_on_missing_prompt() {
    let toml_str = r#"
manifest_version = 1

[workflow]
id = "missing_prompt_wf"
name = "Missing Prompt"
version = 1

[[steps]]
key = "discovery"
name = "Discovery"
role = "researcher"
prompt = "non_existent.prompt.v1"
"#;

    let manifest = WorkflowManifest::from_toml_str(toml_str).unwrap();
    let catalog = InMemoryPromptCatalog::with_builtins();
    let compiler = WorkflowCompiler::new(&catalog);

    let err = compiler.compile(&manifest, None, None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::PromptNotFound { id, version } if id == "non_existent.prompt" && version == 1)
    );
}

#[test]
fn test_compiler_fails_closed_on_role_mismatch() {
    let toml_str = r#"
manifest_version = 1

[workflow]
id = "role_mismatch_wf"
name = "Role Mismatch"
version = 1

[[steps]]
key = "discovery"
name = "Discovery"
role = "implementer" # Step demands Implementer
prompt = "genesis.discovery.v1" # Prompt contract is Researcher -> Mismatch!
"#;

    let manifest = WorkflowManifest::from_toml_str(toml_str).unwrap();
    let catalog = InMemoryPromptCatalog::with_builtins();
    let compiler = WorkflowCompiler::new(&catalog);

    let err = compiler.compile(&manifest, None, None).unwrap_err();
    assert!(
        matches!(err, WorkflowError::RoleMismatch { step_key, step_role, prompt_id, prompt_role }
        if step_key == "discovery" && step_role == "implementer" && prompt_id == "genesis.discovery" && prompt_role == "researcher")
    );
}

#[test]
fn test_compiler_determinism_across_runs() {
    let manifest_path = Path::new("examples/workflows/greenfield.toml");
    let manifest = WorkflowManifest::from_file(manifest_path, None).unwrap();

    let catalog = InMemoryPromptCatalog::with_builtins();
    let compiler = WorkflowCompiler::new(&catalog);

    let compiled1 = compiler.compile(&manifest, None, None).unwrap();
    let compiled2 = compiler.compile(&manifest, None, None).unwrap();

    assert_eq!(compiled1.definition, compiled2.definition);
    assert_eq!(compiled1.topological_order, compiled2.topological_order);
    assert_eq!(
        compiled1.provenance.manifest_hash,
        compiled2.provenance.manifest_hash
    );
    assert_eq!(
        compiled1.provenance.composite_hash,
        compiled2.provenance.composite_hash
    );
    assert_eq!(
        compiled1.provenance.step_provenance,
        compiled2.provenance.step_provenance
    );
}
