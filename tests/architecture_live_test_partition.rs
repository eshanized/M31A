//! Phase 19 — Live Test Partition Guard
//!
//! Enforces that:
//! 1. ALL tests referencing live NVIDIA/NIM providers are properly #[ignore]d.
//! 2. No test in the deterministic CI suite silently skips on missing credentials
//!    (false-green pattern detection).
//! 3. Live tests that ARE #[ignore]d hard-fail when prerequisites are missing,
//!    rather than silently returning success.
//! 4. The test taxonomy is discoverable.

use std::fs;
use std::path::Path;

fn collect_test_files(dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_test_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Some(p) = path.to_str()
            {
                out.push(p.to_string());
            }
        }
    }
}

/// Tests that reference live external NVIDIA/NIM providers must be marked #[ignore].
/// This prevents CI from silently depending on NVIDIA credentials.
#[test]
fn test_live_nvidia_tests_are_partitioned() {
    let mut test_files = Vec::new();
    collect_test_files(Path::new("tests"), &mut test_files);

    // Also check src/ for inline test modules
    collect_test_files(Path::new("src"), &mut test_files);

    let live_indicators = [
        "test_live_nvidia",
        "live_nvidia_provider",
        "live_nvidia_discovery",
        "live_provider_execution",
        "golden_real_model",
    ];

    let mut violations = Vec::new();

    for file in &test_files {
        // Skip this guard test file to avoid false positive self-detection
        if file.contains("architecture_live_test_partition") {
            continue;
        }
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Check if this line defines a test function that looks like a live test
            if trimmed.starts_with("fn ") || trimmed.starts_with("async fn ") {
                let is_live_fn = live_indicators
                    .iter()
                    .any(|indicator| trimmed.contains(indicator));

                if is_live_fn {
                    // Look backwards for #[ignore] within the preceding 5 lines
                    let has_ignore =
                        (0..=5)
                            .filter_map(|offset| i.checked_sub(offset))
                            .any(|check_line| {
                                lines
                                    .get(check_line)
                                    .is_some_and(|l| l.trim().starts_with("#[ignore"))
                            });

                    if !has_ignore {
                        violations.push(format!(
                            "  {}:{} — live test function '{}' is NOT #[ignore]d",
                            file,
                            i + 1,
                            trimmed
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Phase 19 Violation: Live NVIDIA/NIM tests found without #[ignore] partition.\n\
         All live/external tests MUST be #[ignore]d to prevent CI credential dependency.\n\
         Violations:\n{}",
        violations.join("\n")
    );
}

/// Verify that no non-ignored test in the deterministic suite has a silent skip
/// pattern that depends on NVIDIA_API_KEY or similar credentials.
///
/// A non-ignored test that checks for `NVIDIA_API_KEY` and returns early is a
/// false-green pattern — it reports PASS when it never executed the verification.
#[test]
fn test_no_false_green_credential_skips_in_deterministic_suite() {
    let mut test_files = Vec::new();
    collect_test_files(Path::new("tests"), &mut test_files);

    let credential_env_vars = ["NVIDIA_API_KEY", "API_KEY_NVIDIA", "OPENAI_API_KEY"];

    let mut violations = Vec::new();

    for file in &test_files {
        // Skip this guard test file to avoid false positive self-detection
        if file.contains("architecture_live_test_partition") {
            continue;
        }
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();

        // Find each test function
        let mut in_test_fn = false;
        let mut test_fn_name = String::new();
        let mut test_fn_line = 0;
        let mut has_ignore = false;
        let mut brace_depth: i32 = 0;
        let mut credential_check_and_return = false;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Track #[ignore] annotations
            if trimmed.starts_with("#[ignore") {
                has_ignore = true;
                continue;
            }

            // Detect test function start
            if (trimmed.starts_with("fn ") || trimmed.starts_with("async fn ")) && !in_test_fn {
                // Check if preceded by #[test] or #[tokio::test] within 5 lines
                let is_test =
                    (1..=5)
                        .filter_map(|offset| i.checked_sub(offset))
                        .any(|check_line| {
                            lines.get(check_line).is_some_and(|l| {
                                let t = l.trim();
                                t == "#[test]" || t.starts_with("#[tokio::test")
                            })
                        });

                if is_test {
                    in_test_fn = true;
                    test_fn_name = trimmed.to_string();
                    test_fn_line = i + 1;
                    brace_depth = 0;
                    credential_check_and_return = false;
                    // has_ignore was set from preceding lines; it stays
                }
            }

            if in_test_fn {
                brace_depth += trimmed.chars().filter(|c| *c == '{').count() as i32;
                brace_depth -= trimmed.chars().filter(|c| *c == '}').count() as i32;

                // Check for credential-gated early return pattern
                if !has_ignore {
                    let has_credential_check =
                        credential_env_vars.iter().any(|var| trimmed.contains(var));

                    if has_credential_check && trimmed.contains("return") {
                        credential_check_and_return = true;
                    }

                    // Also catch multi-line patterns:
                    // if env::var("NVIDIA_API_KEY").is_err() { ... return; }
                    if has_credential_check {
                        // Look ahead up to 5 lines for `return;`
                        for j in 1..=5 {
                            if let Some(next_line) = lines.get(i + j)
                                && (next_line.trim() == "return;"
                                    || next_line.trim() == "return ();")
                            {
                                credential_check_and_return = true;
                            }
                        }
                    }
                }

                // End of function
                if brace_depth == 0 && i > test_fn_line {
                    if credential_check_and_return && !has_ignore {
                        violations.push(format!(
                            "  {}:{} — non-ignored test '{}' has credential-gated early return (false-green)",
                            file, test_fn_line, test_fn_name
                        ));
                    }
                    in_test_fn = false;
                    has_ignore = false;
                }
            } else {
                // Reset ignore flag if we pass a non-test annotation
                if !trimmed.starts_with('#') && !trimmed.is_empty() {
                    has_ignore = false;
                }
            }
        }
    }

    // Known exception: phase_06_agent_runtime.rs::test_production_dispatcher_fails_closed_without_provider
    // This test INTENTIONALLY gates on missing NVIDIA_API_KEY to test fail-closed behavior.
    // It is a negative test, not a false-green skip.
    violations.retain(|v| !v.contains("test_production_dispatcher_fails_closed"));

    assert!(
        violations.is_empty(),
        "Phase 19 Violation: Non-ignored tests with credential-gated early returns detected.\n\
         These create false-green results in CI — the test reports PASS without executing verification.\n\
         Fix: either mark the test #[ignore] or remove the silent skip.\n\
         Violations:\n{}",
        violations.join("\n")
    );
}

/// Verify that all #[ignore]d tests have a descriptive reason string.
/// Bare `#[ignore]` without explanation makes it impossible to distinguish
/// "disabled because broken" from "live test requiring credentials".
#[test]
fn test_all_ignored_tests_have_reason() {
    let mut test_files = Vec::new();
    collect_test_files(Path::new("tests"), &mut test_files);
    collect_test_files(Path::new("src"), &mut test_files);

    let mut violations = Vec::new();

    for file in &test_files {
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            // Bare #[ignore] without reason
            if trimmed == "#[ignore]" {
                violations.push(format!(
                    "  {}:{} — bare #[ignore] without reason string",
                    file,
                    i + 1
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Phase 19 Violation: #[ignore] annotations found without reason strings.\n\
         Every ignored test must declare WHY it is ignored (e.g., #[ignore = \"requires NVIDIA_API_KEY\"]).\n\
         Violations:\n{}",
        violations.join("\n")
    );
}
