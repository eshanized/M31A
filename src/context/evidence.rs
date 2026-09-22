//! Task-aware semantic context and evidence selection models.
//!
//! Provides structured evidence typing, deterministic relevance scoring,
//! progressive disclosure pipelines, and explicit vs derived context tracking.

use crate::context::envelope::TrustLevel;
use crate::context::tokenizer::TokenizerAdapter;
use crate::kernel::seams::context::{
    ContextCompilationRequest, OmittedEvidenceContract, SelectedEvidenceContract,
};
use crate::repo::query::{BoundedQueryEngine, QueryBounds};
use crate::repo::types::{FactClass, RepositorySymbol, SourceSliceKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// Authoritative origin classification for selected context evidence (Section 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOrigin {
    /// Directly supplied by task description, user input, or explicit runtime arguments.
    Explicit,
    /// Discovered through deterministic repository code graph relationships (calls, imports, tests).
    Derived,
    /// Discovered through heuristic term matching, token expansion, or search rankings.
    Inferred,
    /// Retrieved from prior execution turns, previous attempts, or verification history.
    Historical,
}

impl EvidenceOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Derived => "derived",
            Self::Inferred => "inferred",
            Self::Historical => "historical",
        }
    }
}

impl std::fmt::Display for EvidenceOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Explicit justification for why an evidence item was included in compiled context (Section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSelectionReason {
    /// Explicitly named in task objective or runtime request parameters.
    DirectTaskReference,
    /// Exact or high-confidence match on symbol name.
    SymbolNameMatch,
    /// Direct structural dependency (import / type usage / implements).
    DirectDependency,
    /// Immediate upstream caller invoking the target symbol.
    DirectCaller,
    /// Immediate downstream callee invoked by the target symbol.
    DirectCallee,
    /// Test symbol or test file verifying target implementation.
    AssociatedTest,
    /// Build manifest, package target, or environment configuration.
    ConfigurationOrBuild,
    /// Discovered via change impact analysis blast radius.
    ChangeImpact,
    /// Discovered through recent workspace modification or Git drift.
    RecentChange,
    /// Subsystem architectural boundary or entry point overview.
    SubsystemArchitecture,
    /// Related to diagnostic failure evidence or runtime error trace.
    DiagnosticFailure,
}

impl EvidenceSelectionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DirectTaskReference => "direct_task_reference",
            Self::SymbolNameMatch => "symbol_name_match",
            Self::DirectDependency => "direct_dependency",
            Self::DirectCaller => "direct_caller",
            Self::DirectCallee => "direct_callee",
            Self::AssociatedTest => "associated_test",
            Self::ConfigurationOrBuild => "configuration_or_build",
            Self::ChangeImpact => "change_impact",
            Self::RecentChange => "recent_change",
            Self::SubsystemArchitecture => "subsystem_architecture",
            Self::DiagnosticFailure => "diagnostic_failure",
        }
    }
}

impl std::fmt::Display for EvidenceSelectionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Diagnostic health status of context selection (Section 21).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ContextSelectionStatus {
    /// All requested and related evidence was successfully discovered and bound within budget.
    #[default]
    Complete,
    /// Evidence retrieval succeeded, but lower-priority items were omitted due to budget ceilings.
    Partial,
    /// Workspace files differed from indexed graph hashes; source was refreshed or noted as stale.
    PartiallyStale,
    /// Partial repository graph or index available; graceful degradation applied.
    Degraded,
    /// Repository intelligence or query engine was completely unavailable.
    Unavailable,
}

impl ContextSelectionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::PartiallyStale => "partially_stale",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        }
    }

    pub fn is_usable(&self) -> bool {
        matches!(
            self,
            Self::Complete | Self::Partial | Self::PartiallyStale | Self::Degraded
        )
    }
}

impl std::fmt::Display for ContextSelectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// High-level mode of task-aware context selection (Section 4, 14, 15).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum TaskContextMode {
    /// Standard feature implementation or targeted code edit.
    #[default]
    Implementation,
    /// Diagnostic failure analysis for failing tests or compilation errors.
    Debugging,
    /// Subsystem boundary refactoring, architecture review, or layer boundary work.
    Architecture,
    /// Initial repository intake, topology overview, or greenfield exploration.
    Exploration,
}

impl TaskContextMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Implementation => "implementation",
            Self::Debugging => "debugging",
            Self::Architecture => "architecture",
            Self::Exploration => "exploration",
        }
    }
}

impl std::fmt::Display for TaskContextMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

use std::fmt;

/// An individual item of selected repository evidence (Section 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedEvidenceItem {
    pub id: String,
    pub source_path: String,
    pub origin: EvidenceOrigin,
    pub reason: EvidenceSelectionReason,
    pub relevance_score: u32,
    pub slice_kind: SourceSliceKind,
    pub content: String,
    pub token_count: usize,
    pub provenance: String,
    pub trust_level: TrustLevel,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub fact_class: FactClass,
}

impl SelectedEvidenceItem {
    pub fn to_manifest_record(&self) -> EvidenceManifestRecord {
        EvidenceManifestRecord {
            id: self.id.clone(),
            source_path: self.source_path.clone(),
            origin: self.origin,
            reason: self.reason,
            slice_kind: self.slice_kind,
            token_count: self.token_count,
            relevance_score: self.relevance_score,
            provenance: self.provenance.clone(),
        }
    }
}

/// Audit record tracking an admitted evidence item in ContextCompilationManifest (Section 18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceManifestRecord {
    pub id: String,
    pub source_path: String,
    pub origin: EvidenceOrigin,
    pub reason: EvidenceSelectionReason,
    pub slice_kind: SourceSliceKind,
    pub token_count: usize,
    pub relevance_score: u32,
    pub provenance: String,
}

impl From<EvidenceManifestRecord> for SelectedEvidenceContract {
    fn from(r: EvidenceManifestRecord) -> Self {
        Self {
            id: r.id,
            source_path: r.source_path,
            origin: r.origin.as_str().to_string(),
            reason: r.reason.as_str().to_string(),
            slice_kind: r.slice_kind.as_str().to_string(),
            token_count: r.token_count,
            relevance_score: r.relevance_score,
            provenance: r.provenance,
        }
    }
}

impl From<&EvidenceManifestRecord> for SelectedEvidenceContract {
    fn from(r: &EvidenceManifestRecord) -> Self {
        Self {
            id: r.id.clone(),
            source_path: r.source_path.clone(),
            origin: r.origin.as_str().to_string(),
            reason: r.reason.as_str().to_string(),
            slice_kind: r.slice_kind.as_str().to_string(),
            token_count: r.token_count,
            relevance_score: r.relevance_score,
            provenance: r.provenance.clone(),
        }
    }
}

/// Audit record tracking an evidence item that was omitted due to budget/priority (Section 18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmittedEvidenceRecord {
    pub id: String,
    pub source_path: String,
    pub reason: String,
    pub relevance_score: u32,
}

impl From<OmittedEvidenceRecord> for OmittedEvidenceContract {
    fn from(r: OmittedEvidenceRecord) -> Self {
        Self {
            id: r.id,
            source_path: r.source_path,
            reason: r.reason,
            relevance_score: r.relevance_score,
        }
    }
}

impl From<&OmittedEvidenceRecord> for OmittedEvidenceContract {
    fn from(r: &OmittedEvidenceRecord) -> Self {
        Self {
            id: r.id.clone(),
            source_path: r.source_path.clone(),
            reason: r.reason.clone(),
            relevance_score: r.relevance_score,
        }
    }
}

/// Outcome of task-aware evidence selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEvidenceSelectionResult {
    pub selected_items: Vec<SelectedEvidenceItem>,
    pub omitted_items: Vec<OmittedEvidenceRecord>,
    pub status: ContextSelectionStatus,
    pub mode: TaskContextMode,
    pub total_selected_tokens: usize,
    pub overview_summary: Option<String>,
    pub change_impact_summary: Option<String>,
}

/// Deterministic weights for evidence prioritization (Section 6).
pub mod relevance_weights {
    pub const EXPLICIT_TASK_REF: u32 = 100;
    pub const DIAGNOSTIC_FAILURE: u32 = 85;
    pub const ASSOCIATED_TEST: u32 = 70;
    pub const SYMBOL_NAME_MATCH: u32 = 60;
    pub const CHANGE_IMPACT: u32 = 50;
    pub const DIRECT_DEPENDENCY: u32 = 45;
    pub const DIRECT_CALLER_CALLEE: u32 = 40;
    pub const KEYWORD_SEARCH: u32 = 30;
    pub const SAME_SUBSYSTEM: u32 = 25;
    pub const ENTRY_POINT: u32 = 20;
}

/// Performs deterministic 5-stage progressive disclosure and task-aware evidence selection (Section 4, 5, 10).
pub fn select_task_aware_evidence(
    query_engine: Option<&BoundedQueryEngine>,
    req: &ContextCompilationRequest,
    max_evidence_tokens: usize,
    tokenizer: &TokenizerAdapter,
) -> TaskEvidenceSelectionResult {
    let qe = match query_engine {
        Some(engine) => engine,
        None => {
            return TaskEvidenceSelectionResult {
                selected_items: Vec::new(),
                omitted_items: Vec::new(),
                status: ContextSelectionStatus::Unavailable,
                mode: TaskContextMode::Implementation,
                total_selected_tokens: 0,
                overview_summary: None,
                change_impact_summary: None,
            };
        }
    };

    let mission_obj = req.mission_objective.as_deref().unwrap_or("");
    let task_obj = req.task_objective.as_deref().unwrap_or(mission_obj);

    // 1. Detect TaskContextMode
    let combined_text = format!("{} {}", mission_obj, task_obj).to_lowercase();
    let mode = if req.error_context.is_some()
        || combined_text.contains("error:")
        || combined_text.contains("fail")
        || combined_text.contains("panic")
        || combined_text.contains("fix ")
        || combined_text.contains("bug")
    {
        TaskContextMode::Debugging
    } else if combined_text.contains("architecture")
        || combined_text.contains("subsystem")
        || combined_text.contains("boundary")
        || combined_text.contains("refactor")
    {
        TaskContextMode::Architecture
    } else if combined_text.contains("explore")
        || combined_text.contains("discovery")
        || combined_text.contains("overview")
    {
        TaskContextMode::Exploration
    } else {
        TaskContextMode::Implementation
    };

    // 2. Discover Explicit Target and Test Files
    let mut target_files: Vec<String> = req.explicit_files.clone();
    let mut test_files: Vec<String> = Vec::new();

    for word in mission_obj
        .split_whitespace()
        .chain(task_obj.split_whitespace())
    {
        let clean = word.trim_matches(|c: char| {
            c == '@'
                || c == '"'
                || c == '\''
                || c == '.'
                || c == ','
                || c == ';'
                || c == ':'
                || c == '('
                || c == ')'
        });
        if clean.ends_with(".rs")
            || clean.ends_with(".py")
            || clean.ends_with(".ts")
            || clean.ends_with(".js")
            || clean.ends_with(".go")
        {
            if clean.contains("test") {
                if !test_files.contains(&clean.to_string()) {
                    test_files.push(clean.to_string());
                }
            } else if !target_files.contains(&clean.to_string()) {
                target_files.push(clean.to_string());
            }
        }
    }

    // 3. Stage 1: Repository Overview & Topology
    let index_status = qe.get_index_status();
    let overview_summary = if matches!(
        mode,
        TaskContextMode::Exploration | TaskContextMode::Architecture
    ) || target_files.is_empty()
    {
        let eps = qe.find_entry_points(None, QueryBounds::new(5, 1, 1024));
        let mut ov = format!("Index Status: {}\n", index_status.as_str());
        if !eps.data.is_empty() {
            ov.push_str("Detected Entry Points:\n");
            for ep in &eps.data {
                ov.push_str(&format!(
                    "  - {} ({:?}) in {}:{}\n",
                    ep.id, ep.kind, ep.file_path, ep.line_number
                ));
            }
        }
        Some(ov)
    } else {
        None
    };

    // 4. Stage 2: Subsystem Identification
    let mut candidate_symbols: BTreeMap<
        String,
        (
            RepositorySymbol,
            u32,
            EvidenceOrigin,
            EvidenceSelectionReason,
        ),
    > = BTreeMap::new();

    // Add explicit symbols if specified
    for sym_name in &req.explicit_symbols {
        let res = qe.find_symbols(sym_name, QueryBounds::new(5, 1, 2048));
        for s in res.data {
            candidate_symbols.insert(
                s.id.clone(),
                (
                    s,
                    relevance_weights::EXPLICIT_TASK_REF,
                    EvidenceOrigin::Explicit,
                    EvidenceSelectionReason::DirectTaskReference,
                ),
            );
        }
    }

    // Add symbols from explicit files
    for tf in &target_files {
        let outline = qe.get_file_outline(tf, QueryBounds::new(20, 1, 4096));
        for s in outline.data {
            candidate_symbols.entry(s.id.clone()).or_insert((
                s,
                relevance_weights::EXPLICIT_TASK_REF,
                EvidenceOrigin::Explicit,
                EvidenceSelectionReason::DirectTaskReference,
            ));
        }
    }

    // Add symbols from error context if debugging
    if let Some(ref err_ctx) = req.error_context {
        for word in err_ctx.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
            if clean.len() >= 4 {
                let res = qe.find_symbols(clean, QueryBounds::new(3, 1, 1024));
                for s in res.data {
                    candidate_symbols.entry(s.id.clone()).or_insert((
                        s,
                        relevance_weights::DIAGNOSTIC_FAILURE,
                        EvidenceOrigin::Historical,
                        EvidenceSelectionReason::DiagnosticFailure,
                    ));
                }
            }
        }
    }

    // Keyword search terms for term expansion
    let stop_words: HashSet<&'static str> = [
        "the",
        "and",
        "for",
        "with",
        "this",
        "that",
        "from",
        "into",
        "task",
        "file",
        "test",
        "code",
        "fix",
        "implement",
        "update",
        "run",
        "verify",
    ]
    .into_iter()
    .collect();

    for word in mission_obj
        .split_whitespace()
        .chain(task_obj.split_whitespace())
    {
        let raw = word
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
            .to_lowercase();
        if raw.len() >= 4 && !stop_words.contains(raw.as_str()) {
            let res = qe.find_symbols(&raw, QueryBounds::new(3, 1, 2048));
            for s in res.data {
                candidate_symbols.entry(s.id.clone()).or_insert((
                    s,
                    relevance_weights::KEYWORD_SEARCH,
                    EvidenceOrigin::Inferred,
                    EvidenceSelectionReason::SymbolNameMatch,
                ));
            }
        }
    }

    // 5. Stage 3 & 4: Relational Neighborhood Expansion (Callers, Callees, Dependencies, Tests)
    let top_candidates: Vec<RepositorySymbol> = candidate_symbols
        .values()
        .map(|(s, _, _, _)| s.clone())
        .take(5)
        .collect();

    let mut expanded_relational: Vec<(
        RepositorySymbol,
        u32,
        EvidenceOrigin,
        EvidenceSelectionReason,
    )> = Vec::new();

    for sym in &top_candidates {
        // Associated tests
        let tests = qe.find_tests_for_symbol(&sym.id, QueryBounds::new(3, 1, 1024));
        for t in tests.data {
            expanded_relational.push((
                t,
                relevance_weights::ASSOCIATED_TEST,
                EvidenceOrigin::Derived,
                EvidenceSelectionReason::AssociatedTest,
            ));
        }

        // Callers
        let callers = qe.find_callers(&sym.id, QueryBounds::new(3, 1, 1024));
        for c in callers.data {
            expanded_relational.push((
                c,
                relevance_weights::DIRECT_CALLER_CALLEE,
                EvidenceOrigin::Derived,
                EvidenceSelectionReason::DirectCaller,
            ));
        }

        // Callees
        let callees = qe.find_callees(&sym.id, QueryBounds::new(3, 1, 1024));
        for c in callees.data {
            expanded_relational.push((
                c,
                relevance_weights::DIRECT_CALLER_CALLEE,
                EvidenceOrigin::Derived,
                EvidenceSelectionReason::DirectCallee,
            ));
        }
    }

    for (s, score, origin, reason) in expanded_relational {
        candidate_symbols
            .entry(s.id.clone())
            .and_modify(|existing| {
                if score > existing.1 {
                    *existing = (s.clone(), score, origin, reason);
                }
            })
            .or_insert((s, score, origin, reason));
    }

    // Calculate Change Impact if target files present
    let change_impact_summary = if !target_files.is_empty() {
        let impact = qe.calculate_change_impact(&target_files, QueryBounds::new(10, 2, 4096));
        let mut imp_text = format!("Change Impact for: {}\n", target_files.join(", "));
        if !impact.data.recommended_tests.is_empty() {
            imp_text.push_str("Recommended Verification Tests:\n");
            for t in &impact.data.recommended_tests {
                imp_text.push_str(&format!("  - {}\n", t));
            }
        }
        if !impact.data.downstream_affected_symbols.is_empty() {
            imp_text.push_str("Downstream Affected Symbols:\n");
            for sym in impact.data.downstream_affected_symbols.iter().take(5) {
                imp_text.push_str(&format!("  - {} in {}\n", sym.name, sym.file_path));
            }
        }
        Some(imp_text)
    } else {
        None
    };

    // 6. Stage 5: Ranking and Source Slice Selection
    let mut sorted_candidates: Vec<_> = candidate_symbols.into_values().collect();
    sorted_candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));

    let mut selected_items = Vec::new();
    let mut omitted_items = Vec::new();
    let mut accumulated_tokens = 0;
    let mut has_staleness = false;

    // Direct diagnostic failure evidence item if provided
    if let Some(ref err_ctx) = req.error_context {
        let err_tokens = tokenizer.count_tokens(err_ctx);
        if accumulated_tokens + err_tokens <= max_evidence_tokens {
            accumulated_tokens += err_tokens;
            selected_items.push(SelectedEvidenceItem {
                id: "diagnostic://failure".to_string(),
                source_path: "diagnostic://failure".to_string(),
                origin: EvidenceOrigin::Historical,
                reason: EvidenceSelectionReason::DiagnosticFailure,
                relevance_score: relevance_weights::DIAGNOSTIC_FAILURE,
                slice_kind: SourceSliceKind::ContiguousSlice,
                content: err_ctx.clone(),
                token_count: err_tokens,
                provenance: "diagnostic://failure".to_string(),
                trust_level: TrustLevel::UntrustedToolOutput,
                start_line: None,
                end_line: None,
                fact_class: FactClass::VerifiedFact,
            });
        }
    }

    // Check file staleness if workspace available
    for tf in &target_files {
        if qe.check_file_staleness(tf) == crate::repo::types::FileStaleness::Modified {
            has_staleness = true;
        }
    }

    for (sym, score, origin, reason) in sorted_candidates {
        // Decide slice kind based on relevance and distance
        let (slice_res, slice_kind) = if score >= relevance_weights::DIAGNOSTIC_FAILURE
            || target_files.iter().any(|tf| sym.file_path.ends_with(tf))
        {
            // Direct target: contiguous slice with surrounding context lines
            (
                qe.get_symbol_slice(&sym.id, 5, QueryBounds::new(1, 1, 2048)),
                SourceSliceKind::ContiguousSlice,
            )
        } else {
            // Relational neighbor or lower priority: signature only
            (
                qe.get_symbol_slice(&sym.id, 0, QueryBounds::new(1, 1, 512)),
                SourceSliceKind::SignatureOnly,
            )
        };

        if let Some(bounded_slice) = slice_res {
            let slice = bounded_slice.data;
            if slice.is_stale {
                has_staleness = true;
            }

            let item_tokens = tokenizer.count_tokens(&slice.content);

            if accumulated_tokens + item_tokens <= max_evidence_tokens {
                accumulated_tokens += item_tokens;
                selected_items.push(SelectedEvidenceItem {
                    id: sym.id.clone(),
                    source_path: sym.file_path.clone(),
                    origin,
                    reason,
                    relevance_score: score,
                    slice_kind,
                    content: slice.content,
                    token_count: item_tokens,
                    provenance: format!("repo://symbol/{}", sym.id),
                    trust_level: TrustLevel::UntrustedRepoContent,
                    start_line: Some(slice.start_line),
                    end_line: Some(slice.end_line),
                    fact_class: sym.fact_class,
                });
            } else {
                omitted_items.push(OmittedEvidenceRecord {
                    id: sym.id,
                    source_path: sym.file_path,
                    reason: "budget_exhausted".to_string(),
                    relevance_score: score,
                });
            }
        }
    }

    let status = if has_staleness {
        ContextSelectionStatus::PartiallyStale
    } else if !omitted_items.is_empty() {
        ContextSelectionStatus::Partial
    } else if selected_items.is_empty() {
        ContextSelectionStatus::Degraded
    } else {
        ContextSelectionStatus::Complete
    };

    TaskEvidenceSelectionResult {
        selected_items,
        omitted_items,
        status,
        mode,
        total_selected_tokens: accumulated_tokens,
        overview_summary,
        change_impact_summary,
    }
}
