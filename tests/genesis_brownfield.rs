//! Test suite for Brownfield codebase scanning, topology mapping, and BROWNFIELD.md projection (Package 4).

use m31a::workflow::genesis::BrownfieldMap;
use std::fs::{create_dir_all, write};
use tempfile::tempdir;

#[test]
fn test_brownfield_scan_rust_codebase() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let cargo_toml = r#"
[package]
name = "m31a-brownfield-test"
version = "0.1.0"
edition = "2021"
"#;
    write(root.join("Cargo.toml"), cargo_toml).unwrap();
    create_dir_all(root.join("src/models")).unwrap();
    create_dir_all(root.join("src/controllers")).unwrap();
    write(root.join("src/main.rs"), "fn main() {}").unwrap();
    write(root.join("src/lib.rs"), "pub mod models;").unwrap();
    write(root.join("src/models/user.rs"), "pub struct User;").unwrap();
    write(root.join("src/controllers/api.rs"), "pub fn handler() {}").unwrap();

    let bmap = BrownfieldMap::scan(root).unwrap();

    assert_eq!(bmap.topology.primary_language, "Rust");
    assert!(
        bmap.topology
            .package_manifests
            .contains(&"Cargo.toml".to_string())
    );
    assert!(
        bmap.topology
            .entry_points
            .contains(&"src/main.rs".to_string())
    );
    assert!(
        bmap.topology
            .entry_points
            .contains(&"src/lib.rs".to_string())
    );
    assert!(
        bmap.topology
            .module_tree
            .iter()
            .any(|m| m.contains("models"))
    );
    assert!(
        bmap.topology
            .test_frameworks
            .contains(&"cargo test".to_string())
    );

    let md = bmap.to_markdown();
    assert!(md.contains("# Brownfield Codebase Map"));
    assert!(md.contains("- **Language**: Rust"));
    assert!(md.contains("- **Entry Points**:"));
    assert!(md.contains("## Subsystem Boundaries"));
}

#[test]
fn test_brownfield_scan_typescript_codebase() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    write(
        root.join("package.json"),
        r#"{"name": "my-ts-app", "version": "1.0.0"}"#,
    )
    .unwrap();
    create_dir_all(root.join("src")).unwrap();
    write(root.join("src/index.ts"), "console.log('hello');").unwrap();

    let bmap = BrownfieldMap::scan(root).unwrap();

    assert_eq!(bmap.topology.primary_language, "TypeScript / JavaScript");
    assert!(
        bmap.topology
            .package_manifests
            .contains(&"package.json".to_string())
    );
}

#[test]
fn test_brownfield_delta_scope_and_save_projection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    write(
        root.join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\n",
    )
    .unwrap();

    let mut bmap = BrownfieldMap::scan(root).unwrap();
    bmap.delta_scope = Some("Add OAuth2 and WebAuthn authentication workflows".to_string());

    let path = bmap.save_to_dir(root, ".planning").unwrap();
    assert!(path.exists());
    assert_eq!(path, root.join(".planning/BROWNFIELD.md"));

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("## Delta Scope & Extension Targets"));
    assert!(content.contains("Add OAuth2 and WebAuthn authentication workflows"));
}
