//! Domain types and models for the 7-tier verification hierarchy and completion gate (VER-01, VER-04, D-01, D-04).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::ids::{ArtifactId, CheckId, MissionId, RequirementId, TaskId};

/// The strict 7-tier verification hierarchy (VER-01, D-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckTier {
    Deterministic = 1,
    Compiler = 2,
    Tests = 3,
    StaticAnalysis = 4,
    DiffInvariants = 5,
    IndependentReview = 6,
    ModelDiagnosis = 7,
}

impl CheckTier {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            1 => Some(CheckTier::Deterministic),
            2 => Some(CheckTier::Compiler),
            3 => Some(CheckTier::Tests),
            4 => Some(CheckTier::StaticAnalysis),
            5 => Some(CheckTier::DiffInvariants),
            6 => Some(CheckTier::IndependentReview),
            7 => Some(CheckTier::ModelDiagnosis),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            CheckTier::Deterministic => "deterministic",
            CheckTier::Compiler => "compiler",
            CheckTier::Tests => "tests",
            CheckTier::StaticAnalysis => "static_analysis",
            CheckTier::DiffInvariants => "diff_invariants",
            CheckTier::IndependentReview => "independent_review",
            CheckTier::ModelDiagnosis => "model_diagnosis",
        }
    }
}

impl fmt::Display for CheckTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Execution status of a verification check (D-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    Blocked,
    NotRun,
    SkippedWithReason,
}

impl CheckStatus {
    pub fn is_passed(&self) -> bool {
        matches!(self, CheckStatus::Passed)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, CheckStatus::Failed)
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self, CheckStatus::Blocked)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CheckStatus::Passed => "passed",
            CheckStatus::Failed => "failed",
            CheckStatus::Blocked => "blocked",
            CheckStatus::NotRun => "not_run",
            CheckStatus::SkippedWithReason => "skipped_with_reason",
        }
    }
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for CheckStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "passed" => Ok(CheckStatus::Passed),
            "failed" => Ok(CheckStatus::Failed),
            "blocked" => Ok(CheckStatus::Blocked),
            "not_run" | "notrun" => Ok(CheckStatus::NotRun),
            "skipped_with_reason" | "skipped" => Ok(CheckStatus::SkippedWithReason),
            other => Err(format!("unknown check status: '{other}'")),
        }
    }
}

/// Structured record of an individual verification check (VER-04, D-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub check_id: CheckId,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub tier: CheckTier,
    pub status: CheckStatus,
    pub command_or_tool: String,
    pub inputs_normalized: String,
    pub evidence_artifact_id: Option<ArtifactId>,
    pub summary: String,
    pub failure_class: Option<String>,
    pub snapshot_hash: String,
    pub created_at: DateTime<Utc>,
}

impl VerificationCheck {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mission_id: MissionId,
        task_id: TaskId,
        tier: CheckTier,
        status: CheckStatus,
        command_or_tool: impl Into<String>,
        inputs_normalized: impl Into<String>,
        evidence_artifact_id: Option<ArtifactId>,
        summary: impl Into<String>,
        failure_class: Option<String>,
        snapshot_hash: impl Into<String>,
    ) -> Self {
        Self {
            check_id: CheckId::new(),
            mission_id,
            task_id,
            tier,
            status,
            command_or_tool: command_or_tool.into(),
            inputs_normalized: inputs_normalized.into(),
            evidence_artifact_id,
            summary: summary.into(),
            failure_class,
            snapshot_hash: snapshot_hash.into(),
            created_at: Utc::now(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn passed(
        mission_id: MissionId,
        task_id: TaskId,
        tier: CheckTier,
        command_or_tool: impl Into<String>,
        inputs: impl Into<String>,
        evidence_artifact_id: Option<ArtifactId>,
        summary: impl Into<String>,
        snapshot_hash: impl Into<String>,
    ) -> Self {
        Self::new(
            mission_id,
            task_id,
            tier,
            CheckStatus::Passed,
            command_or_tool,
            inputs,
            evidence_artifact_id,
            summary,
            None,
            snapshot_hash,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn failed(
        mission_id: MissionId,
        task_id: TaskId,
        tier: CheckTier,
        command_or_tool: impl Into<String>,
        inputs: impl Into<String>,
        evidence_artifact_id: Option<ArtifactId>,
        summary: impl Into<String>,
        failure_class: Option<String>,
        snapshot_hash: impl Into<String>,
    ) -> Self {
        Self::new(
            mission_id,
            task_id,
            tier,
            CheckStatus::Failed,
            command_or_tool,
            inputs,
            evidence_artifact_id,
            summary,
            failure_class,
            snapshot_hash,
        )
    }

    pub fn blocked(
        mission_id: MissionId,
        task_id: TaskId,
        tier: CheckTier,
        snapshot_hash: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let reason_str = reason.into();
        Self::new(
            mission_id,
            task_id,
            tier,
            CheckStatus::Blocked,
            "blocked",
            "",
            None,
            format!("Blocked: {}", reason_str),
            Some("PrerequisiteBlocked".into()),
            snapshot_hash,
        )
    }

    pub fn skipped(
        mission_id: MissionId,
        task_id: TaskId,
        tier: CheckTier,
        snapshot_hash: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let reason_str = reason.into();
        Self::new(
            mission_id,
            task_id,
            tier,
            CheckStatus::SkippedWithReason,
            "skipped",
            "",
            None,
            format!("Skipped: {}", reason_str),
            None,
            snapshot_hash,
        )
    }
}

/// Explicit requirement-to-check coverage binding (VER-04, D-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementCheckCoverage {
    pub id: Uuid,
    pub requirement_id: RequirementId,
    pub check_id: CheckId,
    pub coverage_role: String,
    pub is_mandatory: bool,
}

impl RequirementCheckCoverage {
    pub fn new(
        requirement_id: RequirementId,
        check_id: CheckId,
        coverage_role: impl Into<String>,
        is_mandatory: bool,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            requirement_id,
            check_id,
            coverage_role: coverage_role.into(),
            is_mandatory,
        }
    }
}

/// Structured decision record emitted by the Completion Gate (VER-05, D-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionGateDecision {
    pub decision_id: Uuid,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub gate_scope: String,
    pub is_satisfied: bool,
    pub snapshot_hash: String,
    pub violations: Vec<String>,
    pub evidence_summary: String,
    pub created_at: DateTime<Utc>,
}

impl CompletionGateDecision {
    pub fn satisfied(
        mission_id: MissionId,
        task_id: Option<TaskId>,
        gate_scope: impl Into<String>,
        snapshot_hash: impl Into<String>,
        evidence_summary: impl Into<String>,
    ) -> Self {
        Self {
            decision_id: Uuid::now_v7(),
            mission_id,
            task_id,
            gate_scope: gate_scope.into(),
            is_satisfied: true,
            snapshot_hash: snapshot_hash.into(),
            violations: Vec::new(),
            evidence_summary: evidence_summary.into(),
            created_at: Utc::now(),
        }
    }

    pub fn deficient(
        mission_id: MissionId,
        task_id: Option<TaskId>,
        gate_scope: impl Into<String>,
        snapshot_hash: impl Into<String>,
        violations: Vec<String>,
        evidence_summary: impl Into<String>,
    ) -> Self {
        Self {
            decision_id: Uuid::now_v7(),
            mission_id,
            task_id,
            gate_scope: gate_scope.into(),
            is_satisfied: false,
            snapshot_hash: snapshot_hash.into(),
            violations,
            evidence_summary: evidence_summary.into(),
            created_at: Utc::now(),
        }
    }
}

/// Epistemic status of a diagnostic finding or failure evidence item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceEpistemicStatus {
    /// Directly observed in deterministic compiler, test, or process output.
    Observed,
    /// Inferred via static callgraph, dependency topology, or symbol impact.
    Inferred,
    /// Suspected candidate cause subject to verification.
    Suspected,
    /// Contradicted or disproven by fresh verification evidence.
    Contradicted,
    /// Unknown or inconclusive evidence status.
    #[default]
    Unknown,
}

impl fmt::Display for EvidenceEpistemicStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observed => write!(f, "observed"),
            Self::Inferred => write!(f, "inferred"),
            Self::Suspected => write!(f, "suspected"),
            Self::Contradicted => write!(f, "contradicted"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Structured compiler diagnostic item extracted from compiler stderr/stdout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerDiagnosticItem {
    pub code: Option<String>,
    pub message: String,
    pub file_path: Option<String>,
    pub line_number: Option<usize>,
    pub column_number: Option<usize>,
    pub span_snippet: Option<String>,
}

impl CompilerDiagnosticItem {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: None,
            message: message.into(),
            file_path: None,
            line_number: None,
            column_number: None,
            span_snippet: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_location(mut self, file: impl Into<String>, line: usize, col: usize) -> Self {
        self.file_path = Some(file.into());
        self.line_number = Some(line);
        self.column_number = Some(col);
        self
    }
}

impl From<crate::verification::diagnostics::DiagnosticItem> for CompilerDiagnosticItem {
    fn from(item: crate::verification::diagnostics::DiagnosticItem) -> Self {
        Self {
            code: item.code,
            message: item.message,
            file_path: Some(item.file_path),
            line_number: Some(item.line),
            column_number: Some(item.column),
            span_snippet: if !item.snippet_lines.is_empty() {
                Some(item.snippet_lines.join("\n"))
            } else {
                item.suggested_fix
            },
        }
    }
}

/// Structured test failure record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestFailureItem {
    pub test_name: String,
    pub failure_message: String,
    pub panic_location: Option<String>,
    pub is_pre_existing: bool,
}

impl TestFailureItem {
    pub fn new(test_name: impl Into<String>, failure_message: impl Into<String>) -> Self {
        Self {
            test_name: test_name.into(),
            failure_message: failure_message.into(),
            panic_location: None,
            is_pre_existing: false,
        }
    }

    pub fn with_panic_location(mut self, loc: impl Into<String>) -> Self {
        self.panic_location = Some(loc.into());
        self
    }

    pub fn mark_pre_existing(mut self, pre_existing: bool) -> Self {
        self.is_pre_existing = pre_existing;
        self
    }
}

/// Comprehensive, structured failure evidence model.
///
/// Unifies compiler diagnostics, test failures, affected symbols, git hashes,
/// and epistemic status without creating parallel universal failure objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureEvidence {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub primary_error: String,
    pub compiler_diagnostics: Vec<CompilerDiagnosticItem>,
    pub test_failures: Vec<TestFailureItem>,
    pub changed_files: Vec<String>,
    pub affected_symbols: Vec<String>,
    pub pre_mutation_hash: Option<String>,
    pub post_mutation_hash: Option<String>,
    pub epistemic_status: EvidenceEpistemicStatus,
    pub created_at: DateTime<Utc>,
}

impl FailureEvidence {
    pub fn new(mission_id: MissionId, task_id: TaskId, primary_error: impl Into<String>) -> Self {
        Self {
            mission_id,
            task_id,
            exit_code: None,
            stdout: None,
            stderr: None,
            primary_error: primary_error.into(),
            compiler_diagnostics: Vec::new(),
            test_failures: Vec::new(),
            changed_files: Vec::new(),
            affected_symbols: Vec::new(),
            pre_mutation_hash: None,
            post_mutation_hash: None,
            epistemic_status: EvidenceEpistemicStatus::Observed,
            created_at: Utc::now(),
        }
    }

    pub fn with_process_output(
        mut self,
        exit_code: Option<i32>,
        stdout: Option<String>,
        stderr: Option<String>,
    ) -> Self {
        self.exit_code = exit_code;
        self.stdout = stdout;
        self.stderr = stderr;
        self.parse_diagnostics_and_tests();
        self
    }

    pub fn with_changes(
        mut self,
        changed_files: Vec<String>,
        affected_symbols: Vec<String>,
        pre_hash: Option<String>,
        post_hash: Option<String>,
    ) -> Self {
        self.changed_files = changed_files;
        self.affected_symbols = affected_symbols;
        self.pre_mutation_hash = pre_hash;
        self.post_mutation_hash = post_hash;
        self
    }

    pub fn with_epistemic_status(mut self, status: EvidenceEpistemicStatus) -> Self {
        self.epistemic_status = status;
        self
    }

    /// Correlates test failures against known baseline failures to identify regressions (Section 18 & 19).
    pub fn correlate_baseline_failures(&mut self, baseline_failing_tests: &[String]) {
        for test in &mut self.test_failures {
            if baseline_failing_tests.iter().any(|b| b == &test.test_name) {
                test.is_pre_existing = true;
            }
        }
    }

    /// Whether this failure evidence contains newly introduced regressions.
    pub fn has_new_regressions(&self) -> bool {
        self.test_failures.iter().any(|t| !t.is_pre_existing)
            || !self.compiler_diagnostics.is_empty()
    }

    /// Internal parser extracting rustc diagnostics and cargo test failures from output.
    fn parse_diagnostics_and_tests(&mut self) {
        let combined = format!(
            "{}\n{}\n{}",
            self.primary_error,
            self.stderr.as_deref().unwrap_or(""),
            self.stdout.as_deref().unwrap_or("")
        );

        // 1. Parse structured diagnostics across Rust, TypeScript, Python, and Go
        let parsed_items = crate::verification::diagnostics::parse_diagnostics(&combined, None);
        for item in parsed_items {
            let diag: CompilerDiagnosticItem = item.into();
            if !self.compiler_diagnostics.iter().any(|d| {
                d.message == diag.message
                    && d.file_path == diag.file_path
                    && d.line_number == diag.line_number
            }) {
                self.compiler_diagnostics.push(diag);
            }
        }

        let lines: Vec<&str> = combined.lines().collect();
        for line in &lines {
            let trimmed = line.trim();

            // 2. Parse test failures: test test_name ... FAILED
            if trimmed.starts_with("test ") && trimmed.ends_with("... FAILED") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    let test_name = parts[1].to_string();
                    if !self.test_failures.iter().any(|t| t.test_name == test_name) {
                        let failure_msg = format!("Test {} failed", test_name);
                        self.test_failures
                            .push(TestFailureItem::new(test_name, failure_msg));
                    }
                }
            } else if (trimmed.starts_with("assertion failed:")
                || trimmed.starts_with("panicked at"))
                && let Some(last_test) = self.test_failures.last_mut()
            {
                last_test.failure_message = trimmed.to_string();
            }
        }
    }
}
