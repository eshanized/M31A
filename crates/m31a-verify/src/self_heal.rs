//! Targeted self-healing prompt generation from compiler and test diagnostics.

use crate::diagnostics::{Diagnostic, DiagnosticSeverity};
use crate::runners::RawRunResult;
use crate::verification_timeline::VerificationStatus;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationReport {
    pub success: bool,
    pub exit_code: i32,
    pub command: String,
    pub error_count: usize,
    pub warning_count: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub raw_output: String,
    pub self_healing_prompt: Option<String>,
    pub tests_passed: Option<usize>,
    pub tests_failed: Option<usize>,
    pub tests_total: Option<usize>,
    pub status: VerificationStatus,
}

pub struct SelfHealingGenerator;

impl SelfHealingGenerator {
    fn parse_test_counts(output: &str) -> (Option<usize>, Option<usize>, Option<usize>) {
        // 1. Rust format: "test result: ok. 5 passed; 0 failed"
        if let Some(pos) = output.find("test result:") {
            let slice = &output[pos..];
            if let Some(p_pos) = slice.find(" passed;") {
                let start = slice[..p_pos]
                    .rfind(|c: char| !c.is_ascii_digit())
                    .map(|i| i + 1)
                    .unwrap_or(0);
                if let Ok(passed) = slice[start..p_pos].trim().parse::<usize>() {
                    let mut failed = 0;
                    if let Some(f_pos) = slice.find(" failed;") {
                        let f_start = slice[..f_pos]
                            .rfind(|c: char| !c.is_ascii_digit())
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        if let Ok(f) = slice[f_start..f_pos].trim().parse::<usize>() {
                            failed = f;
                        }
                    }
                    return (Some(passed), Some(failed), Some(passed + failed));
                }
            }
        }

        // 2. Pytest format: "X passed"
        if let Some(pos) = output.find(" passed") {
            let start = output[..pos]
                .rfind(|c: char| !c.is_ascii_digit())
                .map(|i| i + 1)
                .unwrap_or(0);
            if let Ok(passed) = output[start..pos].trim().parse::<usize>() {
                return (Some(passed), Some(0), Some(passed));
            }
        }

        (None, None, None)
    }

    /// Evaluates raw run results, parses diagnostics, and generates an automated self-healing prompt if failed.
    pub fn evaluate(raw: RawRunResult, parsed_diags: Vec<Diagnostic>) -> VerificationReport {
        let error_count = parsed_diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count();
        let warning_count = parsed_diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count();

        let mut self_healing_prompt = None;
        if !raw.success || error_count > 0 {
            self_healing_prompt = Some(Self::build_prompt(&raw, &parsed_diags));
        }

        let combined_output = format!("{}\n{}", raw.stdout, raw.stderr);
        let is_test_cmd = raw.command.contains("test");
        let (tests_passed, tests_failed, tests_total) = Self::parse_test_counts(&combined_output);

        let status = if raw.timed_out {
            VerificationStatus::Timeout { duration_ms: 0 }
        } else if !raw.success || error_count > 0 {
            VerificationStatus::Failure {
                exit_code: Some(raw.exit_code),
                diagnostics: self_healing_prompt
                    .clone()
                    .unwrap_or_else(|| combined_output.clone()),
            }
        } else if is_test_cmd && tests_total == Some(0) {
            VerificationStatus::InsufficientEvidence {
                reason: "0 tests executed during test run".to_string(),
            }
        } else {
            VerificationStatus::Success
        };

        let success = matches!(status, VerificationStatus::Success);

        VerificationReport {
            success,
            exit_code: raw.exit_code,
            command: raw.command,
            error_count,
            warning_count,
            diagnostics: parsed_diags,
            raw_output: combined_output.trim().to_string(),
            self_healing_prompt,
            tests_passed,
            tests_failed,
            tests_total,
            status,
        }
    }

    fn build_prompt(raw: &RawRunResult, diags: &[Diagnostic]) -> String {
        let mut prompt = String::new();
        prompt.push_str("[Automated verification failed after code changes]\n");
        prompt.push_str(&format!(
            "Command `{}` exited with status code {}.\n\n",
            raw.command, raw.exit_code
        ));

        if !diags.is_empty() {
            prompt.push_str("Targeted Compiler/Test Diagnostics:\n");
            for (idx, d) in diags.iter().enumerate().take(5) {
                let col_str = d.column.map(|c| format!(":{}", c)).unwrap_or_default();
                prompt.push_str(&format!(
                    "{}. [{:?}] {} ({}:{}{})\n   Message: {}\n",
                    idx + 1,
                    d.severity,
                    d.code.as_deref().unwrap_or("error"),
                    d.file_path.display(),
                    d.line,
                    col_str,
                    d.message
                ));

                if let Some(snippet) = &d.snippet {
                    prompt.push_str(&format!("   Code:\n     {}\n", snippet));
                }
                if let Some(fix) = &d.suggested_fix {
                    prompt.push_str(&format!("   Suggestion: {}\n", fix));
                }
                prompt.push('\n');
            }

            if diags.len() > 5 {
                prompt.push_str(&format!(
                    "... and {} additional diagnostics omitted.\n\n",
                    diags.len() - 5
                ));
            }
        } else {
            prompt.push_str("Raw Failure Output:\n```\n");
            let tail: Vec<&str> = raw
                .stderr
                .lines()
                .chain(raw.stdout.lines())
                .rev()
                .take(30)
                .collect();
            let mut tail_reversed = tail;
            tail_reversed.reverse();
            prompt.push_str(&tail_reversed.join("\n"));
            prompt.push_str("\n```\n");
        }

        prompt.push_str(
            "Please inspect the errors above and generate an atomic patch to resolve them.",
        );
        prompt
    }
}
