//! Phase 19 — Process Execution Authority Regression Guard
//!
//! Enforces Phase 17 invariants:
//! 1. Orchestration layers (controller, runtime, scheduler, cli, interaction)
//!    must NOT directly construct `std::process::Command` or `tokio::process::Command`.
//! 2. Process execution must flow through capability providers, sandbox providers,
//!    or the process subsystem (which itself is below orchestration).
//! 3. The only acceptable `Command::new` sites are:
//!    - `src/capability/providers/` (capability layer)
//!    - `src/sandbox/` (sandbox layer)
//!    - `src/process/` (process subsystem)
//!    - `src/git/` (git subsystem — architectural justification: dedicated domain boundary)
//!    - `src/eval/` (test evaluation harness — not production orchestration)
//!    - `src/init/doctor.rs` (startup diagnostics — not execution authority)
//!    - `src/cli/doctor.rs` (diagnostic tool — not execution authority)
//!    - `src/workflow/genesis/environment.rs` (genesis environment probing — diagnostic only)
//!    - `src/verification/runners/` (verification runners — dedicated provider boundary)

use std::fs;
use std::path::Path;

fn collect_rs_files(dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Some(p) = path.to_str()
            {
                out.push(p.to_string());
            }
        }
    }
}

/// Layers where process execution (Command::new) is FORBIDDEN.
/// These are orchestration/coordination layers that must delegate to providers.
const FORBIDDEN_ORCHESTRATION_LAYERS: &[&str] = &[
    "src/controller/",
    "src/scheduler/",
    "src/interaction/",
    "src/state/",
    "src/state_machine/",
    "src/events/",
    "src/kernel/",
    "src/planning/",
    "src/dag/",
    "src/policy/",
    "src/recovery/",
    "src/budget/",
    "src/report/",
    "src/telemetry/",
    "src/context/",
    "src/pipeline/",
    "src/agent/",
    "src/model/",
    "src/skill/",
    "src/checkpoint/",
    "src/prompt/",
    "src/tui/",
    "src/config/",
    "src/ids/",
    "src/persistence/",
    "src/runtime.rs",
];

const PROCESS_SPAWN_PATTERNS: &[&str] = &[
    "process::Command",
    "std::process::Command",
    "tokio::process::Command",
];

#[test]
fn test_no_direct_process_execution_in_orchestration_layers() {
    let mut violations = Vec::new();

    for layer in FORBIDDEN_ORCHESTRATION_LAYERS {
        let layer_path = Path::new(layer);
        if !layer_path.exists() {
            continue;
        }

        let mut files = Vec::new();
        if layer_path.is_dir() {
            collect_rs_files(layer_path, &mut files);
        } else if layer_path.is_file() {
            files.push(layer.to_string());
        }

        for file in &files {
            let content = match fs::read_to_string(file) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (line_no, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                // Skip comments
                if trimmed.starts_with("//")
                    || trimmed.starts_with("/*")
                    || trimmed.starts_with('*')
                {
                    continue;
                }
                // Skip string literals that happen to contain pattern
                if trimmed.starts_with('"')
                    || trimmed.starts_with("r\"")
                    || trimmed.starts_with("r#\"")
                {
                    continue;
                }
                // Skip #[cfg(test)] blocks — test code is allowed to use Command for setup
                // (We check actual test modules separately if needed)

                for pattern in PROCESS_SPAWN_PATTERNS {
                    if trimmed.contains(pattern) {
                        violations.push(format!(
                            "  {}:{} — pattern '{}' found: {}",
                            file,
                            line_no + 1,
                            pattern,
                            trimmed
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Phase 17 Violation: Direct process execution found in orchestration layers.\n\
         Process spawning must flow through capability/sandbox/process subsystem boundaries.\n\
         Violations:\n{}",
        violations.join("\n")
    );
}

/// The runtime.rs file previously had ONE known pre-existing process execution site:
/// the pre-commit `cargo test` invocation in `commit_changes()`.
/// In Phase 20, this was routed through `LocalVerificationProvider`.
/// This test ensures ZERO direct process execution sites remain in `src/runtime.rs`.
#[test]
fn test_runtime_rs_process_execution_contained_to_known_exception() {
    let path = Path::new("src/runtime.rs");
    if !path.exists() {
        return;
    }

    let content = fs::read_to_string(path).unwrap_or_default();
    let mut sites = Vec::new();

    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }
        let matched = PROCESS_SPAWN_PATTERNS
            .iter()
            .any(|pattern| trimmed.contains(pattern));
        if matched {
            sites.push((line_no + 1, trimmed.to_string()));
        }
    }

    // Zero process execution sites are allowed in src/runtime.rs
    assert!(
        sites.is_empty(),
        "Phase 20 Violation: Direct process execution found in src/runtime.rs.\n\
         All process execution must use capability providers, sandbox providers, or process subsystem.\n\
         Found {} sites:\n{}",
        sites.len(),
        sites
            .iter()
            .map(|(ln, l)| format!("  src/runtime.rs:{} — {}", ln, l))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Verify that process execution in allowed subsystems is constrained
/// to the expected modules and not leaking into unexpected locations.
#[test]
fn test_process_execution_only_in_approved_boundaries() {
    // These are the ONLY directories/files where Command::new is architecturally justified
    let approved_boundaries = [
        "src/capability/providers/",
        "src/sandbox/",
        "src/process/",
        "src/platform/",
        "src/git/",
        "src/eval/",
        "src/init/doctor.rs",
        "src/cli/doctor.rs",
        "src/workflow/genesis/environment.rs",
        "src/verification/runners/",
        // Pre-existing: runtime.rs has a pre-commit `cargo test` invocation.
        // Architectural justification: this is a transitional pattern that should
        // eventually delegate to the verification subsystem (LocalVerificationProvider).
        // Tracked as known technical debt, not a Phase 15-18 regression.
        "src/runtime.rs",
    ];

    let mut all_files = Vec::new();
    collect_rs_files(Path::new("src"), &mut all_files);

    let mut unclassified_violations = Vec::new();

    for file in &all_files {
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let has_command = content.lines().any(|line| {
            let t = line.trim();
            !t.starts_with("//")
                && !t.starts_with("/*")
                && !t.starts_with('*')
                && !t.starts_with('"')
                && PROCESS_SPAWN_PATTERNS.iter().any(|p| t.contains(p))
        });

        if has_command {
            let is_approved = approved_boundaries
                .iter()
                .any(|boundary| file.starts_with(boundary));

            // Also allow test modules (cfg(test))
            let is_test_module = content.contains("#[cfg(test)]");

            if !is_approved && !is_test_module {
                for (line_no, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("//")
                        || trimmed.starts_with("/*")
                        || trimmed.starts_with('*')
                    {
                        continue;
                    }
                    for pattern in PROCESS_SPAWN_PATTERNS {
                        if trimmed.contains(pattern) {
                            unclassified_violations.push(format!(
                                "  {}:{} — {}",
                                file,
                                line_no + 1,
                                trimmed
                            ));
                        }
                    }
                }
            }
        }
    }

    assert!(
        unclassified_violations.is_empty(),
        "Phase 17 Violation: Process execution found outside approved boundaries.\n\
         Approved: {:?}\n\
         Unclassified occurrences:\n{}",
        approved_boundaries,
        unclassified_violations.join("\n")
    );
}
