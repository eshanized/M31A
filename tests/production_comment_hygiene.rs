//! Automated production comment hygiene regression test.
//!
//! Enforces:
//! 1. Zero roadmap phase references (`phase-44`, `phase 45`, `phase-specific`, etc.) in production source code comments (`src/`).
//! 2. Preservation of legitimate domain/algorithmic uses:
//!    - Algorithmic uses of "two-phase" (e.g. `TwoPhaseBudgetEnforcer`, two-phase commit, two-phase signal escalation, candidate-seal protocol).
//!    - Domain planning entities synthesized for target projects (`PHASE-01`, `PHASE-02..N` in `src/workflow/planning/roadmap.rs`).
//!    - Domain identifier `RoadmapPhase`.
//! 3. Prohibits historical roadmap prose: `phase-specific`, `introduced in phase`, `added in phase`, `fixed in phase`, `required by phase`, `phase hardening`, `phase recovery`, `phase security`, `phase release`.

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, files);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
}

#[test]
fn test_production_comments_zero_roadmap_leakage() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);
    assert!(!files.is_empty(), "src/ directory must contain .rs files");

    // Prohibited roadmap phrases across production code comments
    let prohibited_phrases = [
        "phase-specific",
        "phase specific",
        "introduced in phase",
        "added in phase",
        "fixed in phase",
        "required by phase",
        "phase hardening",
        "phase recovery",
        "phase security",
        "phase release",
        "phase requirement",
    ];

    // Regex to detect "phase <number>" or "phase-<number>" or "phase_<number>"
    let phase_num_regex = Regex::new(r"(?i)\bphase[-_\s]*\d+\b").unwrap();

    let mut violations = Vec::new();

    for file in &files {
        let content = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file.display()));

        let rel_path = file.strip_prefix(&manifest_dir).unwrap_or(file);

        for (line_idx, line) in content.lines().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            // Only examine comments (single-line or doc comments or block comment starts)
            let is_comment =
                trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*");

            if !is_comment {
                continue;
            }

            // Exemption: Target project roadmap synthesis in roadmap.rs
            if rel_path.ends_with("src/workflow/planning/roadmap.rs")
                && (trimmed.contains("PHASE-01") || trimmed.contains("PHASE-02..N"))
            {
                continue;
            }

            // Check for prohibited phrases
            let lower = trimmed.to_lowercase();
            for phrase in &prohibited_phrases {
                if lower.contains(phrase) {
                    violations.push(format!(
                        "{}:{}: contains prohibited roadmap phrase '{}': {}",
                        rel_path.display(),
                        line_num,
                        phrase,
                        trimmed
                    ));
                }
            }

            // Check for phase numbers (ignoring allowed algorithmic terms like 'two-phase')
            if phase_num_regex.is_match(trimmed) {
                // If the only match is part of an allowed algorithmic pattern or identifier
                let cleaned = trimmed
                    .replace("two-phase", "")
                    .replace("Two-phase", "")
                    .replace("TWO-PHASE", "");
                if phase_num_regex.is_match(&cleaned) {
                    violations.push(format!(
                        "{}:{}: contains roadmap phase number reference: {}",
                        rel_path.display(),
                        line_num,
                        trimmed
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        let count = violations.len();
        let report = violations.join("\n");
        panic!(
            "Production comment hygiene check failed with {count} violation(s):\n{report}\n\nAll production comments in src/ must explain engineering intent (WHY) rather than roadmap history."
        );
    }
}
