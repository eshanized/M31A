//! Test Integrity Auditing and Mutation Testing Engine (`m31a-verify::integrity`).
//!
//! Audits test suites to detect fake tests, weak assertions, excessive mocking,
//! and tests that pass without validating the underlying requirement.
//!
//! Enforces:
//! 1. Weak Assertion Detection: Flags `assert!(true)`, `assert_eq!(1, 1)`, empty tests,
//!    or assertions that only check `is_ok()` without inspecting content.
//! 2. Mock Abuse Detection: Identifies tests that mock every dependency without executing real logic.
//! 3. Mutation Testing: Injects targeted mutations (e.g. flipping boolean conditions, replacing return values)
//!    and verifies that the test suite actually fails (mutant killed). Tests that pass on mutated code
//!    are flagged as fake or ineffective.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestFlawKind {
    TrivialAssertion,
    EmptyTestBody,
    ShallowAssertion,
    MockOnlyWithoutProductionCode,
    UnreachableAssertion,
    SurvivingMutant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFlaw {
    pub file_path: PathBuf,
    pub line_number: Option<usize>,
    pub test_name: String,
    pub kind: TestFlawKind,
    pub description: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityVerdict {
    Sound,
    FlawsDetected,
    Compromised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationStatus {
    Killed,
    Survived,
    TimedOut,
    NoCandidates,
    CompileFailed,
    InfrastructureFailure,
    RestorationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationResult {
    pub target_file: PathBuf,
    pub line_number: usize,
    pub original_code: String,
    pub mutated_code: String,
    pub killed: bool,
    #[serde(default)]
    pub status: Option<MutationStatus>,
    pub output_snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestIntegrityReport {
    pub total_tests_analyzed: usize,
    pub flaws: Vec<TestFlaw>,
    pub mutations: Vec<MutationResult>,
    pub mutation_kill_rate: f64,
    pub verdict: IntegrityVerdict,
    pub summary: String,
}

/// RAII guard ensuring that mutated source files are always restored to their original
/// contents upon drop, even in the event of panics, timeouts, or early returns.
pub struct MutationGuard {
    path: PathBuf,
    original_bytes: Vec<u8>,
    original_permissions: Option<std::fs::Permissions>,
    initial_modified: Option<std::time::SystemTime>,
    active: bool,
}

impl MutationGuard {
    pub fn new(path: PathBuf, original_content: String) -> Self {
        let original_bytes = original_content.into_bytes();
        let (original_permissions, initial_modified) = match fs::metadata(&path) {
            Ok(m) => (Some(m.permissions()), m.modified().ok()),
            Err(_) => (None, None),
        };
        Self {
            path,
            original_bytes,
            original_permissions,
            initial_modified,
            active: true,
        }
    }

    pub fn restore(&mut self) -> Result<()> {
        if self.active {
            if let (Some(_initial), Ok(_meta)) = (self.initial_modified, fs::metadata(&self.path)) {
                // Inspected metadata prior to restoration
            }
            fs::write(&self.path, &self.original_bytes)?;
            if let Some(perms) = &self.original_permissions {
                let _ = fs::set_permissions(&self.path, perms.clone());
            }
            self.active = false;
        }
        Ok(())
    }
}

impl Drop for MutationGuard {
    fn drop(&mut self) {
        if self.active {
            if let Err(e) = fs::write(&self.path, &self.original_bytes) {
                eprintln!(
                    "[MutationGuard ERROR] Failed to restore file {}: {e}",
                    self.path.display()
                );
            } else if let Some(perms) = &self.original_permissions {
                let _ = fs::set_permissions(&self.path, perms.clone());
            }
        }
    }
}

pub struct TestIntegrityAuditor;

impl TestIntegrityAuditor {
    /// Audits a test file for weak assertions, fake tests, and mock abuse.
    pub fn audit_test_file(path: &Path, content: &str) -> Vec<TestFlaw> {
        let mut flaws = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let trivial_assert_regex = Regex::new(
            r#"(?x)
            assert!\s*\(\s*true\s*\) |
            assert_eq!\s*\(\s*1\s*,\s*1\s*\) |
            assert_eq!\s*\(\s*0\s*,\s*0\s*\) |
            assert_eq!\s*\(\s*"a"\s*,\s*"a"\s*\) |
            expect\s*\(\s*true\s*\)\.to(Be|Equal)\s*\(\s*true\s*\) |
            self\.assertTrue\s*\(\s*True\s*\) |
            assert\s+True\b
            "#,
        )
        .unwrap();

        let shallow_assert_regex = Regex::new(
            r#"(?x)
            assert!\s*\(\s*[a-zA-Z0-9_.]+\.is_ok\(\)\s*\) |
            assert!\s*\(\s*![a-zA-Z0-9_.]+\.is_err\(\)\s*\) |
            expect\s*\(\s*[a-zA-Z0-9_.]+\s*\)\.toBeDefined\(\)
            "#,
        )
        .unwrap();

        let mut current_test_name: Option<String> = None;
        let mut current_test_body: Vec<(usize, &str)> = Vec::new();

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            // Detect test function start (Rust, Python, JS/TS)
            let is_test_start = trimmed.starts_with("#[test]")
                || trimmed.starts_with("def test_")
                || (trimmed.starts_with("test(") || trimmed.starts_with("it("));

            if is_test_start {
                if let Some(name) = current_test_name.take() {
                    Self::analyze_test_body(path, &name, &current_test_body, &mut flaws);
                    current_test_body.clear();
                }

                // Extract name
                let name = if trimmed.starts_with("#[test]") {
                    // Lookahead to next fn
                    lines
                        .iter()
                        .skip(idx + 1)
                        .take(3)
                        .find(|l| l.contains("fn "))
                        .and_then(|l| l.split("fn ").nth(1))
                        .and_then(|l| l.split('(').next())
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|| format!("test_at_line_{}", line_num))
                } else if trimmed.starts_with("def test_") {
                    trimmed
                        .strip_prefix("def ")
                        .and_then(|s| s.split('(').next())
                        .unwrap_or("test")
                        .to_string()
                } else {
                    format!("test_at_line_{}", line_num)
                };

                current_test_name = Some(name);
                continue;
            }

            if current_test_name.is_some() {
                current_test_body.push((line_num, line));

                // 1. Trivial Assertion check
                if trivial_assert_regex.is_match(trimmed) {
                    flaws.push(TestFlaw {
                        file_path: path.to_path_buf(),
                        line_number: Some(line_num),
                        test_name: current_test_name.clone().unwrap_or_default(),
                        kind: TestFlawKind::TrivialAssertion,
                        description: "Trivial assertion always evaluates to true without exercising production code.".to_string(),
                        snippet: Some(trimmed.to_string()),
                    });
                }

                // 2. Shallow assertion check (e.g., assert!(res.is_ok()) without checking value)
                if shallow_assert_regex.is_match(trimmed) {
                    flaws.push(TestFlaw {
                        file_path: path.to_path_buf(),
                        line_number: Some(line_num),
                        test_name: current_test_name.clone().unwrap_or_default(),
                        kind: TestFlawKind::ShallowAssertion,
                        description: "Shallow assertion only verifies is_ok()/toBeDefined() without checking expected output values or invariants.".to_string(),
                        snippet: Some(trimmed.to_string()),
                    });
                }
            }
        }

        if let Some(name) = current_test_name {
            Self::analyze_test_body(path, &name, &current_test_body, &mut flaws);
        }

        flaws
    }

    fn analyze_test_body(
        path: &Path,
        test_name: &str,
        body_lines: &[(usize, &str)],
        flaws: &mut Vec<TestFlaw>,
    ) {
        if body_lines.is_empty() {
            return;
        }

        let non_empty_lines: Vec<&(usize, &str)> = body_lines
            .iter()
            .filter(|(_, l)| {
                let t = l.trim();
                !t.is_empty()
                    && !t.starts_with("//")
                    && !t.starts_with('#')
                    && !t.starts_with("fn ")
                    && !t.starts_with("def ")
                    && t != "{"
                    && t != "}"
                    && !t.ends_with('{')
            })
            .collect();

        // 1. Check for empty test body
        if non_empty_lines.is_empty() {
            flaws.push(TestFlaw {
                file_path: path.to_path_buf(),
                line_number: body_lines.first().map(|(l, _)| *l),
                test_name: test_name.to_string(),
                kind: TestFlawKind::EmptyTestBody,
                description: format!(
                    "Test '{}' has an empty implementation body with no executable statements.",
                    test_name
                ),
                snippet: None,
            });
            return;
        }

        // 2. Check for unreachable assertions (e.g., assertion placed after unconditional return / panic)
        let mut encountered_early_exit = None;
        for &(line_num, line) in body_lines {
            let t = line.trim();
            if t.starts_with("//") {
                continue;
            }
            if encountered_early_exit.is_none()
                && (t == "return;"
                    || t.starts_with("return ")
                    || t.starts_with("panic!(")
                    || t.starts_with("process::exit("))
            {
                encountered_early_exit = Some((line_num, t.to_string()));
            } else if let Some((exit_line, ref exit_stmt)) = encountered_early_exit
                && (t.contains("assert") || t.contains("expect("))
            {
                flaws.push(TestFlaw {
                        file_path: path.to_path_buf(),
                        line_number: Some(line_num),
                        test_name: test_name.to_string(),
                        kind: TestFlawKind::UnreachableAssertion,
                        description: format!(
                            "Assertion on line {} is unreachable because of earlier exit ('{}') on line {}.",
                            line_num, exit_stmt, exit_line
                        ),
                        snippet: Some(t.to_string()),
                    });
            }
        }

        // 3. Check for mock abuse (MockOnlyWithoutProductionCode)
        let mock_indicators = [
            "mock",
            "Mock",
            "when(",
            "given(",
            "expect_then",
            "mocker",
            "sinon",
            "jest.fn",
        ];
        let has_mock_setup = body_lines
            .iter()
            .any(|(_, l)| mock_indicators.iter().any(|m| l.contains(m)));
        let has_real_production_call = body_lines.iter().any(|(_, l)| {
            let t = l.trim();
            !mock_indicators.iter().any(|m| t.contains(m))
                && (t.contains("::new(")
                    || t.contains(".execute(")
                    || t.contains(".process(")
                    || t.contains(".run(")
                    || t.contains(".handle("))
        });

        if has_mock_setup && !has_real_production_call {
            let only_mock_assertions = body_lines.iter().any(|(_, l)| {
                l.contains("expect_")
                    || l.contains("times(")
                    || l.contains("toHaveBeenCalled")
                    || l.contains("assert_called")
            });
            if only_mock_assertions {
                flaws.push(TestFlaw {
                    file_path: path.to_path_buf(),
                    line_number: body_lines.first().map(|(l, _)| *l),
                    test_name: test_name.to_string(),
                    kind: TestFlawKind::MockOnlyWithoutProductionCode,
                    description: format!(
                        "Test '{}' only asserts against mocked interactions without exercising production logic.",
                        test_name
                    ),
                    snippet: None,
                });
            }
        }

        // 4. Check for tests with zero assertions
        let has_assertions = body_lines.iter().any(|(_, l)| {
            let t = l.trim();
            t.contains("assert") || t.contains("expect") || t.contains("should")
        });

        if !has_assertions {
            flaws.push(TestFlaw {
                file_path: path.to_path_buf(),
                line_number: body_lines.first().map(|(l, _)| *l),
                test_name: test_name.to_string(),
                kind: TestFlawKind::TrivialAssertion,
                description: format!("Test '{}' contains executable statements but executes zero assertions or verifications.", test_name),
                snippet: None,
            });
        }
    }

    /// Helper to execute a mutation test command with bounded timeout, concurrent output draining, and process group reaping.
    fn run_mutation_command_with_timeout(
        parts: &[String],
        cwd: &Path,
        timeout: std::time::Duration,
    ) -> (bool, Option<MutationStatus>, String) {
        if parts.is_empty() {
            return (
                false,
                Some(MutationStatus::InfrastructureFailure),
                "Empty command".to_string(),
            );
        }

        let mut cmd = Command::new(&parts[0]);
        if parts.len() > 1 {
            cmd.args(&parts[1..]);
        }
        cmd.current_dir(cwd);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return (
                    false,
                    Some(MutationStatus::InfrastructureFailure),
                    format!("Failed to spawn test command: {}", e),
                );
            }
        };

        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Spawn reader threads to continuously drain stdout and stderr to EOF with bounded retention to prevent OS pipe deadlock
        const MAX_OUTPUT_CAPTURE: usize = 64 * 1024; // 64 KB
        fn drain_sync_reader<R: std::io::Read>(mut reader: R, max_retention: usize) -> Vec<u8> {
            let mut retained = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        if retained.len() < max_retention {
                            let available = max_retention - retained.len();
                            let to_copy = n.min(available);
                            retained.extend_from_slice(&chunk[..to_copy]);
                        }
                    }
                    Err(_) => break,
                }
            }
            retained
        }

        let stdout_handle = std::thread::spawn(move || {
            stdout
                .map(|r| drain_sync_reader(r, MAX_OUTPUT_CAPTURE))
                .unwrap_or_default()
        });

        let stderr_handle = std::thread::spawn(move || {
            stderr
                .map(|r| drain_sync_reader(r, MAX_OUTPUT_CAPTURE))
                .unwrap_or_default()
        });

        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(20);
        let mut timed_out = false;
        let mut exit_status = None;

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit_status = Some(status);
                    break;
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        timed_out = true;
                        break;
                    }
                    std::thread::sleep(poll_interval);
                }
                Err(e) => {
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return (
                        false,
                        Some(MutationStatus::InfrastructureFailure),
                        format!("Process wait error: {}", e),
                    );
                }
            }
        }

        if timed_out {
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(-(pid as libc::pid_t), libc::SIGTERM);
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
                unsafe {
                    libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
                }
            }
            let _ = child.kill();
            let _ = child.wait();

            let _ = stdout_handle.join();
            let _ = stderr_handle.join();

            return (
                true, // Timeout kills the mutant (an infinite loop or hang was induced)
                Some(MutationStatus::TimedOut),
                format!("Mutation caused test suite to time out after {:?}", timeout),
            );
        }

        let status = match exit_status {
            Some(s) => s,
            None => {
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return (
                    false,
                    Some(MutationStatus::InfrastructureFailure),
                    "Unknown process exit state".to_string(),
                );
            }
        };

        let stdout_buf = stdout_handle.join().unwrap_or_default();
        let stderr_buf = stderr_handle.join().unwrap_or_default();

        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&stdout_buf),
            String::from_utf8_lossy(&stderr_buf)
        );
        let snip = text.chars().take(400).collect::<String>();

        if status.success() {
            // Test passed -> mutant survived
            (false, Some(MutationStatus::Survived), snip)
        } else if text.contains("error[E")
            || text.contains("could not compile")
            || text.contains("SyntaxError")
            || text.contains("compilation error")
        {
            // Compiler rejected the mutant - does NOT count as killed by test suite
            (false, Some(MutationStatus::CompileFailed), snip)
        } else {
            // Test failed -> mutant killed
            (true, Some(MutationStatus::Killed), snip)
        }
    }

    /// Evaluates mutation resilience: mutates target file and executes test command to ensure tests fail.
    pub fn evaluate_mutation(
        repo_root: &Path,
        target_file: &Path,
        test_command: &str,
    ) -> Result<MutationResult> {
        let full_path = if target_file.is_absolute() {
            target_file.to_path_buf()
        } else {
            repo_root.join(target_file)
        };

        let original_content = fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = original_content.lines().collect();

        // Find a suitable line to mutate: e.g., == to !=, true to false, or early return
        let mut mutation_line = 0;
        let mut original_line = String::new();
        let mut mutated_line = String::new();

        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.is_empty() || trimmed.contains("test") {
                continue;
            }

            if trimmed.contains(" == ") {
                mutation_line = idx + 1;
                original_line = line.to_string();
                mutated_line = line.replace(" == ", " != ");
                break;
            } else if trimmed.contains("true") && !trimmed.contains("assert") {
                mutation_line = idx + 1;
                original_line = line.to_string();
                mutated_line = line.replacen("true", "false", 1);
                break;
            } else if trimmed.contains(" > ") {
                mutation_line = idx + 1;
                original_line = line.to_string();
                mutated_line = line.replace(" > ", " <= ");
                break;
            }
        }

        if mutation_line == 0 {
            return Ok(MutationResult {
                target_file: target_file.to_path_buf(),
                line_number: 0,
                original_code: "No mutable expression found".to_string(),
                mutated_code: String::new(),
                killed: false, // Cannot mutate, do not report as killed
                status: Some(MutationStatus::NoCandidates),
                output_snippet: "No mutation candidates".to_string(),
            });
        }

        // Apply mutation under RAII protection
        let mut guard = MutationGuard::new(full_path.clone(), original_content.clone());
        let mut new_lines = lines.clone();
        new_lines[mutation_line - 1] = &mutated_line;
        let mutated_content = new_lines.join("\n");
        fs::write(&full_path, &mutated_content)?;

        // Run test command with timeout
        let parts = shlex::split(test_command)
            .unwrap_or_else(|| vec!["cargo".to_string(), "test".to_string()]);
        let (killed, status, snippet) = Self::run_mutation_command_with_timeout(
            &parts,
            repo_root,
            std::time::Duration::from_secs(10),
        );

        // Restore original content immediately
        if let Err(e) = guard.restore() {
            eprintln!(
                "[Mutation] Restoration failure on {}: {e}",
                full_path.display()
            );
            return Ok(MutationResult {
                target_file: target_file.to_path_buf(),
                line_number: mutation_line,
                original_code: original_line,
                mutated_code: mutated_line,
                killed,
                status: Some(MutationStatus::RestorationFailed),
                output_snippet: format!("Restoration failed: {e}"),
            });
        }

        Ok(MutationResult {
            target_file: target_file.to_path_buf(),
            line_number: mutation_line,
            original_code: original_line,
            mutated_code: mutated_line,
            killed,
            status,
            output_snippet: snippet,
        })
    }

    /// Injects multiple diverse mutations into a target file and computes overall mutation kill rate.
    pub fn evaluate_multi_point_mutations(
        repo_root: &Path,
        target_file: &Path,
        test_command: &str,
        max_mutants: usize,
    ) -> Result<Vec<MutationResult>> {
        let full_path = if target_file.is_absolute() {
            target_file.to_path_buf()
        } else {
            repo_root.join(target_file)
        };

        let original_content = fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = original_content.lines().collect();

        let mut mutation_candidates = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.is_empty() || trimmed.contains("test") {
                continue;
            }

            if trimmed.contains(" == ") {
                mutation_candidates.push((idx + 1, line.to_string(), line.replace(" == ", " != ")));
            } else if trimmed.contains(" != ") {
                mutation_candidates.push((idx + 1, line.to_string(), line.replace(" != ", " == ")));
            } else if trimmed.contains("true") && !trimmed.contains("assert") {
                mutation_candidates.push((
                    idx + 1,
                    line.to_string(),
                    line.replacen("true", "false", 1),
                ));
            } else if trimmed.contains("false") && !trimmed.contains("assert") {
                mutation_candidates.push((
                    idx + 1,
                    line.to_string(),
                    line.replacen("false", "true", 1),
                ));
            } else if trimmed.contains(" > ") {
                mutation_candidates.push((idx + 1, line.to_string(), line.replace(" > ", " <= ")));
            } else if trimmed.contains(" < ") {
                mutation_candidates.push((idx + 1, line.to_string(), line.replace(" < ", " >= ")));
            } else if trimmed.contains(" + ") && !trimmed.contains("++") {
                mutation_candidates.push((idx + 1, line.to_string(), line.replace(" + ", " - ")));
            }

            if mutation_candidates.len() >= max_mutants {
                break;
            }
        }

        let mut results = Vec::new();
        let parts = shlex::split(test_command)
            .unwrap_or_else(|| vec!["cargo".to_string(), "test".to_string()]);
        let mut guard = MutationGuard::new(full_path.clone(), original_content.clone());

        for (line_num, orig_line, mut_line) in mutation_candidates {
            let mut new_lines = lines.clone();
            new_lines[line_num - 1] = &mut_line;
            let mutated_content = new_lines.join("\n");
            fs::write(&full_path, &mutated_content)?;

            let (killed, status, snippet) = Self::run_mutation_command_with_timeout(
                &parts,
                repo_root,
                std::time::Duration::from_secs(10),
            );

            results.push(MutationResult {
                target_file: target_file.to_path_buf(),
                line_number: line_num,
                original_code: orig_line,
                mutated_code: mut_line,
                killed,
                status,
                output_snippet: snippet,
            });
        }

        // Always restore original content
        guard.restore()?;
        Ok(results)
    }

    /// Full workspace test integrity audit with optional mutation verification.
    pub fn audit_workspace_with_mutations(
        root: &Path,
        test_files: &[PathBuf],
        mutation_target: Option<(&Path, &str)>,
    ) -> TestIntegrityReport {
        let mut report = Self::audit_workspace(root, test_files);

        if let Some((target_file, test_cmd)) = mutation_target {
            // 1. Baseline verification check: ensure test suite passes on unmodified code first
            let base_parts = shlex::split(test_cmd)
                .unwrap_or_else(|| vec!["cargo".to_string(), "test".to_string()]);
            let (base_passed, _base_status, base_snip) = Self::run_mutation_command_with_timeout(
                &base_parts,
                root,
                std::time::Duration::from_secs(30),
            );

            if !base_passed {
                report.flaws.push(TestFlaw {
                    file_path: target_file.to_path_buf(),
                    line_number: None,
                    test_name: "baseline_execution".to_string(),
                    kind: TestFlawKind::ShallowAssertion,
                    description: format!(
                        "Baseline test execution failed before mutation evaluation: {}",
                        base_snip.lines().next().unwrap_or("Test suite failure")
                    ),
                    snippet: Some(base_snip),
                });
                report.verdict = IntegrityVerdict::Compromised;
                report.summary =
                    "Baseline test suite failed on unmodified code prior to mutation testing."
                        .to_string();
                return report;
            }

            if let Ok(mutations) =
                Self::evaluate_multi_point_mutations(root, target_file, test_cmd, 5)
            {
                let killed_count = mutations.iter().filter(|m| m.killed).count();
                let survived_count = mutations
                    .iter()
                    .filter(|m| m.status == Some(MutationStatus::Survived))
                    .count();
                let total = mutations.len();
                let denominator = killed_count + survived_count;
                let kill_rate = if denominator > 0 {
                    killed_count as f64 / denominator as f64
                } else {
                    0.0
                };

                if total == 0 {
                    report.flaws.push(TestFlaw {
                        file_path: target_file.to_path_buf(),
                        line_number: None,
                        test_name: "mutation_analysis".to_string(),
                        kind: TestFlawKind::ShallowAssertion,
                        description: "No mutable candidate statements found in target file to evaluate test integrity.".to_string(),
                        snippet: None,
                    });
                }

                for m in &mutations {
                    if !m.killed {
                        report.flaws.push(TestFlaw {
                            file_path: m.target_file.clone(),
                            line_number: Some(m.line_number),
                            test_name: format!("mutation_at_line_{}", m.line_number),
                            kind: TestFlawKind::SurvivingMutant,
                            description: format!(
                                "Surviving mutant on line {}: mutation '{}' was not detected by any test.",
                                m.line_number, m.mutated_code.trim()
                            ),
                            snippet: Some(m.mutated_code.clone()),
                        });
                    }
                }

                report.mutations = mutations;
                report.mutation_kill_rate = kill_rate;
                if kill_rate < 0.6 {
                    report.verdict = IntegrityVerdict::Compromised;
                } else if kill_rate < 1.0 && report.verdict == IntegrityVerdict::Sound {
                    report.verdict = IntegrityVerdict::FlawsDetected;
                }
            }
        }

        report
    }

    /// Full workspace test integrity audit.
    pub fn audit_workspace(root: &Path, test_files: &[PathBuf]) -> TestIntegrityReport {
        let mut all_flaws = Vec::new();
        let mut total_tests = 0;

        for path in test_files {
            let full = if path.is_absolute() {
                path.clone()
            } else {
                root.join(path)
            };
            if let Ok(content) = fs::read_to_string(&full) {
                let test_count = content.matches("#[test]").count()
                    + content.matches("def test_").count()
                    + content.matches("it(").count()
                    + content.matches("test(").count();
                total_tests += test_count.max(1);
                let flaws = Self::audit_test_file(&full, &content);
                all_flaws.extend(flaws);
            }
        }

        let verdict = if all_flaws.iter().any(|f| {
            f.kind == TestFlawKind::TrivialAssertion || f.kind == TestFlawKind::EmptyTestBody
        }) {
            IntegrityVerdict::Compromised
        } else if !all_flaws.is_empty() {
            IntegrityVerdict::FlawsDetected
        } else {
            IntegrityVerdict::Sound
        };

        let summary = format!(
            "Analyzed {} tests across {} test files: {} flaws detected (Verdict: {:?})",
            total_tests,
            test_files.len(),
            all_flaws.len(),
            verdict
        );

        TestIntegrityReport {
            total_tests_analyzed: total_tests,
            flaws: all_flaws,
            mutations: Vec::new(),
            mutation_kill_rate: 0.0,
            verdict,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_trivial_assertions() {
        let code = r#"
            #[test]
            fn test_fake() {
                let x = 42;
                assert!(true);
            }

            #[test]
            fn test_weak_numbers() {
                assert_eq!(1, 1);
            }
        "#;
        let flaws = TestIntegrityAuditor::audit_test_file(Path::new("dummy_test.rs"), code);
        assert_eq!(flaws.len(), 2);
        assert_eq!(flaws[0].kind, TestFlawKind::TrivialAssertion);
        assert_eq!(flaws[1].kind, TestFlawKind::TrivialAssertion);
    }

    #[test]
    fn test_detect_empty_test_body() {
        let code = r#"
            #[test]
            fn test_empty() {
                // TODO: implement later
            }
        "#;
        let flaws = TestIntegrityAuditor::audit_test_file(Path::new("dummy_test.rs"), code);
        assert!(flaws.iter().any(|f| f.kind == TestFlawKind::EmptyTestBody));
    }

    #[test]
    fn test_detect_shallow_assertion() {
        let code = r#"
            #[test]
            fn test_shallow() {
                let res = do_something();
                assert!(res.is_ok());
            }
        "#;
        let flaws = TestIntegrityAuditor::audit_test_file(Path::new("dummy_test.rs"), code);
        assert!(
            flaws
                .iter()
                .any(|f| f.kind == TestFlawKind::ShallowAssertion)
        );
    }

    #[test]
    fn test_sound_test() {
        let code = r#"
            #[test]
            fn test_robust() {
                let res = calculate_sum(2, 3);
                assert_eq!(res, 5);
            }
        "#;
        let flaws = TestIntegrityAuditor::audit_test_file(Path::new("dummy_test.rs"), code);
        assert!(flaws.is_empty());
    }

    #[test]
    fn test_detect_unreachable_assertion() {
        let code = r#"
            #[test]
            fn test_early_exit() {
                return;
                assert_eq!(1, 2);
            }
        "#;
        let flaws = TestIntegrityAuditor::audit_test_file(Path::new("dummy_test.rs"), code);
        assert!(
            flaws
                .iter()
                .any(|f| f.kind == TestFlawKind::UnreachableAssertion)
        );
    }

    #[test]
    fn test_detect_mock_only_without_production() {
        let code = r#"
            #[test]
            fn test_mock_only() {
                let mut mock = MockDatabase::new();
                mock.expect_query().times(1);
            }
        "#;
        let flaws = TestIntegrityAuditor::audit_test_file(Path::new("dummy_test.rs"), code);
        assert!(
            flaws
                .iter()
                .any(|f| f.kind == TestFlawKind::MockOnlyWithoutProductionCode)
        );
    }

    #[test]
    fn test_mutation_guard_restores_on_drop() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("source.rs");
        let original_code = "pub fn add(a: i32, b: i32) -> i32 { a + b }";
        fs::write(&file_path, original_code).unwrap();

        {
            let mut guard = MutationGuard::new(file_path.clone(), original_code.to_string());
            fs::write(&file_path, "pub fn add(a: i32, b: i32) -> i32 { a - b }").unwrap();
            // Guard dropped here without calling guard.restore()
            let _ = &mut guard;
        }

        // Must be automatically restored by Drop
        let content_after_drop = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content_after_drop, original_code);
    }

    #[test]
    fn test_zero_mutation_candidates_is_not_killed() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("immutable.rs");
        fs::write(&file_path, "// Only comments here\n").unwrap();

        let res =
            TestIntegrityAuditor::evaluate_mutation(dir.path(), &file_path, "cargo test").unwrap();
        assert!(
            !res.killed,
            "Zero mutation candidates must evaluate to killed: false"
        );
        assert_eq!(res.status, Some(MutationStatus::NoCandidates));
    }

    #[test]
    fn test_mutation_infrastructure_failure_on_nonexistent_binary() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("logic.rs");
        fs::write(&file_path, "pub fn is_ok() -> bool { true }\n").unwrap();

        let res = TestIntegrityAuditor::evaluate_mutation(
            dir.path(),
            &file_path,
            "this_command_definitely_does_not_exist_at_all_xyz",
        )
        .unwrap();

        assert!(!res.killed);
        assert_eq!(res.status, Some(MutationStatus::InfrastructureFailure));
    }

    #[test]
    fn test_mutation_chatty_test_output_drains_without_hang() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("logic.rs");
        fs::write(&file_path, "pub fn is_ok() -> bool { true }\n").unwrap();

        // Generate > 70KB of output to ensure OS pipe buffer (>64KB) is exceeded
        // sh -c 'python3 -c "print(\"A\" * 75000)" && exit 0'
        let res = TestIntegrityAuditor::evaluate_mutation(
            dir.path(),
            &file_path,
            "sh -c 'python3 -c \"print(\\\"A\\\" * 75000)\" && exit 0'",
        )
        .unwrap();

        assert!(!res.killed);
        assert_eq!(res.status, Some(MutationStatus::Survived));
    }

    #[test]
    fn test_empty_mutations_has_zero_kill_rate() {
        let report = TestIntegrityAuditor::audit_workspace(Path::new("."), &[]);
        assert_eq!(
            report.mutation_kill_rate, 0.0,
            "Empty mutations must default to 0.0 kill rate"
        );
    }
}
