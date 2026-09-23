//! Canonical change proposal contracts and software modification primitives.
//!
//! Kernel-owned contracts defining implementation hypotheses, change surfaces,
//! file preconditions, multi-file mutation proposals, and change provenance.
//!
//! Core principle: The model proposes. The runtime decides.
//! The model formulates change proposals; the runtime validates, authorizes,
//! atomically applies, re-observes, verifies, and reviews them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

use crate::ids::{MissionId, TaskId};
use crate::kernel::plan::{TaskRiskLevel, VerificationStrategy};
use crate::repo::query::ChangeImpactReport;

/// Unique identifier for a proposed change set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChangeProposalId(pub Uuid);

impl ChangeProposalId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(u: Uuid) -> Self {
        Self(u)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for ChangeProposalId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ChangeProposalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Structured implementation hypothesis formulated before making changes.
///
/// Forces explicit problem statement, root cause analysis, expected outcome,
/// and verification strategy before code modifications are permitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationHypothesis {
    pub problem: String,
    pub expected_cause: String,
    pub proposed_change: String,
    pub affected_components: Vec<String>,
    pub expected_result: String,
    pub verification_approach: String,
}

impl ImplementationHypothesis {
    pub fn new(
        problem: impl Into<String>,
        expected_cause: impl Into<String>,
        proposed_change: impl Into<String>,
        expected_result: impl Into<String>,
        verification_approach: impl Into<String>,
    ) -> Self {
        Self {
            problem: problem.into(),
            expected_cause: expected_cause.into(),
            proposed_change: proposed_change.into(),
            affected_components: Vec::new(),
            expected_result: expected_result.into(),
            verification_approach: verification_approach.into(),
        }
    }

    pub fn with_affected_components(mut self, components: Vec<String>) -> Self {
        self.affected_components = components;
        self
    }
}

/// Explicit boundary of repository modification.
///
/// Declares the full set of files, test files, configs, and target symbols
/// permitted to be touched by a proposed mutation set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSurface {
    /// Files strictly required for the core change.
    pub required_files: Vec<String>,
    /// Supporting files modified to preserve coherence (e.g. imports, re-exports).
    #[serde(default)]
    pub supporting_files: Vec<String>,
    /// Test files modified or created to verify the change.
    #[serde(default)]
    pub test_files: Vec<String>,
    /// Configuration, schema, or migration files affected.
    #[serde(default)]
    pub configuration_files: Vec<String>,
    /// Target symbols intended to be modified or added.
    #[serde(default)]
    pub target_symbols: Vec<String>,
    /// Justification required for any supporting or configuration files.
    #[serde(default)]
    pub justifications: HashMap<String, String>,
}

impl ChangeSurface {
    pub fn new(required_files: Vec<String>) -> Self {
        Self {
            required_files,
            supporting_files: Vec::new(),
            test_files: Vec::new(),
            configuration_files: Vec::new(),
            target_symbols: Vec::new(),
            justifications: HashMap::new(),
        }
    }

    pub fn all_target_files(&self) -> Vec<String> {
        let mut set = std::collections::BTreeSet::new();
        for f in &self.required_files {
            set.insert(f.clone());
        }
        for f in &self.supporting_files {
            set.insert(f.clone());
        }
        for f in &self.test_files {
            set.insert(f.clone());
        }
        for f in &self.configuration_files {
            set.insert(f.clone());
        }
        set.into_iter().collect()
    }

    pub fn contains_path(&self, path: &str) -> bool {
        let clean = path.trim().trim_start_matches('@').trim_start_matches("./");
        self.all_target_files().iter().any(|f| {
            let f_clean = f.trim().trim_start_matches('@').trim_start_matches("./");
            f_clean == clean
        })
    }
}

/// Precondition assertion verified against repository state prior to mutation.
///
/// Ensures the target file existence, expected content hash, or expected symbols
/// match assumptions before applying mutations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilePrecondition {
    pub path: String,
    pub expected_hash: Option<String>,
    #[serde(default)]
    pub expected_symbols: Vec<String>,
    pub must_exist: bool,
}

impl FilePrecondition {
    pub fn exists(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            expected_hash: None,
            expected_symbols: Vec::new(),
            must_exist: true,
        }
    }

    pub fn exists_with_hash(path: impl Into<String>, expected_hash: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            expected_hash: Some(expected_hash.into()),
            expected_symbols: Vec::new(),
            must_exist: true,
        }
    }

    pub fn must_not_exist(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            expected_hash: None,
            expected_symbols: Vec::new(),
            must_exist: false,
        }
    }

    pub fn with_expected_symbols(mut self, symbols: Vec<String>) -> Self {
        self.expected_symbols = symbols;
        self
    }
}

/// Granular, typed mutation operation on a single file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileMutationOp {
    /// Exact or indent-tolerant substring replacement.
    Substring {
        old_content: String,
        new_content: String,
    },
    /// Replacement of a 1-indexed line range [start_line..=end_line].
    LineRange {
        start_line: usize,
        end_line: usize,
        new_content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_old: Option<String>,
    },
    /// Insertion before or after a 1-indexed line number.
    Insert {
        line_number: usize,
        content: String,
        after: bool,
    },
    /// Deletion of a 1-indexed line range [start_line..=end_line].
    Delete {
        start_line: usize,
        end_line: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_old: Option<String>,
    },
    /// Application of a unified diff patch.
    Patch { patch: String },
    /// Creation of a brand new file with specified content.
    CreateNew { content: String },
    /// Whole-file replacement requiring explicit justification and base hash verification.
    ReplaceFull {
        content: String,
        expected_base_hash: Option<String>,
        justification: String,
    },
}

/// Individual file mutation proposal within a change set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMutationProposal {
    pub path: String,
    pub operation: FileMutationOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_hash: Option<String>,
    pub rationale: String,
}

impl FileMutationProposal {
    pub fn new(
        path: impl Into<String>,
        operation: FileMutationOp,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            operation,
            base_hash: None,
            rationale: rationale.into(),
        }
    }

    pub fn with_base_hash(mut self, hash: impl Into<String>) -> Self {
        self.base_hash = Some(hash.into());
        self
    }
}

/// Authoritative change proposal submitted for runtime authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeProposal {
    pub id: ChangeProposalId,
    pub task_id: TaskId,
    pub mission_id: MissionId,
    pub intent: ImplementationHypothesis,
    pub change_surface: ChangeSurface,
    #[serde(default)]
    pub preconditions: Vec<FilePrecondition>,
    pub mutations: Vec<FileMutationProposal>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub verification_plan: Vec<VerificationStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<TaskRiskLevel>,
    pub timestamp: DateTime<Utc>,
}

impl ChangeProposal {
    pub fn new(
        task_id: TaskId,
        mission_id: MissionId,
        intent: ImplementationHypothesis,
        change_surface: ChangeSurface,
        mutations: Vec<FileMutationProposal>,
    ) -> Self {
        Self {
            id: ChangeProposalId::new(),
            task_id,
            mission_id,
            intent,
            change_surface,
            preconditions: Vec::new(),
            mutations,
            assumptions: Vec::new(),
            verification_plan: Vec::new(),
            risk_level: None,
            timestamp: Utc::now(),
        }
    }

    pub fn with_preconditions(mut self, preconditions: Vec<FilePrecondition>) -> Self {
        self.preconditions = preconditions;
        self
    }

    pub fn with_assumptions(mut self, assumptions: Vec<String>) -> Self {
        self.assumptions = assumptions;
        self
    }

    pub fn with_verification_plan(mut self, plan: Vec<VerificationStrategy>) -> Self {
        self.verification_plan = plan;
        self
    }

    pub fn with_risk_level(mut self, risk: TaskRiskLevel) -> Self {
        self.risk_level = Some(risk);
        self
    }
}

/// Reconciliation violation detected before mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReconciliationViolation {
    StaleFileHash {
        path: String,
        expected: String,
        actual: String,
    },
    MissingFile {
        path: String,
    },
    UnexpectedFileExists {
        path: String,
    },
    MissingSymbol {
        symbol: String,
        path: String,
    },
    InvalidatedAssumption {
        assumption: String,
    },
    ProtectedFileAccess {
        path: String,
        reason: String,
    },
    GeneratedFileDirectEdit {
        path: String,
        generator: Option<String>,
    },
    ConcurrentTaskConflict {
        path: String,
        conflicting_task_id: TaskId,
    },
    ScopeViolation {
        path: String,
        reason: String,
    },
    PolicyBlocked {
        reason: String,
    },
}

impl fmt::Display for ReconciliationViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleFileHash {
                path,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "stale file hash for '{}': expected {}, found {}",
                    path, expected, actual
                )
            }
            Self::MissingFile { path } => write!(f, "required file '{}' does not exist", path),
            Self::UnexpectedFileExists { path } => {
                write!(f, "file '{}' already exists when expected absent", path)
            }
            Self::MissingSymbol { symbol, path } => {
                write!(f, "symbol '{}' not found in '{}'", symbol, path)
            }
            Self::InvalidatedAssumption { assumption } => {
                write!(f, "invalidated assumption: {}", assumption)
            }
            Self::ProtectedFileAccess { path, reason } => {
                write!(f, "access to protected path '{}' denied: {}", path, reason)
            }
            Self::GeneratedFileDirectEdit { path, generator } => {
                write!(
                    f,
                    "direct edit of generated file '{}' rejected (generator: {:?})",
                    path, generator
                )
            }
            Self::ConcurrentTaskConflict {
                path,
                conflicting_task_id,
            } => {
                write!(
                    f,
                    "concurrent task conflict on '{}': locked by task {}",
                    path, conflicting_task_id
                )
            }
            Self::ScopeViolation { path, reason } => {
                write!(f, "scope violation for '{}': {}", path, reason)
            }
            Self::PolicyBlocked { reason } => write!(f, "policy blocked: {}", reason),
        }
    }
}

/// Result of pre-mutation repository reconciliation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconciliationReport {
    pub is_valid: bool,
    pub violations: Vec<ReconciliationViolation>,
    pub pre_change_impact: Option<ChangeImpactReport>,
    pub recommended_tests: Vec<String>,
}

impl ReconciliationReport {
    pub fn valid(
        pre_change_impact: Option<ChangeImpactReport>,
        recommended_tests: Vec<String>,
    ) -> Self {
        Self {
            is_valid: true,
            violations: Vec::new(),
            pre_change_impact,
            recommended_tests,
        }
    }

    pub fn invalid(violations: Vec<ReconciliationViolation>) -> Self {
        Self {
            is_valid: false,
            violations,
            pre_change_impact: None,
            recommended_tests: Vec::new(),
        }
    }
}

/// Diff inspection violation detected after mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiffReviewViolation {
    FakeImplementation {
        path: String,
        line: usize,
        snippet: String,
        pattern: String,
    },
    WeakerTestAssertion {
        path: String,
        line: usize,
        snippet: String,
    },
    AccidentalEdit {
        path: String,
        reason: String,
    },
    LeftoverDebug {
        path: String,
        line: usize,
        snippet: String,
    },
    UnresolvedTodo {
        path: String,
        line: usize,
        snippet: String,
    },
}

impl fmt::Display for DiffReviewViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FakeImplementation {
                path,
                line,
                snippet,
                pattern,
            } => {
                write!(
                    f,
                    "fake implementation in {}:{} (pattern: '{}'): {}",
                    path, line, pattern, snippet
                )
            }
            Self::WeakerTestAssertion {
                path,
                line,
                snippet,
            } => {
                write!(
                    f,
                    "weakened test assertion in {}:{}: {}",
                    path, line, snippet
                )
            }
            Self::AccidentalEdit { path, reason } => {
                write!(f, "accidental edit in '{}': {}", path, reason)
            }
            Self::LeftoverDebug {
                path,
                line,
                snippet,
            } => {
                write!(f, "leftover debug code in {}:{}: {}", path, line, snippet)
            }
            Self::UnresolvedTodo {
                path,
                line,
                snippet,
            } => {
                write!(f, "unresolved TODO/FIXME in {}:{}: {}", path, line, snippet)
            }
        }
    }
}

/// Outcome of diff self-review pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffReviewReport {
    pub passed: bool,
    pub violations: Vec<DiffReviewViolation>,
    pub files_reviewed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

impl DiffReviewReport {
    pub fn clean(files_reviewed: usize, lines_added: usize, lines_removed: usize) -> Self {
        Self {
            passed: true,
            violations: Vec::new(),
            files_reviewed,
            lines_added,
            lines_removed,
        }
    }

    pub fn with_violations(
        violations: Vec<DiffReviewViolation>,
        files_reviewed: usize,
        lines_added: usize,
        lines_removed: usize,
    ) -> Self {
        let passed = violations.is_empty();
        Self {
            passed,
            violations,
            files_reviewed,
            lines_added,
            lines_removed,
        }
    }
}

/// Durable audit record binding a change proposal to its requirements and verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeProvenanceRecord {
    pub proposal_id: ChangeProposalId,
    pub task_id: TaskId,
    pub mission_id: MissionId,
    pub requirement_keys: Vec<String>,
    pub affected_files: Vec<String>,
    pub affected_symbols: Vec<String>,
    pub pre_mutation_hash: String,
    pub post_mutation_hash: String,
    pub diff_summary: String,
    pub verification_passed: bool,
    pub timestamp: DateTime<Utc>,
}
