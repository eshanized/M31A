//! Phase 13 — Static Architectural Dependency Guard Tests (AD-007)
//!
//! These tests verify that the seam inversion has been maintained:
//! - Lower-level modules do NOT import from `controller` for interface definitions
//! - `kernel::seams` does NOT import from `controller`, `scheduler` impl, etc.
//! - All cross-layer contracts are defined in `kernel::seams`
//!
//! The guards work by scanning source files and asserting the absence of forbidden
//! import patterns. This catches regressions if someone re-introduces an upward dependency.

use std::fs;
use std::path::Path;

/// Collects all Rust source files under a given directory path.
fn collect_rust_files(dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    let root = Path::new(dir);
    if !root.exists() {
        return files;
    }
    collect_rs_recursive(root, &mut files);
    files
}

fn collect_rs_recursive(dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_recursive(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Some(p) = path.to_str()
            {
                out.push(p.to_string());
            }
        }
    }
}

fn read_file_content(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Checks that no file in `target_dir` contains any of the `forbidden_patterns`.
/// Returns a list of (file, pattern, line_number, line) for any violations.
fn find_violations(
    target_dir: &str,
    forbidden_patterns: &[&str],
) -> Vec<(String, String, usize, String)> {
    let mut violations = Vec::new();
    for file in collect_rust_files(target_dir) {
        let content = read_file_content(&file);
        for (line_no, line) in content.lines().enumerate() {
            for pattern in forbidden_patterns {
                if line.contains(pattern) {
                    violations.push((
                        file.clone(),
                        pattern.to_string(),
                        line_no + 1,
                        line.to_string(),
                    ));
                }
            }
        }
    }
    violations
}

/// AD-007 GUARD: kernel::seams must NOT import from controller, scheduler impl,
/// agent impl, verification impl, policy impl, TUI, CLI, persistence, context, planning, policy, state.
#[test]
fn ad007_kernel_seams_must_not_import_upward() {
    let forbidden = [
        "use crate::controller",
        "use crate::scheduler",
        "use crate::agent::",
        "use crate::tui",
        "use crate::cli",
        "use crate::persistence",
        "use crate::verification",
        "use crate::runtime",
        "use crate::workflow",
        "use crate::context",
        "use crate::planning",
        "use crate::policy",
        "use crate::state::",
        "use crate::dag",
        "use crate::pipeline",
        "use crate::capability",
        "use crate::process",
        "use crate::tools",
        "use crate::sandbox",
        "crate::context::",
        "crate::planning::",
        "crate::policy::",
        "crate::scheduler::",
        "crate::agent::",
        "crate::state::",
        "crate::persistence::",
    ];

    let violations = find_violations("src/kernel", &forbidden);

    if !violations.is_empty() {
        let details: Vec<String> = violations
            .iter()
            .map(|(f, p, ln, l)| format!("  {}:{} — pattern '{}' found in: {}", f, ln, p, l.trim()))
            .collect();
        panic!(
            "AD-007 VIOLATED: src/kernel imports upward dependencies:\n{}",
            details.join("\n")
        );
    }
}

/// AD-007 GUARD: scheduler must NOT import from controller for interface traits.
/// It may import controller types for orchestration (e.g., ControllerDependencies)
/// but should not import controller::seams traits (which no longer exist there).
#[test]
fn ad007_scheduler_must_not_import_controller_seams() {
    let forbidden = ["use crate::controller::seams", "controller::seams::"];

    let violations = find_violations("src/scheduler", &forbidden);

    if !violations.is_empty() {
        let details: Vec<String> = violations
            .iter()
            .map(|(f, p, ln, l)| format!("  {}:{} — '{}': {}", f, ln, p, l.trim()))
            .collect();
        panic!(
            "AD-007 VIOLATED: scheduler imports controller::seams (upward dependency):\n{}",
            details.join("\n")
        );
    }
}

/// AD-007 GUARD: agent module must NOT import controller::seams.
#[test]
fn ad007_agent_must_not_import_controller_seams() {
    let forbidden = ["use crate::controller::seams", "controller::seams::"];

    let violations = find_violations("src/agent", &forbidden);

    if !violations.is_empty() {
        let details: Vec<String> = violations
            .iter()
            .map(|(f, p, ln, l)| format!("  {}:{} — '{}': {}", f, ln, p, l.trim()))
            .collect();
        panic!(
            "AD-007 VIOLATED: agent imports controller::seams (upward dependency):\n{}",
            details.join("\n")
        );
    }
}

/// AD-007 GUARD: verification module must NOT import controller::seams.
#[test]
fn ad007_verification_must_not_import_controller_seams() {
    let forbidden = ["use crate::controller::seams", "controller::seams::"];

    let violations = find_violations("src/verification", &forbidden);

    if !violations.is_empty() {
        let details: Vec<String> = violations
            .iter()
            .map(|(f, p, ln, l)| format!("  {}:{} — '{}': {}", f, ln, p, l.trim()))
            .collect();
        panic!(
            "AD-007 VIOLATED: verification imports controller::seams (upward dependency):\n{}",
            details.join("\n")
        );
    }
}

/// AD-007 GUARD: policy module must NOT import controller::seams.
#[test]
fn ad007_policy_must_not_import_controller_seams() {
    let forbidden = ["use crate::controller::seams", "controller::seams::"];

    let violations = find_violations("src/policy", &forbidden);

    if !violations.is_empty() {
        let details: Vec<String> = violations
            .iter()
            .map(|(f, p, ln, l)| format!("  {}:{} — '{}': {}", f, ln, p, l.trim()))
            .collect();
        panic!(
            "AD-007 VIOLATED: policy imports controller::seams (upward dependency):\n{}",
            details.join("\n")
        );
    }
}

/// AD-007 GUARD: recovery module must NOT import controller::seams.
#[test]
fn ad007_recovery_must_not_import_controller_seams() {
    let forbidden = ["use crate::controller::seams", "controller::seams::"];

    let violations = find_violations("src/recovery", &forbidden);

    if !violations.is_empty() {
        let details: Vec<String> = violations
            .iter()
            .map(|(f, p, ln, l)| format!("  {}:{} — '{}': {}", f, ln, p, l.trim()))
            .collect();
        panic!(
            "AD-007 VIOLATED: recovery imports controller::seams (upward dependency):\n{}",
            details.join("\n")
        );
    }
}

/// AD-007 GUARD: planning module must NOT import controller::seams.
#[test]
fn ad007_planning_must_not_import_controller_seams() {
    let forbidden = ["use crate::controller::seams", "controller::seams::"];

    let violations = find_violations("src/planning", &forbidden);

    if !violations.is_empty() {
        let details: Vec<String> = violations
            .iter()
            .map(|(f, p, ln, l)| format!("  {}:{} — '{}': {}", f, ln, p, l.trim()))
            .collect();
        panic!(
            "AD-007 VIOLATED: planning imports controller::seams (upward dependency):\n{}",
            details.join("\n")
        );
    }
}

/// AD-007 GUARD: budget module must NOT import controller:: for budget kinds.
/// BudgetKind now lives in budget::kind — controller re-exports it, not the other way around.
#[test]
fn ad007_budget_must_not_import_controller_for_budget_kind() {
    let forbidden = ["use crate::controller::budget_tracker::BudgetKind"];

    let violations = find_violations("src/budget", &forbidden);

    if !violations.is_empty() {
        let details: Vec<String> = violations
            .iter()
            .map(|(f, p, ln, l)| format!("  {}:{} — '{}': {}", f, ln, p, l.trim()))
            .collect();
        panic!(
            "AD-007 VIOLATED: budget imports BudgetKind from controller (upward dependency):\n{}",
            details.join("\n")
        );
    }
}

/// AD-007 POSITIVE: kernel::seams must be publicly accessible.
/// This verifies the neutral layer is properly exposed.
#[test]
fn ad007_kernel_seams_directory_exists() {
    assert!(
        Path::new("src/kernel/seams").exists(),
        "AD-007: src/kernel/seams/ must exist as the neutral seam layer"
    );
    assert!(
        Path::new("src/kernel/seams/mod.rs").exists(),
        "AD-007: src/kernel/seams/mod.rs must exist"
    );

    // Verify all 8 seam contracts are present
    let expected = [
        "src/kernel/seams/context.rs",
        "src/kernel/seams/escalation.rs",
        "src/kernel/seams/execution.rs",
        "src/kernel/seams/planner.rs",
        "src/kernel/seams/policy.rs",
        "src/kernel/seams/recovery.rs",
        "src/kernel/seams/scheduler.rs",
        "src/kernel/seams/verifier.rs",
    ];
    for f in expected {
        assert!(
            Path::new(f).exists(),
            "AD-007: Expected seam file {f} missing"
        );
    }
}

/// AD-007 NEGATIVE: controller::seams/ directory must NOT exist.
/// After migration, the old location should be gone.
#[test]
fn ad007_controller_seams_directory_removed() {
    assert!(
        !Path::new("src/controller/seams").exists(),
        "AD-007: src/controller/seams/ should have been removed after seam migration. \
         All seam contracts must live in src/kernel/seams/"
    );
}

/// Verifies that kernel::seams compiles and exports the WorkScheduler trait.
/// This is a type-level guard that kernel::seams is properly importable.
#[test]
fn ad007_kernel_seams_exports_work_scheduler() {
    // This test compiles if and only if WorkScheduler is accessible from kernel::seams.
    // It's a compile-time structural verification.
    fn _check_trait_accessible() {
        fn _accepts_scheduler<T: m31a::kernel::seams::WorkScheduler>(_t: &T) {}
        let _ = std::marker::PhantomData::<dyn m31a::kernel::seams::WorkScheduler>;
    }
}

/// Verifies that kernel::seams exports PolicyGate trait.
#[test]
fn ad007_kernel_seams_exports_policy_gate() {
    fn _check_trait_accessible() {
        let _ = std::marker::PhantomData::<dyn m31a::kernel::seams::PolicyGate>;
    }
}

/// Verifies that kernel::seams exports VerificationEngine trait.
#[test]
fn ad007_kernel_seams_exports_verification_engine() {
    fn _check_trait_accessible() {
        let _ = std::marker::PhantomData::<dyn m31a::kernel::seams::VerificationEngine>;
    }
}

/// Verifies that kernel::seams exports WorkerDispatcher trait.
#[test]
fn ad007_kernel_seams_exports_worker_dispatcher() {
    fn _check_trait_accessible() {
        let _ = std::marker::PhantomData::<dyn m31a::kernel::seams::WorkerDispatcher>;
    }
}

/// Verifies that kernel::seams exports RecoveryEngine trait.
#[test]
fn ad007_kernel_seams_exports_recovery_engine() {
    fn _check_trait_accessible() {
        let _ = std::marker::PhantomData::<dyn m31a::kernel::seams::RecoveryEngine>;
    }
}
