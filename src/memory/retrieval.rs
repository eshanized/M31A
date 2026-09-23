//! Task-Aware Structured Memory Retrieval.
//!
//! Provides deterministic relevance filtering for architectural decisions,
//! active assumptions, diagnostic failures, and review findings based on:
//! - Target files & symbols
//! - Requirement keys
//! - Diagnostic error signatures
//! - Mission and task scopes
//!
//! Excludes unrelated project history from model context.

use std::collections::HashSet;

use crate::ids::{MissionId, TaskId};
use crate::memory::repository::EngineeringMemoryStore;
use crate::memory::types::{
    AssumptionStatus, DecisionStatus, EngineeringMemorySnapshot, MemoryScope, ReviewFindingStatus,
};

/// Parameters specifying the target task boundary for memory retrieval.
#[derive(Debug, Clone, Default)]
pub struct MemoryRetrievalCriteria<'a> {
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub task_objective: &'a str,
    pub target_files: &'a [String],
    pub target_symbols: &'a [String],
    pub requirement_keys: &'a [String],
    pub error_context: Option<&'a str>,
}

/// Deterministic, task-aware memory retriever.
pub struct TaskAwareMemoryRetriever<'a, S: ?Sized + EngineeringMemoryStore> {
    store: &'a S,
}

impl<'a, S: ?Sized + EngineeringMemoryStore> TaskAwareMemoryRetriever<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }

    /// Retrieve a consolidated snapshot of only task-relevant engineering knowledge.
    pub async fn retrieve_snapshot(
        &self,
        criteria: &MemoryRetrievalCriteria<'a>,
    ) -> Result<EngineeringMemorySnapshot, sqlx::Error> {
        let file_set: HashSet<String> = criteria
            .target_files
            .iter()
            .map(|f| f.trim_start_matches("./").to_string())
            .collect();
        let sym_set: HashSet<String> = criteria.target_symbols.iter().cloned().collect();
        let req_set: HashSet<String> = criteria.requirement_keys.iter().cloned().collect();

        // 1. Retrieve & Filter Architectural Decisions
        let all_decisions = self
            .store
            .list_decisions(None, Some(criteria.mission_id), None)
            .await?;

        let mut relevant_decisions = Vec::new();
        for dec in all_decisions {
            // Must be authoritative (Accepted) or currently Under Operator Decision
            if !matches!(
                dec.status,
                DecisionStatus::Accepted | DecisionStatus::NeedsOperatorDecision
            ) {
                continue;
            }

            let mut is_match = false;
            // Match against linked files
            for lf in &dec.linked_files {
                let clean_lf = lf.trim_start_matches("./");
                if file_set.contains(clean_lf) || file_set.iter().any(|f| f.ends_with(clean_lf)) {
                    is_match = true;
                    break;
                }
            }

            // Match against linked symbols
            if !is_match {
                for ls in &dec.linked_symbols {
                    if sym_set.contains(ls) {
                        is_match = true;
                        break;
                    }
                }
            }

            // Match against linked requirements
            if !is_match {
                for lr in &dec.linked_requirements {
                    if req_set.contains(lr) {
                        is_match = true;
                        break;
                    }
                }
            }

            // Match project-level decisions if unconstrained or keyword overlap
            if !is_match && dec.scope == MemoryScope::Project {
                if dec.linked_files.is_empty() && dec.linked_requirements.is_empty() {
                    is_match = true;
                } else {
                    let dec_title_lower = dec.title.to_lowercase();
                    let obj_lower = criteria.task_objective.to_lowercase();
                    for word in dec_title_lower.split_whitespace() {
                        let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
                        if clean.len() >= 5 && obj_lower.contains(clean) {
                            is_match = true;
                            break;
                        }
                    }
                }
            }

            if is_match {
                relevant_decisions.push(dec);
            }
        }

        // 2. Retrieve & Filter Assumptions
        let all_assumptions = self
            .store
            .list_assumptions(criteria.mission_id, None, None)
            .await?;

        let mut relevant_assumptions = Vec::new();
        for asm in all_assumptions {
            // Only active or confirmed assumptions
            if !matches!(
                asm.status,
                AssumptionStatus::Active | AssumptionStatus::Confirmed
            ) {
                continue;
            }

            let mut is_match = false;
            if let Some(ref tf) = asm.target_file {
                let clean_tf = tf.trim_start_matches("./");
                if file_set.contains(clean_tf) || file_set.iter().any(|f| f.ends_with(clean_tf)) {
                    is_match = true;
                }
            }

            if !is_match
                && asm
                    .target_symbol
                    .as_ref()
                    .is_some_and(|ts| sym_set.contains(ts))
            {
                is_match = true;
            }

            if !is_match
                && let Some(tid) = criteria.task_id
                && asm.task_id == Some(tid)
            {
                is_match = true;
            }

            if is_match {
                relevant_assumptions.push(asm);
            }
        }

        // 3. Retrieve & Filter Failure Diagnoses
        let all_diagnoses = self
            .store
            .list_diagnoses(criteria.mission_id, criteria.task_id)
            .await?;

        let mut relevant_failures = Vec::new();
        if let Some(err_ctx) = criteria.error_context {
            let err_lower = err_ctx.to_lowercase();
            for diag in all_diagnoses {
                if err_lower.contains(&diag.failure_signature.to_lowercase())
                    || diag.error_message.to_lowercase().contains(&err_lower)
                    || err_lower.contains(&diag.error_message.to_lowercase())
                {
                    relevant_failures.push(diag);
                }
            }
        } else {
            // Include recent failed repair attempts for this task so agent doesn't repeat them
            for diag in all_diagnoses {
                if diag.repair_status == crate::memory::types::RepairStatus::VerifiedFailure
                    || diag.recurrence_count > 0
                {
                    relevant_failures.push(diag);
                }
            }
        }

        // 4. Retrieve & Filter Open Review Findings
        let all_findings = self
            .store
            .list_review_findings(criteria.mission_id, None, Some(ReviewFindingStatus::Open))
            .await?;

        let mut open_findings = Vec::new();
        for finding in all_findings {
            let clean_path = finding.file_path.trim_start_matches("./");
            if file_set.contains(clean_path)
                || file_set.iter().any(|f| f.ends_with(clean_path))
                || (criteria.task_id.is_some() && finding.task_id == criteria.task_id.unwrap())
            {
                open_findings.push(finding);
            }
        }

        // 5. Retrieve Verification Records for Linked Requirements
        let mut verified_requirements = Vec::new();
        for req_key in criteria.requirement_keys {
            if let Some(rec) = self
                .store
                .find_latest_verification_for_requirement(criteria.mission_id, req_key)
                .await?
            {
                verified_requirements.push(rec);
            }
        }

        Ok(EngineeringMemorySnapshot {
            active_decisions: relevant_decisions,
            active_assumptions: relevant_assumptions,
            open_findings,
            recent_failures: relevant_failures,
            verified_requirements,
        })
    }
}
