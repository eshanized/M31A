//! Dedicated integration tests for `m31a-watcher` (src/filter.rs).
//!
//! Validates:
//! - Default ignored directory rules (.git, target, node_modules, etc.)
//! - User-defined additional directory ignore patterns
//! - File ignore rules (dotfiles with .env exception, editor tilde backups, noise extensions)
//! - Case-insensitivity of filtered extensions

use m31a_watcher::filter::WatcherFilter;
use std::path::Path;

#[test]
fn test_default_ignored_directories() {
    let filter = WatcherFilter::default();

    let standard_ignored = [
        ".git", "target", "node_modules", ".m31a", "dist", "build",
        "out", ".cache", "__pycache__", ".pytest_cache", ".venv", "venv",
    ];

    for dir in standard_ignored {
        assert!(filter.should_ignore_dir(dir), "Directory {} should be ignored", dir);
    }

    let non_ignored = ["src", "crates", "tests", "docs", "pkg", "public"];
    for dir in non_ignored {
        assert!(!filter.should_ignore_dir(dir), "Directory {} should NOT be ignored", dir);
    }
}

#[test]
fn test_custom_additional_ignore_directories() {
    let filter = WatcherFilter::new()
        .with_additional_ignore("coverage")
        .with_additional_ignore("tmp_artifacts");

    assert!(filter.should_ignore_dir("coverage"));
    assert!(filter.should_ignore_dir("tmp_artifacts"));
    assert!(filter.should_ignore_dir("target")); // Retains defaults
    assert!(!filter.should_ignore_dir("src"));
}

#[test]
fn test_should_ignore_file_rules() {
    let filter = WatcherFilter::default();

    // Dotfiles are ignored except .env
    assert!(filter.should_ignore_file(Path::new(".DS_Store")));
    assert!(filter.should_ignore_file(Path::new(".bash_history")));
    assert!(filter.should_ignore_file(Path::new(".gitkeep")));
    assert!(!filter.should_ignore_file(Path::new(".env")), ".env must be preserved for environment discovery");

    // Editor backup files
    assert!(filter.should_ignore_file(Path::new("main.rs~")));
    assert!(filter.should_ignore_file(Path::new("config.toml~")));

    // Filtered extensions (case-insensitive)
    let ignored_files = [
        "doc.swp", "data.tmp", "archive.bak", "Cargo.lock", "compiled.pyc",
        "doc.SWP", "data.TMP", "archive.BAK", "Cargo.LOCK", "compiled.PYC",
    ];
    for f in ignored_files {
        assert!(filter.should_ignore_file(Path::new(f)), "File {} should be ignored", f);
    }

    // Normal source and resource files must NOT be ignored
    let allowed_files = [
        "main.rs", "lib.rs", "package.json", "Cargo.toml", "index.html",
        "styles.css", "schema.sql", "README.md",
    ];
    for f in allowed_files {
        assert!(!filter.should_ignore_file(Path::new(f)), "File {} should NOT be ignored", f);
    }
}
