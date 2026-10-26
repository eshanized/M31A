//! Phase 19 — Live Test Partition Guard
//!
//! Enforces that:
//! 1. ALL tests referencing live NVIDIA/NIM providers are properly #[ignore]d.
//! 2. No test in the deterministic CI suite silently skips on missing credentials
//!    (false-green pattern detection).
//! 3. Live tests that ARE #[ignore]d hard-fail when prerequisites are missing,
//!    rather than silently returning success.
//! 4. The test taxonomy is discoverable.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

#[path = "support/live_inventory.rs"]
mod live_inventory;
use live_inventory::{CANONICAL_LIVE_TEST_INVENTORY, LiveTestCategory};

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

    // Known exception: agent_runtime.rs::test_production_dispatcher_fails_closed_without_provider
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

// ============================================================================
// Phase 19 / Issue #61: Deep Anti-Bypass Architecture Guards
// ============================================================================

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ParsedTestFunction {
    file: String,
    name: String,
    line: usize,
    is_ignored: bool,
    ignore_reason: Option<String>,
    body: String,
}

fn extract_fn_name(line: &str) -> String {
    let after_fn = if let Some(idx) = line.find("fn ") {
        &line[idx + 3..]
    } else {
        line
    };
    let trimmed = after_fn.trim_start();
    let name_end = trimmed
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(trimmed.len());
    trimmed[..name_end].to_string()
}

fn extract_ignore_reason(line: &str) -> Option<String> {
    if let Some(idx) = line.find("ignore") {
        let rest = &line[idx + 6..];
        if let Some(eq_idx) = rest.find('=') {
            let after_eq = rest[eq_idx + 1..].trim();
            if let Some(stripped) = after_eq.strip_prefix('"') {
                if let Some(end_idx) = stripped.find('"') {
                    return Some(stripped[..end_idx].to_string());
                }
            } else if let Some(stripped) = after_eq.strip_prefix("r#\"") {
                if let Some(end_idx) = stripped.find("\"#") {
                    return Some(stripped[..end_idx].to_string());
                }
            }
        }
    }
    None
}

fn count_braces_outside_strings(line: &str, in_str: &mut bool) -> (usize, usize) {
    let mut open = 0;
    let mut close = 0;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if *in_str {
            if c == '\\' {
                chars.next();
            } else if c == '"' {
                *in_str = false;
            }
        } else if c == '"' {
            *in_str = true;
        } else if c == '/' && chars.peek() == Some(&'/') {
            break;
        } else if c == '{' {
            open += 1;
        } else if c == '}' {
            close += 1;
        }
    }
    (open, close)
}

fn parse_test_functions_from_file(file_path: &str) -> Vec<ParsedTestFunction> {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut tests = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();

        if trimmed == "#[test]" || trimmed.starts_with("#[tokio::test") {
            let mut is_ignored = false;
            let mut ignore_reason = None;
            let mut fn_idx = None;

            for back in 1..=3 {
                if let Some(prev) = i.checked_sub(back).and_then(|idx| lines.get(idx)) {
                    let prev_trim = prev.trim();
                    if prev_trim.starts_with("#[ignore") {
                        is_ignored = true;
                        if let Some(r) = extract_ignore_reason(prev_trim) {
                            ignore_reason = Some(r);
                        }
                    }
                }
            }

            let limit = std::cmp::min(i + 15, lines.len());
            for (j, line) in lines.iter().enumerate().take(limit).skip(i + 1) {
                let forward_trim = line.trim();
                if forward_trim.starts_with("#[ignore") {
                    is_ignored = true;
                    if let Some(r) = extract_ignore_reason(forward_trim) {
                        ignore_reason = Some(r);
                    }
                } else if forward_trim.starts_with("pub fn ")
                    || forward_trim.starts_with("fn ")
                    || forward_trim.starts_with("pub async fn ")
                    || forward_trim.starts_with("async fn ")
                {
                    fn_idx = Some(j);
                    break;
                }
            }

            if let Some(fn_line_idx) = fn_idx {
                let fn_line = lines[fn_line_idx].trim();
                let fn_name = extract_fn_name(fn_line);

                let mut body = String::new();
                let mut brace_depth = 0;
                let mut started = false;
                let mut in_str = false;
                let mut end_line_idx = fn_line_idx;

                for (k, line) in lines.iter().enumerate().skip(fn_line_idx) {
                    body.push_str(line);
                    body.push('\n');

                    let (open, close) = count_braces_outside_strings(line, &mut in_str);
                    brace_depth += open as i32;
                    brace_depth -= close as i32;

                    if open > 0 {
                        started = true;
                    }

                    if started && brace_depth <= 0 {
                        end_line_idx = k;
                        break;
                    }
                }

                tests.push(ParsedTestFunction {
                    file: file_path.to_string(),
                    name: fn_name,
                    line: fn_line_idx + 1,
                    is_ignored,
                    ignore_reason,
                    body,
                });

                i = end_line_idx + 1;
                continue;
            }
        }
        i += 1;
    }

    tests
}

/// Strip comments (line and block) and string literals from Rust source text,
/// leaving only executable code tokens and whitespace.
fn strip_comments_and_strings(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_str = false;
    let mut in_raw_str = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(c) = chars.next() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                out.push('\n');
            } else {
                out.push(' ');
            }
        } else if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
                out.push(' ');
                out.push(' ');
            } else if c == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
        } else if in_raw_str {
            if c == '"' && chars.peek() == Some(&'#') {
                chars.next();
                in_raw_str = false;
                out.push(' ');
                out.push(' ');
            } else if c == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
        } else if in_str {
            if c == '\\' {
                chars.next();
                out.push(' ');
                out.push(' ');
            } else if c == '"' {
                in_str = false;
                out.push(' ');
            } else if c == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
        } else if c == 'r' && chars.peek() == Some(&'#') {
            chars.next();
            if chars.peek() == Some(&'"') {
                chars.next();
                in_raw_str = true;
                out.push(' ');
                out.push(' ');
                out.push(' ');
            } else {
                out.push('r');
                out.push('#');
            }
        } else if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            in_line_comment = true;
            out.push(' ');
            out.push(' ');
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            out.push(' ');
            out.push(' ');
        } else if c == '"' {
            in_str = true;
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Detect tautologies, vacuous assertions, and false comparisons in executable code.
fn detect_tautologies_in_code(executable: &str) -> Vec<String> {
    let mut violations = Vec::new();

    // 1. assert!(true) or assert!(true, ...) or assert!(true || ...)
    if executable.contains("assert!(true)")
        || executable.contains("assert!(true,")
        || executable.contains("assert!(true ||")
        || executable.contains("assert !(true)")
        || executable.contains("assert !(true,")
        || executable.contains("assert !(true ||")
    {
        violations.push("Vacuous assertion detected: assert!(true...) tautology".to_string());
    }

    let lines: Vec<&str> = executable.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // 2. assert!(condition || !condition) or assert!(!condition || condition)
        if trimmed.starts_with("assert!") || trimmed.starts_with("assert !") {
            let inner = if let Some(part) = trimmed.split("assert!").nth(1) {
                part
            } else {
                trimmed.split("assert !").nth(1).unwrap_or("")
            };
            let inner = inner.trim().strip_prefix('(').unwrap_or(inner);
            let inner = inner.split(';').next().unwrap_or(inner);
            if inner.contains("||") {
                let parts: Vec<&str> = inner.split("||").map(|p| p.trim()).collect();
                if parts.len() >= 2 {
                    let a = parts[0].trim_matches(|c| c == '(' || c == ')');
                    let b = parts[1]
                        .trim_matches(|c| c == '(' || c == ')')
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim();
                    if (b.starts_with('!') && b[1..].trim() == a)
                        || (a.starts_with('!') && a[1..].trim() == b)
                        || a == "true"
                        || b == "true"
                    {
                        violations.push(format!(
                            "Line {}: Tautological assertion detected: '{}'",
                            idx + 1,
                            trimmed
                        ));
                    }
                }
            }
        }

        // 3. assert_eq!(x, x) self-comparisons
        if trimmed.starts_with("assert_eq!") || trimmed.starts_with("assert_eq !") {
            let inner = if let Some(part) = trimmed.split("assert_eq!").nth(1) {
                part
            } else {
                trimmed.split("assert_eq !").nth(1).unwrap_or("")
            };
            let inner = inner.trim().strip_prefix('(').unwrap_or(inner);
            let args: Vec<&str> = inner.split(',').collect();
            if args.len() >= 2 {
                let left = args[0].trim();
                let right = args[1]
                    .trim()
                    .trim_matches(|c| c == ')' || c == ';' || c == ' ' || c == '\r' || c == '\n');
                if left == right && !left.is_empty() {
                    violations.push(format!(
                        "Line {}: Tautological self-comparison detected: 'assert_eq!({left}, {right})'",
                        idx + 1
                    ));
                }
            }
        }

        // 4. Vacuous enum pattern match accepting all outcomes: Enforced | Unsupported | Failed
        if trimmed.contains("Enforced")
            && trimmed.contains("Unsupported")
            && trimmed.contains("Failed")
            && trimmed.contains('|')
        {
            violations.push(format!(
                "Line {}: Vacuous enum match pattern accepting all variants: '{}'",
                idx + 1,
                trimmed
            ));
        }
    }

    violations
}

/// Extract the body of any function by name from raw file content.
fn find_function_body_in_file(content: &str, target_name: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if (trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn ")
            || trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub async fn "))
            && extract_fn_name(trimmed) == target_name
        {
            let mut body = String::new();
            let mut brace_depth = 0;
            let mut started = false;
            let mut in_str = false;
            for line in lines.iter().skip(idx) {
                body.push_str(line);
                body.push('\n');
                let (open, close) = count_braces_outside_strings(line, &mut in_str);
                brace_depth += open as i32;
                brace_depth -= close as i32;
                if open > 0 {
                    started = true;
                }
                if started && brace_depth <= 0 {
                    break;
                }
            }
            return Some(body);
        }
    }
    None
}

/// Detect false-green early-return anti-patterns and tautologies in a test function body.
fn detect_bypass_in_test_body(body: &str) -> Vec<String> {
    let mut violations = Vec::new();

    // 1. Detect tautological assertions on executable tokens
    let executable = strip_comments_and_strings(body);
    violations.extend(detect_tautologies_in_code(&executable));

    // 2. String-level checks for skip indicators, suppression macros, and raw credential skips
    if (body.contains("LIVE SKIPPED") || body.contains("[SKIPPED]")) && body.contains("return") {
        violations.push("Silent skip notification followed by return".to_string());
    }

    if body.contains("skip_if_no_credentials")
        || body.contains("suppress_setup_failure")
        || body.contains("ignore_missing_credentials")
    {
        violations.push("Silent skip/suppression macro or helper detected".to_string());
    }

    let raw_lines: Vec<&str> = body.lines().collect();
    for (i, line) in raw_lines.iter().enumerate() {
        let trimmed = line.trim();
        if (trimmed.contains("NVIDIA_API_KEY") || trimmed.contains("API_KEY_NVIDIA"))
            && (trimmed.contains("is_err") || trimmed.contains("is_none") || trimmed.contains("is_empty"))
        {
            for j in 0..=5 {
                if let Some(next) = raw_lines.get(i + j) {
                    let next_trim = next.trim();
                    if next_trim == "return;" || next_trim == "return Ok(());" || next_trim == "return ();" || next_trim.starts_with("return ") {
                        violations.push(format!(
                            "Line {}: Credential absence early return without fail-closed panic: '{}'",
                            i + 1,
                            next_trim
                        ));
                        break;
                    }
                }
            }
        }
    }

    let lines: Vec<&str> = executable.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // 3. ensure_harness / RealModelHarness::ensure with early return
        if trimmed.contains("ensure_harness()") || trimmed.contains("RealModelHarness::ensure()") {
            for j in 0..=5 {
                if let Some(next) = lines.get(i + j) {
                    let next_trim = next.trim();
                    if next_trim.contains("return") {
                        violations.push(format!(
                            "Line {}: ensure_harness/RealModelHarness::ensure early return bypass: '{}'",
                            i + 1,
                            next_trim
                        ));
                        break;
                    }
                }
            }
        }

        // 4. SafeEnvironmentStatus::probe with early return
        if trimmed.contains("SafeEnvironmentStatus::probe()") {
            for j in 0..=5 {
                if let Some(next) = lines.get(i + j) {
                    let next_trim = next.trim();
                    if next_trim.contains("return") && !next_trim.contains("panic!") {
                        violations.push(format!(
                            "Line {}: SafeEnvironmentStatus early return bypass: '{}'",
                            i + 1,
                            next_trim
                        ));
                        break;
                    }
                }
            }
        }

        // 5. provider_configured early return
        if trimmed.contains("provider_configured")
            && (trimmed.contains("if !")
                || trimmed.contains("if not_")
                || trimmed.contains("if !is_"))
        {
            for j in 0..=5 {
                if let Some(next) = lines.get(i + j) {
                    let next_trim = next.trim();
                    if next_trim == "return;" || next_trim == "return Ok(());" {
                        violations.push(format!(
                            "Line {}: provider_configured early return bypass: '{}'",
                            i + 1,
                            next_trim
                        ));
                        break;
                    }
                }
            }
        }
    }

    violations
}

/// Enforce that all registered live tests in `CANONICAL_LIVE_TEST_INVENTORY` exist in source
/// code and are correctly partitioned with `#[ignore = "..."]` (unless offline deterministic).
#[test]
fn test_canonical_live_inventory_completeness() {
    let mut missing = Vec::new();
    let mut misconfigured = Vec::new();

    for entry in CANONICAL_LIVE_TEST_INVENTORY {
        let file_path = format!("tests/{}.rs", entry.test_binary);
        if !Path::new(&file_path).exists() {
            missing.push(format!(
                "Test binary '{}' ({}) does not exist on disk",
                entry.test_binary, file_path
            ));
            continue;
        }

        let parsed_tests = parse_test_functions_from_file(&file_path);
        let found = parsed_tests.iter().find(|t| t.name == entry.test_name);

        match found {
            None => {
                missing.push(format!(
                    "Function '{}' not found in '{}'",
                    entry.test_name, file_path
                ));
            }
            Some(test_fn) => {
                if entry.category == LiveTestCategory::OfflineDeterministic {
                    if test_fn.is_ignored {
                        misconfigured.push(format!(
                            "{}:{} — '{}' is categorized as OfflineDeterministic but is marked #[ignore]",
                            file_path, test_fn.line, entry.test_name
                        ));
                    }
                } else {
                    if !test_fn.is_ignored {
                        misconfigured.push(format!(
                            "{}:{} — '{}' is a live test ({:?}) but is NOT marked #[ignore]",
                            file_path, test_fn.line, entry.test_name, entry.category
                        ));
                    }
                    if test_fn.ignore_reason.as_deref().unwrap_or("").is_empty() {
                        misconfigured.push(format!(
                            "{}:{} — '{}' is marked #[ignore] but lacks an explanatory reason string",
                            file_path, test_fn.line, entry.test_name
                        ));
                    }
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Canonical live test inventory contains missing tests:\n{}",
        missing.join("\n")
    );

    assert!(
        misconfigured.is_empty(),
        "Canonical live test inventory contains misconfigured partitioning:\n{}",
        misconfigured.join("\n")
    );
}

/// Verify that EVERY live test defined in `tests/` is registered in `CANONICAL_LIVE_TEST_INVENTORY`.
/// This prevents new live tests from being introduced without registering in the canonical gate.
#[test]
fn test_all_live_tests_are_registered_in_canonical_inventory() {
    let mut test_files = Vec::new();
    collect_test_files(Path::new("tests"), &mut test_files);

    let mut unregistered = Vec::new();

    for file in &test_files {
        if file.contains("architecture_live_test_partition")
            || file.contains("tests/support")
            || file.contains("prompt_verification_and_diagnostics")
            || file.contains("tui_3_live_cockpit")
            || file.contains("tui_ux_remediation_suite")
            || file.contains("tui_first_frame_regression")
            || file.contains("tui_wiring_integrity_remediation")
            || file.contains("user_defined_commands")
            || file.contains("wiring_remediation_regression")
            || file.contains("security_p0_credential_boundary")
            || file.contains("remediation_runtime_convergence")
        {
            continue;
        }

        let parsed_tests = parse_test_functions_from_file(file);
        let binary_name = Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        for test_fn in parsed_tests {
            let name = &test_fn.name;
            let is_live_candidate = name.starts_with("live_")
                || name.starts_with("test_live_")
                || name.starts_with("test_real_model_")
                || name.starts_with("golden_real_model")
                || name.starts_with("golden_interactive")
                || name == "test_condition_n_live_provider_execution"
                || (binary_name == "model_decision_surfaces_live_nvidia"
                    && name.starts_with("real_"));

            if is_live_candidate {
                let in_inventory = CANONICAL_LIVE_TEST_INVENTORY
                    .iter()
                    .any(|e| e.test_binary == binary_name && e.test_name == name);

                if !in_inventory {
                    unregistered.push(format!(
                        "  {}:{} — live test function '{}' is not registered in CANONICAL_LIVE_TEST_INVENTORY",
                        file, test_fn.line, name
                    ));
                }
            }
        }
    }

    assert!(
        unregistered.is_empty(),
        "Live tests found that are NOT registered in CANONICAL_LIVE_TEST_INVENTORY:\n\
         Every live test must be registered in tests/support/live_inventory.rs to guarantee gating.\n\
         Unregistered:\n{}",
        unregistered.join("\n")
    );
}

/// the shell runner must execute every runnable entry in the canonical inventory,
/// with the same feature selection. release rehearsals use the release-candidate
/// evidence script and are not inference-runner entries.
#[test]
fn test_canonical_live_runner_matches_inventory() {
    let script = fs::read_to_string("scripts/run_live_tests.sh")
        .expect("canonical live-test runner must be present");
    let start = script
        .find("SUITE_CORE=(")
        .expect("runner must define canonical suites");
    let end = script[start..]
        .find("# ── Selection Resolution")
        .map(|offset| start + offset)
        .expect("runner suite definitions must end before selection resolution");
    let suite_source = &script[start..end];

    let mut parsed_runner_entries = Vec::new();
    for line in suite_source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('"') || !trimmed.ends_with('"') {
            continue;
        }
        let entry = &trimmed[1..trimmed.len() - 1];
        let parts: Vec<_> = entry.splitn(3, ':').collect();
        if parts.len() == 3 {
            parsed_runner_entries.push((
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            ));
        }
    }
    let actual: BTreeSet<_> = parsed_runner_entries.iter().cloned().collect();
    assert_eq!(
        parsed_runner_entries.len(),
        actual.len(),
        "canonical live runner must not execute the same test more than once"
    );

    let expected: BTreeSet<_> = CANONICAL_LIVE_TEST_INVENTORY
        .iter()
        .filter(|entry| entry.category != LiveTestCategory::ReleaseRehearsal)
        .map(|entry| {
            (
                entry.test_binary.to_string(),
                entry.required_feature.unwrap_or("none").to_string(),
                entry.test_name.to_string(),
            )
        })
        .collect();

    let missing: Vec<_> = expected.difference(&actual).cloned().collect();
    let unregistered: Vec<_> = actual.difference(&expected).cloned().collect();
    assert!(
        missing.is_empty() && unregistered.is_empty(),
        "Issue #61: live runner and canonical inventory have drifted.\nMissing from runner: {missing:#?}\nNot registered: {unregistered:#?}"
    );

    assert!(
        script.contains("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;"),
        "runner must reject an exit-zero cargo invocation that executed zero or multiple tests"
    );
    assert!(
        script.contains("M31A_LIVE_EVIDENCE_FILE")
            && script.contains("inference_response_count")
            && script.contains("request_attempt_count")
            && script.contains("ProviderVerification")
            && script.contains("LiveCancellationProbe")
            && script.contains("Unsupported live-test category"),
        "runner must enforce provider evidence and fail closed on unknown inventory categories"
    );

    // 1. CI Workflows Policy:
    // Ordinary GitHub CI (release-gates.yml and development.yml) must NOT define live-provider jobs (live-probes, live-acceptance)
    let release_gates_wf = fs::read_to_string(".github/workflows/release-gates.yml")
        .expect("release-gates workflow must be present");
    let dev_wf = fs::read_to_string(".github/workflows/development.yml")
        .expect("development workflow must be present");

    for (name, content) in [
        ("release-gates.yml", &release_gates_wf),
        ("development.yml", &dev_wf),
    ] {
        assert!(
            !content.contains("live-probes:") && !content.contains("live-acceptance:"),
            "{name} must not define live-provider jobs (live acceptance is excluded from GitHub CI)"
        );
        assert!(
            !content.contains("NVIDIA_API_KEY")
                && !content.contains("ANTHROPIC_API_KEY")
                && !content.contains("OPENAI_API_KEY")
                && !content.contains("GEMINI_API_KEY"),
            "{name} must not inject live-provider API keys into CI build/test jobs"
        );
    }

    // 2. Local release check script maintains local live acceptance requirements
    let release_check =
        fs::read_to_string("scripts/release-check.sh").expect("local release gate must be present");
    assert!(
        release_check.contains("make test-live-all"),
        "local production release gate must require the full live-provider acceptance suite"
    );

    // 3. Release workflow policy (.github/workflows/release.yml):
    let release_workflow = fs::read_to_string(".github/workflows/release.yml")
        .expect("production release workflow must be present");

    // Must NOT define live-acceptance job
    assert!(
        !release_workflow.contains("live-acceptance:"),
        "release.yml must not define a live-acceptance CI job"
    );

    // Release workflow must use version-tag pushes as its automatic trigger
    assert!(
        release_workflow.contains("tags:") && release_workflow.contains("\"v*\""),
        "release.yml must trigger automatically on version-tag pushes (v*)"
    );

    // Release workflow must validate tag and version consistency before building
    assert!(
        release_workflow.contains("validate-release-tag:")
            && release_workflow.contains("Cargo.toml")
            && release_workflow.contains("RELEASE_TAG"),
        "release.yml must validate that release tag matches canonical Cargo.toml package version"
    );

    // Release generation must remain build-focused: builds must not depend on deterministic-tests, fmt, or clippy
    assert!(
        !release_workflow.contains("needs: [deterministic-tests")
            && !release_workflow.contains("needs: [clippy")
            && !release_workflow.contains("needs: [fmt"),
        "release.yml builds must be build-focused and must not depend on test, fmt, or clippy jobs"
    );

    // Release workflow must not acquire a dependency on the ordinary deterministic release-gates workflow
    assert!(
        !release_workflow.contains("workflow_run:") || !release_workflow.contains("release-gates"),
        "release.yml must not depend on release-gates workflow run"
    );

    // Release workflow must validate artifact outputs before publishing
    assert!(
        release_workflow.contains("validate_release_manifests.py")
            || (release_workflow.contains("checksum") && release_workflow.contains("validate")),
        "release.yml must validate release artifact contents and manifests before publishing"
    );

    // Every real NVIDIA HTTP request path must emit dispatch and successful-response
    // evidence, and every chat-completion path must additionally prove inference.
    let provider = fs::read_to_string("src/model/provider/nvidia.rs")
        .expect("canonical NVIDIA provider must be present");
    let outbound_calls = provider.matches(".send()").count();
    let request_attempt_events = provider
        .matches(r#"record_live_provider_event("request_attempt")"#)
        .count();
    let request_events = provider
        .matches(r#"record_live_provider_event("request")"#)
        .count();
    let response_events = provider
        .matches(r#"record_live_provider_event("response")"#)
        .count();
    let inference_requests = provider
        .matches(r#"record_live_provider_event("inference_request")"#)
        .count();
    let inference_responses = provider
        .matches(r#"record_live_provider_event("inference_response")"#)
        .count();
    assert_eq!(
        outbound_calls, request_attempt_events,
        "every NVIDIA HTTP send path must record a dispatch attempt"
    );
    assert_eq!(
        outbound_calls, request_events,
        "every completed HTTP exchange must expose its response receipt"
    );
    assert_eq!(
        outbound_calls, response_events,
        "every NVIDIA HTTP path must expose its successful-response branch"
    );
    assert_eq!(
        inference_requests, inference_responses,
        "every chat-completion evidence path must expose completed inference"
    );
    assert!(
        inference_requests >= 1,
        "the provider must instrument real chat-completion requests"
    );
    assert!(
        provider.contains("#[cfg(debug_assertions)]\nfn record_live_provider_event(event: &str)")
            && provider.contains(
                "#[cfg(not(debug_assertions))]\nfn record_live_provider_event(_event: &str) {}"
            ),
        "provider evidence file writes must be disabled in optimized release builds"
    );

    let rc_evidence = fs::read_to_string("scripts/rc-evidence.sh")
        .expect("release-candidate evidence script must be present");
    assert!(
        rc_evidence.contains("release_candidate_validation_suite")
            && rc_evidence.contains("M31A_RC_DIR")
            && rc_evidence.contains("cargo test --test"),
        "the excluded release-rehearsal inventory entry must remain wired to release evidence"
    );

    let fixture_status = Command::new("bash")
        .arg("scripts/test_run_live_tests.sh")
        .status()
        .expect("live-runner false-green regression fixtures must execute");
    assert!(
        fixture_status.success(),
        "live-runner regression fixtures failed; a green result must require an executed test and real provider evidence"
    );
}

/// Verify that NO registered live test has a silent early-return or false-green skip.
#[test]
fn test_no_silent_early_returns_in_live_tests() {
    let mut violations = Vec::new();

    for entry in CANONICAL_LIVE_TEST_INVENTORY {
        if entry.category == LiveTestCategory::OfflineDeterministic {
            continue;
        }

        let file_path = format!("tests/{}.rs", entry.test_binary);
        let parsed_tests = parse_test_functions_from_file(&file_path);

        if let Some(test_fn) = parsed_tests.iter().find(|t| t.name == entry.test_name) {
            let bypasses = detect_bypass_in_test_body(&test_fn.body);
            for b in bypasses {
                violations.push(format!(
                    "  {}:{} ({}) — {}",
                    file_path, test_fn.line, entry.test_name, b
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Issue #61 Anti-Bypass Violation: Silent early-return patterns detected in live tests.\n\
         Live tests must fail closed when prerequisites are missing, never exit cleanly.\n\
         Violations:\n{}",
        violations.join("\n")
    );
}

/// Verify that all PositiveInference live tests assert model execution evidence or outcomes
/// using executable code analysis that rejects tautologies, strings, and comments.
#[test]
fn test_positive_live_tests_assert_usage_or_evidence() {
    let mut unasserted = Vec::new();

    for entry in CANONICAL_LIVE_TEST_INVENTORY {
        if entry.category != LiveTestCategory::PositiveInference || entry.min_requests == 0 {
            continue;
        }

        let file_path = format!("tests/{}.rs", entry.test_binary);
        let file_content = fs::read_to_string(&file_path).unwrap_or_default();
        let parsed_tests = parse_test_functions_from_file(&file_path);

        if let Some(test_fn) = parsed_tests.iter().find(|t| t.name == entry.test_name) {
            let mut full_body = test_fn.body.clone();
            // If the synchronous test function delegates to an `_inner()` helper, include it:
            let inner_name = format!("{}_inner", entry.test_name);
            if let Some(inner_body) = find_function_body_in_file(&file_content, &inner_name) {
                full_body.push_str(&inner_body);
            }

            let executable = strip_comments_and_strings(&full_body);
            let tautologies = detect_tautologies_in_code(&executable);
            let has_meaningful_assertion = (executable.contains("assert!")
                || executable.contains("assert_eq!")
                || executable.contains("assert_ne!")
                || executable.contains("assert_invocations")
                || executable.contains("execution_evidence")
                || executable.contains("expect_output"))
                && tautologies.is_empty();

            if !has_meaningful_assertion {
                unasserted.push(format!(
                    "  {}:{} ({}) — positive inference test lacks meaningful assertions of model outcome or invocation (tautologies: {:?})",
                    file_path, test_fn.line, entry.test_name, tautologies
                ));
            }
        }
    }

    assert!(
        unasserted.is_empty(),
        "Issue #61 Violation: Positive inference live tests without meaningful assertions detected:\n{}",
        unasserted.join("\n")
    );
}

/// Regression fixtures verifying that the architecture guard correctly flags bypasses,
/// tautologies, vacuous assertions, comment/string masking, and cleanly accepts valid fail-closed patterns.
#[test]
fn test_architecture_guard_detects_bypass_patterns_fixture() {
    // 1. Silent skip notification followed by return
    let bad_snippet_1 = r#"
        async fn test_sample() {
            if !provider_configured {
                eprintln!("LIVE SKIPPED");
                return;
            }
        }
    "#;
    let v1 = detect_bypass_in_test_body(bad_snippet_1);
    assert!(!v1.is_empty(), "Must detect LIVE SKIPPED return pattern");

    // 2. ensure_harness match with early return
    let bad_snippet_2 = r#"
        async fn test_sample() {
            let harness = match ensure_harness() {
                Ok(h) => h,
                Err(_) => return,
            };
        }
    "#;
    let v2 = detect_bypass_in_test_body(bad_snippet_2);
    assert!(
        !v2.is_empty(),
        "Must detect ensure_harness Err(_) => return"
    );

    // 3. SafeEnvironmentStatus probe with early return
    let bad_snippet_3 = r#"
        async fn test_sample() {
            let status = SafeEnvironmentStatus::probe();
            if !status.is_safe() {
                return;
            }
        }
    "#;
    let v3 = detect_bypass_in_test_body(bad_snippet_3);
    assert!(!v3.is_empty(), "Must detect SafeEnvironmentStatus return");

    // 4. Raw env var check followed by return Ok(())
    let bad_snippet_4 = r#"
        async fn test_sample() {
            if std::env::var("NVIDIA_API_KEY").is_err() {
                return Ok(());
            }
        }
    "#;
    let v4 = detect_bypass_in_test_body(bad_snippet_4);
    assert!(!v4.is_empty(), "Must detect NVIDIA_API_KEY is_err return");

    // 5. Tautology: assert!(true)
    let bad_tautology_1 = r#"
        async fn test_sample() {
            assert!(true);
        }
    "#;
    let v5 = detect_bypass_in_test_body(bad_tautology_1);
    assert!(!v5.is_empty(), "Must detect assert!(true) tautology");

    // 6. Tautology: assert_eq!(val, val)
    let bad_tautology_2 = r#"
        async fn test_sample() {
            let x = 42;
            assert_eq!(x, x);
        }
    "#;
    let v6 = detect_bypass_in_test_body(bad_tautology_2);
    assert!(!v6.is_empty(), "Must detect assert_eq!(x, x) tautology");

    // 7. Tautology: assert!(cond || !cond)
    let bad_tautology_3 = r#"
        async fn test_sample() {
            let diff = "something";
            assert!(diff.is_empty() || !diff.is_empty());
        }
    "#;
    let v7 = detect_bypass_in_test_body(bad_tautology_3);
    assert!(!v7.is_empty(), "Must detect assert!(cond || !cond) tautology");

    // 8. Tautology: assert!(true || suspicious_call())
    let bad_tautology_4 = r#"
        async fn test_sample() {
            assert!(true || false);
        }
    "#;
    let v8 = detect_bypass_in_test_body(bad_tautology_4);
    assert!(!v8.is_empty(), "Must detect assert!(true || ...) tautology");

    // 9. Assertion only in comment must NOT count as valid assertion
    let comment_only = r#"
        async fn test_sample() {
            // assert!(resp.is_ok());
            let _ = 123;
        }
    "#;
    let stripped_comment = strip_comments_and_strings(comment_only);
    assert!(
        !stripped_comment.contains("assert!"),
        "Stripped executable code must NOT contain assertion from comment"
    );

    // 10. Assertion only in string literal must NOT count as valid assertion
    let string_only = r#"
        async fn test_sample() {
            let fake_assert = "assert!(resp.is_ok())";
        }
    "#;
    let stripped_str = strip_comments_and_strings(string_only);
    assert!(
        !stripped_str.contains("assert!"),
        "Stripped executable code must NOT contain assertion from string literal"
    );

    // 11. Silent skip macro or helper suppressing failure
    let bad_macro = r#"
        async fn test_sample() {
            skip_if_no_credentials!();
        }
    "#;
    let v11 = detect_bypass_in_test_body(bad_macro);
    assert!(!v11.is_empty(), "Must detect skip_if_no_credentials macro");

    // 12. Compliant fail-closed positive test
    let compliant_snippet = r#"
        async fn test_sample() {
            let harness = RealModelHarness::require().await.expect("credentials required");
            let resp = harness.caller().call_model_with_options("hello", &Default::default()).await.unwrap();
            assert!(!resp.is_empty());
            harness.assert_invocations(1);
        }
    "#;
    let v12 = detect_bypass_in_test_body(compliant_snippet);
    assert!(
        v12.is_empty(),
        "Compliant fail-closed pattern must produce zero violations, got: {:?}",
        v12
    );

    // 13. Compliant failure-probe test
    let compliant_probe = r#"
        async fn test_probe() {
            let provider = NvidiaProvider::new_with_token("bad_token");
            let err = provider.call_model("test").await.unwrap_err();
            assert!(matches!(err, ModelError::AuthenticationFailed(_)));
        }
    "#;
    let v13 = detect_bypass_in_test_body(compliant_probe);
    assert!(
        v13.is_empty(),
        "Compliant failure probe must produce zero violations, got: {:?}",
        v13
    );

    // 14. Runner result/evidence gates must remain fail-closed.
    let runner = fs::read_to_string("scripts/run_live_tests.sh").expect("canonical runner exists");
    assert!(runner.contains("passing_summary_count"));
    assert!(runner.contains("inference_request_count"));
    assert!(runner.contains("inference_response_count"));
    assert!(runner.contains("validate_inventory_selection"));
}
