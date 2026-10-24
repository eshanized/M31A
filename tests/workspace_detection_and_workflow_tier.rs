//! Regression test suite for Phase 2, 3, and 4 remediation:
//! - Phase 2: Independent WorkspaceMode and WorkflowTier contracts
//! - Phase 3: Evidence-driven Brownfield / Greenfield detection matrix (14 scenarios)
//! - Phase 4: Workflow tier classification truth table and invariant enforcement

use m31a::workflow::genesis::{
    GenesisError, GenesisMode, WorkflowTier, WorkspaceEnvironment, WorkspaceMode,
    discovery::classify_workflow_tier,
};
use std::fs::{create_dir_all, write};
use std::process::Command;
use tempfile::tempdir;

// ============================================================================
// PHASE 3 — EVIDENCE-DRIVEN DETECTION MATRIX (14 SCENARIOS)
// ============================================================================

#[test]
fn test_matrix_01_empty_directory_is_greenfield() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Greenfield);
    assert_eq!(env.detected_mode, GenesisMode::Greenfield);
    assert!(env.is_greenfield());
    assert!(!env.is_brownfield());
    assert_eq!(env.file_count, 0);
}

#[test]
fn test_matrix_02_readme_only_is_greenfield() {
    let dir = tempdir().unwrap();
    write(dir.path().join("README.md"), "# New Project").unwrap();
    write(dir.path().join("LICENSE"), "MIT").unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Greenfield);
    assert!(env.is_greenfield());
    assert!(!env.is_brownfield());
}

#[test]
fn test_matrix_03_cargo_project_is_brownfield() {
    let dir = tempdir().unwrap();
    write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
    assert!(!env.is_greenfield());
    assert!(
        env.detected_stack
            .as_deref()
            .unwrap_or("")
            .contains("Rust (Cargo)")
    );
}

#[test]
fn test_matrix_04_node_project_is_brownfield() {
    let dir = tempdir().unwrap();
    write(dir.path().join("package.json"), "{\"name\": \"pkg\"}").unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
    assert!(
        env.detected_stack
            .as_deref()
            .unwrap_or("")
            .contains("Node.js")
    );
}

#[test]
fn test_matrix_05_python_project_is_brownfield() {
    let dir = tempdir().unwrap();
    write(dir.path().join("requirements.txt"), "requests>=2.0").unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
    assert!(
        env.detected_stack
            .as_deref()
            .unwrap_or("")
            .contains("Python")
    );
}

#[test]
fn test_matrix_06_go_project_is_brownfield() {
    let dir = tempdir().unwrap();
    write(
        dir.path().join("go.mod"),
        "module example.com/mod\n\ngo 1.22\n",
    )
    .unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
    assert!(env.detected_stack.as_deref().unwrap_or("").contains("Go"));
}

#[test]
fn test_matrix_07_nested_source_tree_is_brownfield() {
    let dir = tempdir().unwrap();
    let deep = dir.path().join("backend").join("services").join("api");
    create_dir_all(&deep).unwrap();
    write(deep.join("handler.rs"), "pub fn handle() {}").unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
}

#[test]
fn test_matrix_08_monorepo_is_brownfield() {
    let dir = tempdir().unwrap();
    let app = dir.path().join("packages").join("web");
    create_dir_all(&app).unwrap();
    write(app.join("package.json"), "{\"name\": \"@app/web\"}").unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
}

#[test]
fn test_matrix_09_git_repo_with_unusual_layout_is_brownfield() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let _ = Command::new("git")
        .args(["init"])
        .current_dir(root)
        .status();
    let deep = root.join("custom").join("scripts");
    create_dir_all(&deep).unwrap();
    write(deep.join("deploy.py"), "print('deploy')").unwrap();
    let env = WorkspaceEnvironment::probe(root).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
    assert!(env.has_git);
}

#[test]
fn test_matrix_10_inaccessible_workspace_fails_closed() {
    let non_existent = std::path::Path::new("/tmp/m31a_definitely_does_not_exist_9823749823");
    let res = WorkspaceEnvironment::probe(non_existent);
    assert!(res.is_err());
    match res.unwrap_err() {
        GenesisError::EnvironmentProbeFailed(msg) => {
            assert!(msg.contains("does not exist"));
        }
        other => panic!("expected EnvironmentProbeFailed, got {:?}", other),
    }
}

#[test]
fn test_matrix_11_probe_failure_is_explicit_error() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("not_a_dir.txt");
    write(&file_path, "regular file").unwrap();
    // Probing a regular file instead of a directory must fail closed
    let res = WorkspaceEnvironment::probe(&file_path);
    assert!(res.is_err());
    match res.unwrap_err() {
        GenesisError::EnvironmentProbeFailed(msg) => {
            assert!(msg.contains("not a directory"));
        }
        other => panic!("expected EnvironmentProbeFailed, got {:?}", other),
    }
}

#[test]
fn test_matrix_12_source_code_without_manifest_is_brownfield() {
    let dir = tempdir().unwrap();
    write(dir.path().join("main.py"), "print('hello world')").unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
}

#[test]
fn test_matrix_13_manifest_without_source_is_brownfield() {
    let dir = tempdir().unwrap();
    write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"manifest-only\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
}

#[test]
fn test_matrix_14_existing_project_with_ci_files_is_brownfield() {
    let dir = tempdir().unwrap();
    let gh = dir.path().join(".github").join("workflows");
    create_dir_all(&gh).unwrap();
    write(gh.join("ci.yml"), "name: CI\non: push\n").unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert_eq!(env.workspace_mode, WorkspaceMode::Brownfield);
    assert!(env.is_brownfield());
}

// ============================================================================
// PHASE 2 & 4 — WORKFLOW CLASSIFICATION TRUTH TABLE & SEPARATION OF CONCERNS
// ============================================================================

#[test]
fn test_classification_truth_table() {
    // 1. Greenfield empty workspace
    let greenfield_dir = tempdir().unwrap();
    let greenfield_env = WorkspaceEnvironment::probe(greenfield_dir.path()).unwrap();

    // 2. Brownfield existing workspace
    let brownfield_dir = tempdir().unwrap();
    write(
        brownfield_dir.path().join("Cargo.toml"),
        "[package]\nname = \"existing\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    create_dir_all(brownfield_dir.path().join("src")).unwrap();
    write(
        brownfield_dir.path().join("src/lib.rs"),
        "pub fn existing() {}",
    )
    .unwrap();
    let brownfield_env = WorkspaceEnvironment::probe(brownfield_dir.path()).unwrap();

    // CASE A: existing repository + tiny change = Brownfield + Tiny
    let tier_a = classify_workflow_tier("inspect repository and check status", &brownfield_env);
    assert_eq!(tier_a, WorkflowTier::Tiny);
    assert!(brownfield_env.is_brownfield());

    // CASE B: existing repository + add authentication = Brownfield + Medium (NOT Greenfield!)
    let tier_b = classify_workflow_tier(
        "build authentication service with session tokens",
        &brownfield_env,
    );
    assert_eq!(tier_b, WorkflowTier::Medium);
    assert_ne!(tier_b, WorkflowTier::Standard);
    assert_ne!(tier_b, WorkflowTier::Greenfield);
    assert!(brownfield_env.is_brownfield());

    // CASE C: existing repository + rewrite persistence = Brownfield + Consequential
    let tier_c = classify_workflow_tier("rewrite storage layer to postgres", &brownfield_env);
    assert_eq!(tier_c, WorkflowTier::Consequential);
    assert!(brownfield_env.is_brownfield());

    // CASE D: empty workspace + build application = Greenfield + Standard
    let tier_d = classify_workflow_tier("build expense tracking application", &greenfield_env);
    assert_eq!(tier_d, WorkflowTier::Standard);
    assert!(tier_d.is_standard());
    assert!(greenfield_env.is_greenfield());

    // CASE E: read-only queries do NOT trigger unnecessary genesis
    let tier_e = classify_workflow_tier("inspect repository dependencies", &brownfield_env);
    assert_eq!(tier_e, WorkflowTier::Tiny);

    // CASE F: fix typo does NOT trigger architecture generation
    let tier_f = classify_workflow_tier("fix typo in README", &brownfield_env);
    assert_eq!(tier_f, WorkflowTier::Medium); // Medium fast-path converges immediately, skips genesis
    assert_ne!(tier_f, WorkflowTier::Consequential);
    assert_ne!(tier_f, WorkflowTier::Standard);
}
