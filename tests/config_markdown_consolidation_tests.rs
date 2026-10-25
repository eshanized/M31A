//! Architectural consolidation tests for canonical Markdown configuration persistence,
//! path precedence, fail-closed semantics, and CLI config operations.

use m31a::config::PlatformPaths;
use m31a::config::resolved::ResolvedConfiguration;
use m31a::config::schema::{AppConfig, parse_and_validate_config, persist_config_document};
use m31a::document::split_frontmatter;
use m31a::storage::StorageLayout;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_persist_config_document_creates_and_preserves_body() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    let config_path = PlatformPaths::canonical_workspace_config_file(ws);

    let mut cfg = AppConfig::default();
    cfg.runtime.concurrency_limit = 8;
    cfg.runtime.timeout_secs = 500;

    // 1. First write to non-existent file
    persist_config_document(&config_path, &cfg).expect("persist initial config");
    assert!(config_path.is_file());

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("concurrency_limit: 8"));
    assert!(content.contains("# M31A Configuration"));

    // 2. Add custom commentary in body
    let custom_body = "\n# Custom Team Guidelines\n1. Always run verification before commit.\n";
    let (fm, _) = split_frontmatter(&content).unwrap();
    let updated_content = format!("---\n{}---\n{}", fm, custom_body);
    fs::write(&config_path, updated_content).unwrap();

    // 3. Update configuration via persist_config_document
    cfg.runtime.timeout_secs = 600;
    persist_config_document(&config_path, &cfg).expect("persist updated config");

    let round_trip_content = fs::read_to_string(&config_path).unwrap();
    assert!(round_trip_content.contains("timeout_secs: 600"));
    assert!(round_trip_content.contains("Always run verification before commit"));

    // 4. Validate parsed config round-trips
    let parsed = parse_and_validate_config(&round_trip_content).unwrap();
    assert_eq!(parsed.runtime.timeout_secs, 600);
    assert_eq!(parsed.runtime.concurrency_limit, 8);
}

#[test]
fn test_persist_config_document_fails_closed_on_corrupt_existing() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    let config_path = PlatformPaths::canonical_workspace_config_file(ws);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    // Corrupt markdown file (missing closing delimiter)
    let corrupt_content = "---\nkind: config\nschema_version: 1\nruntime:\n  invalid yaml ::::";
    fs::write(&config_path, corrupt_content).unwrap();

    let cfg = AppConfig::default();
    let err = persist_config_document(&config_path, &cfg);
    assert!(err.is_err(), "Must fail closed if existing file is corrupt");

    // Existing corrupt file must NOT have been overwritten
    let file_after = fs::read_to_string(&config_path).unwrap();
    assert_eq!(file_after, corrupt_content);
}

#[test]
fn test_persist_config_document_rejects_schema_invalid_config() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    let config_path = PlatformPaths::canonical_workspace_config_file(ws);

    let mut invalid_cfg = AppConfig::default();
    invalid_cfg.runtime.concurrency_limit = 0; // Invalid: concurrency must be > 0

    let err = persist_config_document(&config_path, &invalid_cfg);
    assert!(err.is_err(), "Must reject invalid concurrency limit");
    assert!(!config_path.exists(), "Must not write file if invalid");
}

#[test]
fn test_workspace_canonical_precedence_over_legacy() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    let layout = StorageLayout::for_workspace(ws);
    layout.ensure_workspace_dir().unwrap();

    let canonical_file = layout.canonical_workspace_config_file();
    let legacy_toml = ws.join(".m31a/config.toml");

    let legacy_content = r#"
[runtime]
concurrency_limit = 2
"#;
    fs::write(&legacy_toml, legacy_content).unwrap();

    let mut canonical_cfg = AppConfig::default();
    canonical_cfg.runtime.concurrency_limit = 12;
    persist_config_document(&canonical_file, &canonical_cfg).unwrap();

    // StorageLayout candidate resolution: canonical_file must be chosen
    let resolved_file = layout.workspace_config_file();
    assert_eq!(resolved_file, canonical_file);

    // ResolvedConfiguration must reflect the canonical concurrency_limit
    let resolved_cfg = ResolvedConfiguration::load_strict(ws).unwrap();
    assert_eq!(resolved_cfg.app_config.runtime.concurrency_limit, 12);
}

#[test]
fn test_workspace_canonical_authority_when_canonical_absent() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    let layout = StorageLayout::for_workspace(ws);
    layout.ensure_workspace_dir().unwrap();

    let legacy_toml = ws.join(".m31a/config.toml");
    let canonical_file = layout.canonical_workspace_config_file();
    let legacy_content = r#"
[runtime]
concurrency_limit = 6
"#;
    fs::write(&legacy_toml, legacy_content).unwrap();

    // Normal discovery always points to canonical config.md
    let resolved_file = layout.workspace_config_file();
    assert_eq!(resolved_file, canonical_file);

    // Candidates returns canonical first and includes legacy fallbacks
    let candidates = layout.workspace_config_candidates();
    assert_eq!(candidates[0], canonical_file);
    assert!(candidates.contains(&legacy_toml));

    // Explicit migration converts legacy TOML to canonical Markdown
    let report = m31a::config::migrate::migrate_path(&legacy_toml, false, true);
    assert_eq!(report.migrated.len(), 1);

    let resolved_cfg = ResolvedConfiguration::load_strict(ws).unwrap();
    assert_eq!(resolved_cfg.app_config.runtime.concurrency_limit, 6);
}

#[test]
fn test_workspace_corrupt_canonical_fails_closed_without_legacy_fallback() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    let layout = StorageLayout::for_workspace(ws);
    layout.ensure_workspace_dir().unwrap();

    let canonical_file = layout.canonical_workspace_config_file();
    let legacy_toml = ws.join(".m31a/config.toml");

    let corrupt_canonical =
        "---\nkind: config\nschema_version: 1\nruntime: [invalid yaml :::\n---\n";
    let valid_legacy = r#"
[runtime]
concurrency_limit = 4
"#;
    fs::write(&canonical_file, corrupt_canonical).unwrap();
    fs::write(&legacy_toml, valid_legacy).unwrap();

    // Must NOT fall back to legacy TOML if canonical Markdown exists but is corrupt
    let res = ResolvedConfiguration::load_strict(ws);
    assert!(
        res.is_err(),
        "Must fail closed if canonical config is corrupt"
    );
}
