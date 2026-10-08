//! Evidence-Backed Completion Gate with Cryptographic Snapshot Binding (VER-04, VER-05, D-04, Law 6).
//!
//! Enforces Law 6 ("The model proposes. The runtime decides."):
//! No task or mission may transition to Succeeded on model self-report alone.
//! The runtime independently queries SQLite verification_checks and requirement_check_coverage,
//! verifies external artifact existence and SHA-256 hashes in FsArtifactStore,
//! confirms that the workspace snapshot matches the verified baseline,
//! and persists authoritative completion gate decisions.
//!
//! Canonical `ToolCapabilityClass` replaces scattered literal tool-name
//! matching with capability-based classification.

use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::sync::Arc;

use crate::ids::{ArtifactId, CheckId, MissionId, RequirementId, SessionId, TaskId};
use crate::kernel::seams::verifier::{
    CompletionGateOutcome, TaskVerificationRequest, VerificationEngine, VerificationError,
    VerificationOutcome,
};
use crate::persistence::artifacts::ArtifactStore;
use crate::repo::drift::RepositoryBaseline;
use crate::verification::types::{
    CheckStatus, CheckTier, CompletionGateDecision, VerificationCheck,
};

/// Canonical capability classification for tool names used in completion gate evaluation.
///
/// Replaces scattered `if tool_name == "write_file"` checks with a single authoritative
/// classification function. Adding a new tool that has mutation semantics only requires
/// updating `classify_tool` — not hunting through multiple match arms across the gate.
///
/// Canonical capability classification (VER-04): evidence layer must not depend
/// on arbitrary literal tool names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCapabilityClass {
    /// Tool that mutates workspace files (triggers verification requirement).
    WorkspaceMutation,
    /// Tool that runs tests or static analysis to verify correctness.
    VerificationRun,
    /// Tool that reads files without modification.
    RepositoryRead,
    /// Tool that executes an external process.
    ProcessExecution,
    /// Tool that performs research/information retrieval.
    Research,
    /// Tool that creates or writes artifacts.
    ArtifactCreation,
    /// Tool with unknown or uncategorized capability.
    Uncategorized,
}

impl ToolCapabilityClass {
    /// Classify a tool name into its canonical capability class.
    ///
    /// This is the single authoritative location for tool capability classification
    /// in the verification layer. All gate evaluations MUST use this function rather
    /// than inline string matching.
    pub fn classify(tool_name: &str) -> Self {
        let lower = tool_name.to_lowercase();
        let lower = lower.trim();

        // Workspace mutation: file writes, edits, patches
        if matches!(
            lower,
            "edit_file"
                | "write_file"
                | "apply_patch"
                | "fs.write"
                | "fs_write"
                | "workspace_fs_write"
                | "create_file"
                | "delete_file"
                | "move_file"
                | "rename_file"
                | "create_directory"
                | "copy_file"
                | "apply_workspace_patch"
        ) {
            return Self::WorkspaceMutation;
        }

        // Verification: test runners and static analysis
        if matches!(
            lower,
            "run_tests"
                | "run_linter"
                | "cargo.test"
                | "cargo.check"
                | "cargo.clippy"
                | "test_runner"
                | "run_checks"
                | "verify"
                | "lint"
        ) {
            return Self::VerificationRun;
        }

        // Repository read tools
        if matches!(
            lower,
            "read_file"
                | "glob"
                | "grep"
                | "repo_search"
                | "repo_symbols"
                | "lsp_goto_definition"
                | "lsp_find_references"
                | "lsp_hover"
                | "lsp_symbols"
                | "list_dir"
                | "ls"
                | "find"
                | "git_log"
                | "git_diff"
                | "git_status"
        ) {
            return Self::RepositoryRead;
        }

        // Research tools
        if matches!(
            lower,
            "web_search" | "web_fetch" | "fetch_url" | "search_docs" | "research"
        ) {
            return Self::Research;
        }

        // Artifact creation
        if matches!(
            lower,
            "create_artifact" | "write_artifact" | "store_artifact"
        ) {
            return Self::ArtifactCreation;
        }

        // Process execution
        if matches!(lower, "run_command" | "execute" | "shell" | "bash") {
            return Self::ProcessExecution;
        }

        Self::Uncategorized
    }

    /// Whether this class represents a workspace mutation (triggers verification requirement).
    pub fn is_mutation(&self) -> bool {
        matches!(self, Self::WorkspaceMutation)
    }

    /// Whether this class represents a verification run.
    pub fn is_verification(&self) -> bool {
        matches!(self, Self::VerificationRun)
    }
}

pub struct EvidenceCompletionGate {
    pub pool: SqlitePool,
    pub artifact_store: Arc<dyn ArtifactStore>,
    pub workspace_root: PathBuf,
    pub hierarchy_engine: Option<Arc<crate::verification::hierarchy::VerificationHierarchyEngine>>,
}

impl EvidenceCompletionGate {
    pub fn new(
        pool: SqlitePool,
        artifact_store: Arc<dyn ArtifactStore>,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        let ws = workspace_root.into();
        let hierarchy = Arc::new(
            crate::verification::hierarchy::VerificationHierarchyEngine::for_workspace(
                &ws, None, None,
            ),
        );
        Self {
            pool,
            artifact_store,
            workspace_root: ws,
            hierarchy_engine: Some(hierarchy),
        }
    }

    pub fn with_hierarchy_engine(
        mut self,
        hierarchy: Arc<crate::verification::hierarchy::VerificationHierarchyEngine>,
    ) -> Self {
        self.hierarchy_engine = Some(hierarchy);
        self
    }

    /// Evaluates whether a task has satisfied all verification and evidence requirements (VER-05, D-04).
    pub async fn evaluate_task_completion(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        current_snapshot_hash: &str,
    ) -> Result<CompletionGateDecision, String> {
        let mut violations = Vec::new();

        // 1. Query all verification checks bound to this task
        let check_rows = sqlx::query(
            r#"
            SELECT id, tier, status, summary, failure_class, snapshot_hash, evidence_artifact_id
            FROM verification_checks
            WHERE task_id = ?
            "#,
        )
        .bind(task_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to query verification_checks: {}", e))?;

        if check_rows.is_empty() {
            violations
                .push("No verification check records found for task (Law 6 violation)".to_string());
        }

        let mut has_passed_check = false;
        let mut current_rows = 0;

        for check in &check_rows {
            let status_str: String = check.get("status");
            let summary_str: String = check.get("summary");
            let snapshot_str: String = check.get("snapshot_hash");
            // Only rows from the current verification run decide
            // pass/fail. Rows recorded under a different snapshot belong to
            // superseded attempts; they remain queryable for audit but must
            // not re-fail the current attempt (recovery would be futile).
            if snapshot_str != current_snapshot_hash {
                continue;
            }
            current_rows += 1;
            let check_id_bytes: Vec<u8> = check.get("id");
            let check_id_arr: [u8; 16] = check_id_bytes
                .as_slice()
                .try_into()
                .map_err(|_| "Corrupted check_id in DB".to_string())?;
            let check_id = CheckId::from_bytes(check_id_arr);

            // 2. Unresolved failures or blocks fail closed immediately.
            // Only current-snapshot rows are considered (stale rows were
            // skipped above); a failed check here is fresh evidence.
            if status_str == "failed" {
                violations.push(format!(
                    "Unresolved check failure in check {}: {}",
                    check_id, summary_str
                ));
            } else if status_str == "blocked" {
                violations.push(format!(
                    "Check {} remained blocked: {}",
                    check_id, summary_str
                ));
            } else if status_str == "passed" {
                has_passed_check = true;
            }

            // 4. If an external artifact was referenced, confirm it exists in ArtifactStore
            let evidence_art_opt: Option<Vec<u8>> = check.get("evidence_artifact_id");
            if let Some(ref art_bytes) = evidence_art_opt {
                let art_arr: [u8; 16] = art_bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| "Corrupted artifact_id in DB".to_string())?;
                let art_id = ArtifactId::from_bytes(art_arr);

                // Check retrieval with standard extensions
                let exists = self.artifact_store.retrieve(art_id, "log").await.is_ok()
                    || self.artifact_store.retrieve(art_id, "txt").await.is_ok()
                    || self.artifact_store.retrieve(art_id, "bin").await.is_ok();

                if !exists {
                    violations.push(format!(
                        "Referenced verification artifact {} does not exist in artifact store",
                        art_id
                    ));
                }
            }
        }

        if current_rows == 0 {
            violations.push(
                "No current verification checks recorded for task (Law 6 violation)".to_string(),
            );
        } else if !has_passed_check {
            violations.push("No checks passed for this task".to_string());
        }

        // 5. Query requirement-to-check coverage bindings, scoped to the
        // current snapshot so superseded attempts cannot fail the gate.
        let coverage_rows = sqlx::query(
            r#"
            SELECT rc.requirement_id, rc.is_mandatory, vc.status
            FROM requirement_check_coverage rc
            JOIN verification_checks vc ON rc.check_id = vc.id
            WHERE vc.task_id = ? AND vc.snapshot_hash = ?
            "#,
        )
        .bind(task_id.as_bytes().as_slice())
        .bind(current_snapshot_hash)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to query requirement_check_coverage: {}", e))?;

        for cov in coverage_rows {
            let is_mand: i64 = cov.get("is_mandatory");
            let status: String = cov.get("status");
            if is_mand != 0 && status != "passed" {
                let req_bytes: Vec<u8> = cov.get("requirement_id");
                let req_arr: [u8; 16] = req_bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| "Corrupted requirement_id in DB".to_string())?;
                let req_id = RequirementId::from_bytes(req_arr);
                violations.push(format!(
                    "Mandatory requirement {} does not have a passed verification check",
                    req_id
                ));
            }
        }

        let is_satisfied = violations.is_empty();
        let evidence_summary = if is_satisfied {
            format!(
                "Verified {} check(s) successfully matching snapshot {}",
                check_rows.len(),
                current_snapshot_hash
            )
        } else {
            format!(
                "Completion gate deficient: {} violation(s)",
                violations.len()
            )
        };

        let decision = if is_satisfied {
            CompletionGateDecision::satisfied(
                mission_id,
                Some(task_id),
                "task",
                current_snapshot_hash,
                evidence_summary,
            )
        } else {
            CompletionGateDecision::deficient(
                mission_id,
                Some(task_id),
                "task",
                current_snapshot_hash,
                violations.clone(),
                evidence_summary,
            )
        };

        // 6. Authoritative persistence in completion_gate_decisions
        let violations_json = serde_json::to_string(&decision.violations)
            .map_err(|e| format!("Serialization error: {}", e))?;
        let decision_str = if decision.is_satisfied {
            "satisfied"
        } else {
            "deficient"
        };

        sqlx::query(
            r#"
            INSERT INTO completion_gate_decisions (
                id, mission_id, task_id, gate_scope, decision,
                snapshot_hash, violations_json, evidence_summary, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(decision.decision_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(task_id.as_bytes().as_slice())
        .bind(&decision.gate_scope)
        .bind(decision_str)
        .bind(&decision.snapshot_hash)
        .bind(&violations_json)
        .bind(&decision.evidence_summary)
        .bind(decision.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to record completion_gate_decision: {}", e))?;

        Ok(decision)
    }

    /// Evaluates whether an interactive session's turns satisfy evidence completion gate requirements.
    ///
    /// Authority classification (D3): NON-AUTHORITATIVE advisory evaluation. This method
    /// builds a `CompletionGateDecision` value in memory and does NOT
    /// persist it to `completion_gate_decisions` and does NOT complete any
    /// mission or task. Session completion state alone can never complete a
    /// mission; only `evaluate_task_completion` / `evaluate_mission_completion`
    /// (persisted through the evidence gate) decide durable completion.
    pub async fn evaluate_session_completion(
        &self,
        session_id: SessionId,
        turns: &[crate::interaction::session::ConversationTurn],
    ) -> Result<CompletionGateDecision, String> {
        self.evaluate_session_completion_with_intent(session_id, turns, None)
            .await
    }

    /// Evaluates whether an interactive session's turns and active intent state satisfy evidence completion requirements.
    pub async fn evaluate_session_completion_with_intent(
        &self,
        session_id: SessionId,
        turns: &[crate::interaction::session::ConversationTurn],
        intent_state: Option<&crate::agent::intent::IntentState>,
    ) -> Result<CompletionGateDecision, String> {
        use crate::agent::intent::FormedTaskStatus;
        use crate::interaction::session::ConversationTurn;
        use crate::planning::risks::UnknownFate;

        let mut violations = Vec::new();
        let mut mutated_files = Vec::new();

        // 1. Scan turns for workspace mutation tools using canonical ToolCapabilityClass
        let mut has_mutated = false;
        let mut last_mutation_turn_idx = None;

        for (idx, turn) in turns.iter().enumerate() {
            match turn {
                ConversationTurn::ToolCallMessage {
                    tool_name,
                    arguments,
                    ..
                } => {
                    if ToolCapabilityClass::classify(tool_name).is_mutation() {
                        has_mutated = true;
                        last_mutation_turn_idx = Some(idx);
                        if let Some(path) = arguments.get("path").and_then(|p| p.as_str()) {
                            let clean = path
                                .trim()
                                .trim_start_matches('@')
                                .trim_start_matches("./")
                                .to_string();
                            if !mutated_files.contains(&clean) {
                                mutated_files.push(clean);
                            }
                        }
                    }
                }
                ConversationTurn::ToolResultMessage {
                    tool_name,
                    success: true,
                    ..
                } if ToolCapabilityClass::classify(tool_name).is_mutation() => {
                    has_mutated = true;
                    last_mutation_turn_idx = Some(idx);
                }
                ConversationTurn::ToolResultMessage { .. } => {}
                _ => {}
            }
        }

        // 2. If code mutations occurred, verify that verification tests ran and passed after the mutation
        if has_mutated {
            let mut has_verified_after_mutation = false;
            let start_scan = last_mutation_turn_idx.unwrap_or(0);

            for turn in &turns[start_scan..] {
                if let ConversationTurn::ToolResultMessage {
                    tool_name,
                    output,
                    success,
                    ..
                } = turn
                {
                    // Use canonical ToolCapabilityClass instead of literal tool names
                    if *success
                        && ToolCapabilityClass::classify(tool_name).is_verification()
                        && !output.contains("Failed tests:")
                        && !output.contains("Compilation failed")
                        && !output.contains("Compiler errors detected")
                        && !output.contains("test result: FAILED")
                    {
                        has_verified_after_mutation = true;
                        break;
                    }
                }
            }

            if !has_verified_after_mutation {
                violations.push(
                    "Premature completion rejected (AGENTS.md Rule 6: Completion requires evidence): \
                     Code changes were made, but verification tests have not passed after the last modification. \
                     You must run 'run_tests' and ensure all tests pass before completing.".to_string(),
                );
            }

            // 3. Anti-fake diff review on mutated files
            if !mutated_files.is_empty() {
                let surface = crate::kernel::change::ChangeSurface::new(mutated_files.clone());
                let mut synthesized_diff = String::new();
                for file in &mutated_files {
                    let full = self.workspace_root.join(file);
                    if let Ok(bytes) = std::fs::read(&full) {
                        let file_diff =
                            crate::change::observer::PostMutationObserver::generate_file_unified_diff(
                                file,
                                None,
                                Some(&bytes),
                            );
                        synthesized_diff.push_str(&file_diff);
                    }
                }

                let review = crate::change::review::DiffReviewer::review(
                    &synthesized_diff,
                    &surface,
                    &mutated_files,
                );
                if !review.passed {
                    violations.push(format!(
                        "Premature completion rejected (AGENTS.md Rule 5: Never fake success): \
                         Diff review detected quality violations in changes: {:?}",
                        review.violations
                    ));
                }
            }
        }

        // 4. Intent state validation
        if let Some(intent) = intent_state {
            // Check for unresolved blocking unknowns
            let blocking_unknowns: Vec<_> = intent
                .unknowns
                .iter()
                .filter(|u| !u.is_resolved() && u.fate == UnknownFate::Blocking)
                .collect();
            if !blocking_unknowns.is_empty() {
                violations.push(format!(
                    "Unresolved blocking unknown(s) prevent completion: {}",
                    blocking_unknowns
                        .iter()
                        .map(|u| format!("[{}] {}", u.id, u.description))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }

            // Check for unresolved consequential decisions
            let pending_decisions = intent.pending_decisions();
            if !pending_decisions.is_empty() {
                violations.push(format!(
                    "Unresolved consequential decision(s) prevent completion: {}",
                    pending_decisions
                        .iter()
                        .map(|d| format!("[{}] {}", d.id, d.question))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }

            // Check formed tasks:
            // Blocked tasks prevent completion
            let blocked_tasks: Vec<_> = intent
                .formed_tasks
                .iter()
                .filter(|t| matches!(t.status, FormedTaskStatus::Blocked { .. }))
                .collect();
            if !blocked_tasks.is_empty() {
                violations.push(format!(
                    "Blocked task(s) prevent completion: {}",
                    blocked_tasks
                        .iter()
                        .map(|t| format!("[{}] {}", t.id, t.title))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }

            // In-progress or pending tasks (that are not superseded or completed) prevent completion
            let incomplete_tasks: Vec<_> = intent
                .formed_tasks
                .iter()
                .filter(|t| {
                    matches!(
                        t.status,
                        FormedTaskStatus::Pending | FormedTaskStatus::InProgress
                    )
                })
                .collect();
            if !incomplete_tasks.is_empty() {
                violations.push(format!(
                    "Incomplete task(s) prevent completion: {}",
                    incomplete_tasks
                        .iter()
                        .map(|t| format!("[{}] {}", t.id, t.title))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }

            // Failed tasks that have NOT been superseded prevent completion
            let unaddressed_failed_tasks: Vec<_> = intent
                .formed_tasks
                .iter()
                .filter(|t| matches!(t.status, FormedTaskStatus::Failed { .. }))
                .collect();
            if !unaddressed_failed_tasks.is_empty() {
                violations.push(format!(
                    "Unaddressed failed task(s) prevent completion: {}",
                    unaddressed_failed_tasks
                        .iter()
                        .map(|t| format!("[{}] {}", t.id, t.title))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }

            // Note: Superseded tasks (FormedTaskStatus::Superseded) do NOT block completion!
        }

        let is_satisfied = violations.is_empty();
        let evidence_summary = if is_satisfied {
            format!(
                "Session {} completion gate passed with verified evidence (mutated_files={})",
                session_id,
                mutated_files.len()
            )
        } else {
            format!(
                "Session {} completion gate deficient: {} violation(s)",
                session_id,
                violations.len()
            )
        };

        let snapshot_hash = "session_interactive";
        let decision = if is_satisfied {
            CompletionGateDecision::satisfied(
                MissionId::new(),
                None,
                "session",
                snapshot_hash,
                evidence_summary,
            )
        } else {
            CompletionGateDecision::deficient(
                MissionId::new(),
                None,
                "session",
                snapshot_hash,
                violations,
                evidence_summary,
            )
        };

        Ok(decision)
    }

    /// Evaluates mission-wide completion gate criteria.
    pub async fn evaluate_mission_completion(
        &self,
        mission_id: MissionId,
        current_snapshot_hash: &str,
    ) -> Result<CompletionGateDecision, String> {
        let mut violations = Vec::new();

        // 1. Ensure all tasks in the active task graph for this mission are completed and verified
        let task_rows = sqlx::query(
            r#"
            SELECT t.id, t.status, t.role 
            FROM tasks t
            WHERE t.mission_id = ?
              AND (
                t.task_graph_id = (
                    SELECT id FROM task_graphs 
                    WHERE mission_id = ? AND status = 'active' 
                    ORDER BY revision DESC LIMIT 1
                )
                OR NOT EXISTS (SELECT 1 FROM task_graphs WHERE mission_id = ?)
              )
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to query tasks: {}", e))?;

        if task_rows.is_empty() {
            violations.push("Mission contains no tasks".to_string());
        }

        for t in &task_rows {
            let status_str: String = t.get("status");
            let norm = status_str.to_lowercase();
            if norm != "completed" && norm != "succeeded" {
                violations.push(format!(
                    "Task is not in completed state: status={}",
                    status_str
                ));
            }
        }

        let all_read_only = !task_rows.is_empty()
            && task_rows.iter().all(|t| {
                let role_str: String = t.get("role");
                let role = crate::state_machine::agent::AgentRole::new(&role_str);
                crate::agent::registry::RoleRegistry::global()
                    .read()
                    .ok()
                    .and_then(|guard| guard.read_only_flag(&role))
                    .unwrap_or_else(|| {
                        role_str == "researcher"
                            || role_str == "diagnostician"
                            || role_str == "synthesizer"
                    })
            });

        let adapter =
            crate::verification::adapter::ProjectAdapter::detect(&self.workspace_root, None, None);
        // An empty manifest filename (undetected toolchain) means there is
        // no suite to gate on — never treat the workspace root itself as a
        // manifest (target-neutral verification).
        let has_manifest = !adapter.manifest_file.is_empty()
            && self.workspace_root.join(&adapter.manifest_file).exists();

        // 2. Empirical Verification Gate (Law 6): Workspace tests must pass
        if violations.is_empty() && !all_read_only && has_manifest {
            let hierarchy = self.hierarchy_engine.clone().unwrap_or_else(|| {
                Arc::new(
                    crate::verification::hierarchy::VerificationHierarchyEngine::for_workspace(
                        &self.workspace_root,
                        None,
                        None,
                    ),
                )
            });
            let test_res = hierarchy
                .tier3_tests
                .execute(
                    mission_id,
                    TaskId::default(),
                    &self.workspace_root,
                    current_snapshot_hash,
                )
                .await;
            match test_res {
                Ok(check) if check.status != crate::verification::types::CheckStatus::Passed => {
                    violations.push(format!(
                        "Mission verification gate failed: test suite rejected with status '{}': {}",
                        check.status.as_str(),
                        check.summary
                    ));
                }
                Err(e) => {
                    violations.push(format!(
                        "Mission verification gate failed: test runner error: {}",
                        e
                    ));
                }
                _ => {}
            }
        }

        let is_satisfied = violations.is_empty();
        let evidence_summary = if is_satisfied {
            format!(
                "Mission completion verified across {} task(s)",
                task_rows.len()
            )
        } else {
            format!(
                "Mission completion deficient: {} violation(s)",
                violations.len()
            )
        };

        let decision = if is_satisfied {
            CompletionGateDecision::satisfied(
                mission_id,
                None,
                "mission",
                current_snapshot_hash,
                evidence_summary,
            )
        } else {
            CompletionGateDecision::deficient(
                mission_id,
                None,
                "mission",
                current_snapshot_hash,
                violations,
                evidence_summary,
            )
        };

        let violations_json = serde_json::to_string(&decision.violations)
            .map_err(|e| format!("Serialization error: {}", e))?;
        let decision_str = if decision.is_satisfied {
            "satisfied"
        } else {
            "deficient"
        };

        let task_id_opt: Option<&[u8]> = None;
        sqlx::query(
            r#"
            INSERT INTO completion_gate_decisions (
                id, mission_id, task_id, gate_scope, decision,
                snapshot_hash, violations_json, evidence_summary, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(decision.decision_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(task_id_opt)
        .bind(&decision.gate_scope)
        .bind(decision_str)
        .bind(&decision.snapshot_hash)
        .bind(&violations_json)
        .bind(&decision.evidence_summary)
        .bind(decision.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to record mission completion_gate_decision: {}", e))?;

        Ok(decision)
    }

    /// Capture current workspace snapshot hash using RepositoryBaseline.
    pub fn capture_current_snapshot_hash(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
    ) -> Result<String, String> {
        let baseline = RepositoryBaseline::capture(&self.workspace_root, mission_id, task_id, None)
            .map_err(|e| format!("Failed to capture workspace baseline: {}", e))?;

        let mut combined = String::new();
        for (rel, hash) in &baseline.file_hashes {
            combined.push_str(rel);
            combined.push(':');
            combined.push_str(hash);
            combined.push(';');
        }

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Lists all verification check IDs for a given mission.
    pub async fn list_check_ids_for_mission(
        pool: &SqlitePool,
        mission_id: MissionId,
    ) -> Result<Vec<CheckId>, sqlx::Error> {
        let rows = sqlx::query("SELECT id FROM verification_checks WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_all(pool)
            .await?;

        let mut check_ids = Vec::with_capacity(rows.len());
        for row in rows {
            let id_bytes: Vec<u8> = row.get("id");
            if id_bytes.len() == 16 {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&id_bytes);
                check_ids.push(CheckId::from_bytes(arr));
            }
        }
        Ok(check_ids)
    }

    /// Retrieve all verification checks recorded across missions and tasks (canonical query).
    pub async fn list_all_checks(&self) -> Result<Vec<VerificationCheck>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized, evidence_artifact_id, summary, failure_class, snapshot_hash, created_at
            FROM verification_checks
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Self::map_check_rows(rows)
    }

    /// Retrieve recent verification checks up to specified limit.
    pub async fn list_recent_checks(
        &self,
        limit: usize,
    ) -> Result<Vec<VerificationCheck>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized, evidence_artifact_id, summary, failure_class, snapshot_hash, created_at
            FROM verification_checks
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Self::map_check_rows(rows)
    }

    /// Internal helper to deserialize verification check rows from SQLite.
    pub fn map_check_rows(
        rows: Vec<sqlx::sqlite::SqliteRow>,
    ) -> Result<Vec<VerificationCheck>, sqlx::Error> {
        use chrono::{DateTime, Utc};
        use std::str::FromStr;

        let mut checks = Vec::with_capacity(rows.len());

        for row in rows {
            let id_bytes: Vec<u8> = row.try_get("id")?;
            let m_bytes: Vec<u8> = row.try_get("mission_id")?;
            let t_bytes: Vec<u8> = row.try_get("task_id")?;
            let tier_int: i64 = row.try_get("tier")?;
            let status_str: String = row.try_get("status")?;
            let command_or_tool: String = row.try_get("command_or_tool")?;
            let inputs_normalized: String = row.try_get("inputs_normalized")?;
            let art_bytes: Option<Vec<u8>> = row.try_get("evidence_artifact_id")?;
            let summary: String = row.try_get("summary")?;
            let failure_class: Option<String> = row.try_get("failure_class")?;
            let snapshot_hash: String = row.try_get("snapshot_hash")?;
            let created_at_str: String = row.try_get("created_at")?;

            let check_id = if id_bytes.len() == 16 {
                let mut b = [0u8; 16];
                b.copy_from_slice(&id_bytes);
                CheckId::from_bytes(b)
            } else {
                return Err(sqlx::Error::Protocol(format!(
                    "corrupt verification check identity: expected 16 bytes, got {}",
                    id_bytes.len()
                )));
            };

            let mission_id = if m_bytes.len() == 16 {
                let mut b = [0u8; 16];
                b.copy_from_slice(&m_bytes);
                MissionId::from_bytes(b)
            } else {
                return Err(sqlx::Error::Protocol(format!(
                    "corrupt verification check mission identity: expected 16 bytes, got {}",
                    m_bytes.len()
                )));
            };

            let task_id = if t_bytes.len() == 16 {
                let mut b = [0u8; 16];
                b.copy_from_slice(&t_bytes);
                TaskId::from_bytes(b)
            } else {
                return Err(sqlx::Error::Protocol(format!(
                    "corrupt verification check task identity: expected 16 bytes, got {}",
                    t_bytes.len()
                )));
            };

            let tier = CheckTier::from_u8(tier_int as u8).unwrap_or(CheckTier::Deterministic);
            // Fail closed on persistence corruption (D7): malformed persisted
            // statuses NEVER become a legitimate semantic state. A corrupt `status`
            // value is a persistence-integrity failure: surface it as a typed
            // `Protocol` error instead of silently coercing to `NotRun`. Completion
            // circuits treat load errors as incomplete, so corruption can never
            // read as false success or false legitimate-not-run. The TUI renders
            // unrecognized strings as UNRECOGNIZED at the projection layer without
            // touching this.
            let status = CheckStatus::from_str(&status_str).map_err(|e| {
                sqlx::Error::Protocol(format!(
                    "corrupt verification_checks.status value '{status_str}': {e}"
                ))
            })?;

            let evidence_artifact_id = art_bytes.and_then(|ab| {
                if ab.len() == 16 {
                    let mut b = [0u8; 16];
                    b.copy_from_slice(&ab);
                    Some(ArtifactId::from_bytes(b))
                } else {
                    None
                }
            });

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            checks.push(VerificationCheck {
                check_id,
                mission_id,
                task_id,
                tier,
                status,
                command_or_tool,
                inputs_normalized,
                evidence_artifact_id,
                summary,
                failure_class,
                snapshot_hash,
                created_at,
            });
        }

        Ok(checks)
    }
}

#[async_trait]
impl VerificationEngine for EvidenceCompletionGate {
    async fn verify_task(
        &self,
        req: TaskVerificationRequest,
    ) -> Result<VerificationOutcome, VerificationError> {
        let snapshot_hash = self
            .capture_current_snapshot_hash(req.mission_id, Some(req.task_id))
            .unwrap_or_else(|_| "unknown_snapshot".to_string());

        // Every Verify stage executes a FRESH hierarchy run.
        // A previous attempt's rows must never decide the current attempt:
        // re-scoring stale failure rows made recovery futile (every retry
        // re-failed on the prior attempt's evidence). Fresh checks are
        // recorded alongside stale rows (audit trail preserved); evaluation
        // below scopes pass/fail to the current snapshot.
        let (role_str, verif_str) = if let Ok(Some(row)) =
            sqlx::query("SELECT role, verification FROM tasks WHERE id = ?")
                .bind(req.task_id.as_bytes().as_slice())
                .fetch_optional(&self.pool)
                .await
        {
            let r: String = row.get("role");
            let v: String = row.get("verification");
            (r, v)
        } else {
            ("implementer".to_string(), "{}".to_string())
        };

        let hierarchy = self.hierarchy_engine.clone().unwrap_or_else(|| {
            Arc::new(
                crate::verification::hierarchy::VerificationHierarchyEngine::for_workspace(
                    &self.workspace_root,
                    None,
                    None,
                ),
            )
        });
        let checks = hierarchy
            .execute_hierarchy_for_task(
                req.mission_id,
                req.task_id,
                &self.workspace_root,
                &snapshot_hash,
                &role_str,
                &verif_str,
            )
            .await;

        for c in checks {
            let null_blob: Option<&[u8]> = None;
            let art_blob = c.evidence_artifact_id.map(|id| id.as_bytes().to_vec());
            let _ = sqlx::query(
                    r#"
                    INSERT INTO verification_checks (
                        id, mission_id, task_id, tier, status, command_or_tool,
                        inputs_normalized, evidence_artifact_id, summary, failure_class, snapshot_hash, created_at
                    )
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(c.check_id.as_bytes().as_slice())
                .bind(c.mission_id.as_bytes().as_slice())
                .bind(c.task_id.as_bytes().as_slice())
                .bind(c.tier.as_u8() as i64)
                .bind(c.status.as_str())
                .bind(&c.command_or_tool)
                .bind(&c.inputs_normalized)
                .bind(art_blob.as_deref().or(null_blob))
                .bind(&c.summary)
                .bind(c.failure_class.as_deref())
                .bind(&c.snapshot_hash)
                .bind(c.created_at.to_rfc3339())
                .execute(&self.pool)
                .await;
        }

        let decision = self
            .evaluate_task_completion(req.mission_id, req.task_id, &snapshot_hash)
            .await
            .map_err(VerificationError::Failed)?;

        if decision.is_satisfied {
            Ok(VerificationOutcome::Passed)
        } else {
            Ok(VerificationOutcome::Failed {
                reason: decision.violations.join("; "),
            })
        }
    }

    async fn verify_completion_gate(
        &self,
        mission_id: MissionId,
    ) -> Result<CompletionGateOutcome, VerificationError> {
        let snapshot_hash = self
            .capture_current_snapshot_hash(mission_id, None)
            .unwrap_or_else(|_| "unknown_snapshot".to_string());

        let decision = self
            .evaluate_mission_completion(mission_id, &snapshot_hash)
            .await
            .map_err(VerificationError::Failed)?;

        if decision.is_satisfied {
            Ok(CompletionGateOutcome::Satisfied)
        } else {
            Ok(CompletionGateOutcome::Deficient {
                violations: decision.violations,
            })
        }
    }
}
