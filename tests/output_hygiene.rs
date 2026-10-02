//! P1-02 regression: repository-level output hygiene.
//!
//! Raw `println!`/`eprintln!`/`dbg!` output is confined to operator
//! presentation modules (`interaction`, `cli`, `tui`, `main`). Model,
//! pipeline, policy, process, persistence, and telemetry modules must use
//! structured metrics (counts/bytes/status/digests) — never raw payloads —
//! and no `tracing!` call may interpolate secret-shaped fields.

use std::path::{Path, PathBuf};

/// Modules where direct console output is legitimate operator presentation.
const PRESENTATION_ALLOWLIST: &[&str] = &[
    "src/interaction",
    "src/cli",
    "src/tui",
    "src/main.rs",
    "src/report", // operator-facing report rendering (redacted at construction)
];

/// Security-critical modules that must never print raw output.
const QUIET_MODULES: &[&str] = &[
    "src/model",
    "src/agent",
    "src/pipeline",
    "src/events",
    "src/telemetry",
    "src/policy",
    "src/process",
    "src/sandbox",
    "src/verification",
    "src/persistence",
    "src/config",
    "src/prompt",
    "src/context",
    "src/capability",
    "src/scheduler",
    "src/runtime.rs",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn walk_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip build artifacts, vendored deps, and scratch space.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name == "tmp" || name == ".git" {
                continue;
            }
            walk_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn is_allowlisted(path: &Path) -> bool {
    let rel = path
        .strip_prefix(manifest_dir())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    PRESENTATION_ALLOWLIST.iter().any(|prefix| {
        rel == *prefix || rel.starts_with(&format!("{prefix}/")) || rel.starts_with(prefix)
    })
}

#[test]
fn no_raw_console_output_in_quiet_modules() {
    let root = manifest_dir().join("src");
    let mut files = Vec::new();
    walk_rs_files(&root, &mut files);
    let mut violations = Vec::new();
    for path in &files {
        let rel = path
            .strip_prefix(manifest_dir())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let in_quiet = QUIET_MODULES
            .iter()
            .any(|m| rel == *m || rel.starts_with(&format!("{m}/")) || rel.starts_with(m));
        if !in_quiet || is_allowlisted(path) {
            continue;
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        for (idx, line) in content.lines().enumerate() {
            let t = line.trim_start();
            // Allow comments mentioning the macros in prose.
            if t.starts_with("//") || t.starts_with("///") || t.starts_with("//!") {
                continue;
            }
            if line.contains("println!") || line.contains("eprintln!") || line.contains("dbg!") {
                violations.push(format!("{}:{}: {}", rel, idx + 1, t));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "raw console output in security-quiet modules (use structured metrics):\n{}",
        violations.join("\n")
    );
}

#[test]
fn no_secret_shaped_fields_in_tracing_calls() {
    let root = manifest_dir().join("src");
    let mut files = Vec::new();
    walk_rs_files(&root, &mut files);
    let mut violations = Vec::new();
    for path in &files {
        let rel = path
            .strip_prefix(manifest_dir())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let content = std::fs::read_to_string(path).unwrap_or_default();
        for (idx, line) in content.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") || t.starts_with("///") || t.starts_with("//!") {
                continue;
            }
            let lower = line.to_ascii_lowercase();
            let is_trace = lower.contains("tracing::info!")
                || lower.contains("tracing::debug!")
                || lower.contains("tracing::warn!")
                || lower.contains("tracing::error!")
                || lower.contains("info!(")
                || lower.contains("debug!(");
            if !is_trace {
                continue;
            }
            // Allow lines that are ABOUT redaction/sanitization/masking.
            if lower.contains("redact")
                || lower.contains("sanitiz")
                || lower.contains("scrub")
                || lower.contains("mask")
                || lower.contains("forbidden")
                || lower.contains("digest")
            {
                continue;
            }
            if lower.contains("api_key")
                || lower.contains("bearer")
                || lower.contains("passwd")
                || (lower.contains("secret") && !lower.contains("secret_redactor"))
                || lower.contains("nvidia_api")
            {
                violations.push(format!("{}:{}: {}", rel, idx + 1, t));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "secret-shaped fields in tracing calls:\n{}",
        violations.join("\n")
    );
}

#[test]
fn telemetry_uses_metrics_not_payloads() {
    // Structural spot-check: the pipeline telemetry stage must record
    // byte counts and digests, never raw output strings.
    let stage = manifest_dir().join("src/pipeline/stages/telemetry_record.rs");
    let content = std::fs::read_to_string(&stage).expect("telemetry stage source");
    assert!(
        content.contains("output_bytes") || content.contains("audit_digest"),
        "telemetry stage must record structural metrics"
    );
}
