//! Test suite for Project Genesis environment probing and workspace detection (Package 4).

use m31a::workflow::genesis::{EnvironmentCategory, GenesisMode, WorkspaceEnvironment};
use std::fs::{create_dir_all, write};
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_probe_empty_directory_detects_greenfield() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();

    assert!(env.is_greenfield());
    assert_eq!(env.detected_mode, GenesisMode::Greenfield);
    assert_eq!(env.file_count, 0);
    assert!(!env.has_git);

    assert!(env.has_fact("platform.os"));
    assert!(env.has_fact("platform.arch"));
    assert_eq!(env.get_fact("git.present"), Some("false"));
}

#[test]
fn test_probe_rust_workspace_detects_brownfield() {
    let dir = tempdir().unwrap();
    let cargo_toml = r#"
[package]
name = "sample-app"
version = "0.1.0"
edition = "2021"
"#;
    write(dir.path().join("Cargo.toml"), cargo_toml).unwrap();
    create_dir_all(dir.path().join("src")).unwrap();
    write(
        dir.path().join("src/main.rs"),
        "fn main() { println!(\"hello\"); }",
    )
    .unwrap();

    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();

    assert!(!env.is_greenfield());
    assert_eq!(env.detected_mode, GenesisMode::Brownfield);
    assert_eq!(env.get_fact("manifest.cargo"), Some("Cargo.toml"));
    assert!(
        env.detected_stack
            .as_deref()
            .unwrap_or("")
            .contains("Rust (Cargo)")
    );
}

#[test]
fn test_probe_nodejs_workspace_detects_brownfield() {
    let dir = tempdir().unwrap();
    let pkg_json = r#"{"name": "test-pkg", "version": "1.0.0"}"#;
    write(dir.path().join("package.json"), pkg_json).unwrap();
    create_dir_all(dir.path().join("src")).unwrap();
    write(dir.path().join("src/index.js"), "console.log('hi');").unwrap();

    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();

    assert!(!env.is_greenfield());
    assert_eq!(env.detected_mode, GenesisMode::Brownfield);
    assert_eq!(env.get_fact("manifest.package_json"), Some("package.json"));
    assert!(
        env.detected_stack
            .as_deref()
            .unwrap_or("")
            .contains("Node.js")
    );
}

#[test]
fn test_probe_python_workspace_detects_brownfield() {
    let dir = tempdir().unwrap();
    write(
        dir.path().join("pyproject.toml"),
        "[project]\nname='demo'\n",
    )
    .unwrap();
    write(dir.path().join("main.py"), "print('hello')").unwrap();

    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();

    assert!(!env.is_greenfield());
    assert_eq!(env.detected_mode, GenesisMode::Brownfield);
    assert!(
        env.detected_stack
            .as_deref()
            .unwrap_or("")
            .contains("Python")
    );
}

#[test]
fn test_probe_git_repository_state() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Initialize git
    let init_ok = Command::new("git")
        .args(["init"])
        .current_dir(root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !init_ok {
        return; // Skip if git binary is not installed
    }

    let _ = Command::new("git")
        .args(["config", "user.email", "test@m31a.local"])
        .current_dir(root)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.name", "M31A Test"])
        .current_dir(root)
        .status();

    write(root.join("README.md"), "# Test Project").unwrap();

    let env_dirty = WorkspaceEnvironment::probe(root).unwrap();
    assert!(env_dirty.has_git);
    assert_eq!(env_dirty.get_fact("git.present"), Some("true"));
    assert_eq!(env_dirty.get_fact("git.dirty"), Some("true"));

    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .status();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(root)
        .status();

    let env_clean = WorkspaceEnvironment::probe(root).unwrap();
    assert!(env_clean.has_git);
    assert_eq!(env_clean.get_fact("git.dirty"), Some("false"));
    assert!(env_clean.has_fact("git.branch"));
}

#[test]
fn test_host_toolchain_facts_recorded() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();

    let toolchains = env.facts_in_category(EnvironmentCategory::Toolchain);
    // Since rustc/cargo are running this test, they should be present
    assert!(!toolchains.is_empty());
    assert!(env.has_fact("toolchain.cargo") || env.has_fact("toolchain.rustc"));
}
