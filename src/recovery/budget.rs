//! Layered Recovery Budget Tracker & Durable Audit Store (FLC-02, FLC-05, D-06).
//!
//! Enforces class-specific retry caps layered strictly under global task and mission recovery ceilings.
//! Deterministically non-retryable classes (`Permission`, `Policy`) have zero retry budget.
//! Transient failures calculate bounded exponential backoff with jitter and honor `Retry-After`.
//! Every attempt is recorded in the SQLite `recovery_attempts` table prior to executing state transitions.

use chrono::Utc;
use sqlx::SqlitePool;
use std::time::Duration;
use uuid::Uuid;

use crate::ids::{MissionId, TaskId};
use crate::kernel::seams::recovery::FailureClassification;

/// Configuration parameters for exponential backoff with jitter (D-06).
#[derive(Debug, Clone, PartialEq)]
pub struct BackoffConfig {
    pub initial_ms: u64,
    pub multiplier: f64,
    pub max_delay_ms: u64,
    pub jitter_percent: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_ms: 500,
            multiplier: 2.0,
            max_delay_ms: 10_000,
            jitter_percent: 10.0,
        }
    }
}

/// Outcome of a recovery budget evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetEvaluation {
    Permitted {
        remaining_class_budget: usize,
        remaining_overall_budget: usize,
        backoff_delay: Duration,
    },
    NonRetryable {
        reason: String,
    },
    Exhausted {
        reason: String,
        consumed: usize,
        ceiling: usize,
    },
    RepeatedIdentical {
        reason: String,
    },
}

/// Classification of a recovery/retry attempt strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStrategyClassification {
    /// Attempt introduces a distinct strategy or repair patch not previously attempted.
    GenuinelyNew,
    /// Attempt modifies prior attempts (e.g. adjustments to files or parameters).
    ModifiedAttempt,
    /// Attempt is an identical repeat of a prior failed attempt.
    RepeatedIdentical,
}

/// Semantic signature of a failure state for detecting repeated/oscillating unrecoverable states across tasks and replan cycles.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SemanticFailureSignature {
    /// Failure classification (e.g. Compilation, Tests, StaticAnalysis, Dependency).
    pub failure_class: FailureClassification,
    /// Target domain or work item (e.g. affected files or component or task key).
    pub target_domain: String,
    /// Normalized error fingerprint (e.g. error code or normalized root cause).
    pub normalized_error: String,
    /// Candidate repair or diagnosis fingerprint if available.
    pub candidate_fingerprint: Option<String>,
}

impl SemanticFailureSignature {
    pub fn new(
        failure_class: FailureClassification,
        target_domain: impl Into<String>,
        normalized_error: impl Into<String>,
        candidate_fingerprint: Option<String>,
    ) -> Self {
        Self {
            failure_class,
            target_domain: target_domain.into(),
            normalized_error: normalized_error.into(),
            candidate_fingerprint,
        }
    }

    /// Evaluates semantic equivalence between two failure signatures.
    pub fn is_equivalent(&self, other: &Self) -> bool {
        if self.failure_class != other.failure_class {
            return false;
        }

        // If candidate repair proposal fingerprints match, it is an identical repair candidate failing repeatedly
        if let (Some(a), Some(b)) = (&self.candidate_fingerprint, &other.candidate_fingerprint) {
            if a == b {
                return true;
            }
        }

        let domain_matches = self.target_domain.is_empty()
            || other.target_domain.is_empty()
            || self.target_domain == other.target_domain;

        let error_matches = self.normalized_error == other.normalized_error
            || self.normalized_error.contains(&other.normalized_error)
            || other.normalized_error.contains(&self.normalized_error);

        domain_matches && error_matches
    }
}

/// Normalize an error string into a deterministic semantic fingerprint by removing volatile data (pointers, timestamps, UUIDs).
pub fn normalize_error_message(err: &str) -> String {
    let mut normalized = String::new();
    let first_line = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let tokens: Vec<&str> = first_line.split_whitespace().collect();
    for token in tokens {
        // Strip volatile tokens: hex pointers, uuids, timestamps
        if token.starts_with("0x")
            || (token.len() == 36 && token.contains('-'))
            || (token.len() >= 19 && token.contains('T') && token.contains(':'))
        {
            continue;
        }
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(&token.to_lowercase());
    }
    if normalized.is_empty() {
        normalized = "unspecified_error".to_string();
    }
    normalized
}

/// Extract target domain (e.g. file path or component) from error text or context.
pub fn extract_target_domain(err_msg: &str, affected_files: &[String]) -> String {
    if !affected_files.is_empty() {
        return affected_files.join(",");
    }
    // Look for Rust compiler/linter file pointers: "--> path/to/file.ext"
    for line in err_msg.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("-->") {
            let path_part = rest.trim().split(':').next().unwrap_or("").trim();
            if !path_part.is_empty() {
                return path_part.to_string();
            }
        }
        if let Some(idx) = trimmed.find("--> ") {
            let rest = &trimmed[idx + 4..];
            let path_part = rest.split(':').next().unwrap_or("").trim();
            if !path_part.is_empty() {
                return path_part.to_string();
            }
        }
    }
    String::new()
}

/// Computes a deterministic SHA-256 fingerprint of a change proposal's mutations.
pub fn compute_mutation_fingerprint(proposal: &crate::kernel::change::ChangeProposal) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for m in &proposal.mutations {
        hasher.update(m.path.as_bytes());
        match &m.operation {
            crate::kernel::change::FileMutationOp::Substring {
                old_content,
                new_content,
            } => {
                hasher.update(b":sub:");
                hasher.update(old_content.as_bytes());
                hasher.update(b"->");
                hasher.update(new_content.as_bytes());
            }
            crate::kernel::change::FileMutationOp::LineRange {
                start_line,
                end_line,
                new_content,
                expected_old,
            } => {
                hasher.update(b":lines:");
                hasher.update(start_line.to_string().as_bytes());
                hasher.update(b"-");
                hasher.update(end_line.to_string().as_bytes());
                hasher.update(new_content.as_bytes());
                if let Some(old) = expected_old {
                    hasher.update(old.as_bytes());
                }
            }
            crate::kernel::change::FileMutationOp::Insert {
                line_number,
                content,
                after,
            } => {
                hasher.update(b":ins:");
                hasher.update(line_number.to_string().as_bytes());
                hasher.update(if *after {
                    b":after:".as_slice()
                } else {
                    b":before:".as_slice()
                });
                hasher.update(content.as_bytes());
            }
            crate::kernel::change::FileMutationOp::Delete {
                start_line,
                end_line,
                expected_old,
            } => {
                hasher.update(b":del:");
                hasher.update(start_line.to_string().as_bytes());
                hasher.update(b"-");
                hasher.update(end_line.to_string().as_bytes());
                if let Some(old) = expected_old {
                    hasher.update(old.as_bytes());
                }
            }
            crate::kernel::change::FileMutationOp::Patch { patch } => {
                hasher.update(b":patch:");
                hasher.update(patch.as_bytes());
            }
            crate::kernel::change::FileMutationOp::CreateNew { content } => {
                hasher.update(b":create:");
                hasher.update(content.as_bytes());
            }
            crate::kernel::change::FileMutationOp::ReplaceFull { content, .. } => {
                hasher.update(b":replace:");
                hasher.update(content.as_bytes());
            }
        }
    }
    format!("{:x}", hasher.finalize())
}

/// Durable record to insert into the `recovery_attempts` SQLite table (FLC-05).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecoveryAttemptRecord {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub failure_class: FailureClassification,
    pub strategy: String,
    pub attempt_number: usize,
    pub budget_consumed: usize,
    pub remaining_class_budget: usize,
    pub remaining_overall_budget: usize,
    pub backoff_delay_ms: u64,
    pub action_taken: String,
    pub result: String,
    /// Durable mutation identity for cross-restart duplicate-attempt protection
    /// (hydrates the in-memory fingerprint maps).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutation_fingerprint: Option<String>,
    /// Durable semantic failure signature (JSON) for cross-restart circuit-breaker history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_signature: Option<String>,
}

/// Layered recovery budget tracker.
#[derive(Debug, Clone)]
pub struct RecoveryBudgetTracker {
    pub task_retry_ceiling: usize,
    pub mission_retry_ceiling: usize,
    pub circuit_breaker_threshold: usize,
    pub backoff_config: BackoffConfig,
    pub attempt_fingerprints:
        std::sync::Arc<std::sync::RwLock<std::collections::HashMap<TaskId, Vec<String>>>>,
    pub mission_fingerprints:
        std::sync::Arc<std::sync::RwLock<std::collections::HashMap<MissionId, Vec<String>>>>,
    pub failure_history: std::sync::Arc<
        std::sync::RwLock<std::collections::HashMap<MissionId, Vec<SemanticFailureSignature>>>,
    >,
}

impl Default for RecoveryBudgetTracker {
    fn default() -> Self {
        Self::new(3, 10)
    }
}

impl RecoveryBudgetTracker {
    pub fn new(task_retry_ceiling: usize, mission_retry_ceiling: usize) -> Self {
        Self {
            task_retry_ceiling,
            mission_retry_ceiling,
            circuit_breaker_threshold: 3,
            backoff_config: BackoffConfig::default(),
            attempt_fingerprints: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            mission_fingerprints: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            failure_history: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    pub fn with_backoff_config(mut self, config: BackoffConfig) -> Self {
        self.backoff_config = config;
        self
    }

    pub fn with_circuit_breaker_threshold(mut self, threshold: usize) -> Self {
        self.circuit_breaker_threshold = threshold;
        self
    }

    /// Classifies an attempt strategy for a task against prior attempted mutation fingerprints.
    pub fn classify_attempt(
        &self,
        task_id: TaskId,
        fingerprint: &str,
    ) -> AttemptStrategyClassification {
        self.classify_attempt_with_mission(None, task_id, fingerprint)
    }

    /// Classifies an attempt strategy considering both task-level and mission-level history across replan cycles.
    pub fn classify_attempt_with_mission(
        &self,
        mission_id: Option<MissionId>,
        task_id: TaskId,
        fingerprint: &str,
    ) -> AttemptStrategyClassification {
        if let Ok(guard) = self.attempt_fingerprints.read()
            && let Some(list) = guard.get(&task_id)
        {
            if list.iter().any(|f| f == fingerprint) {
                return AttemptStrategyClassification::RepeatedIdentical;
            }
            if !list.is_empty() {
                return AttemptStrategyClassification::ModifiedAttempt;
            }
        }
        if let Some(m_id) = mission_id
            && let Ok(guard) = self.mission_fingerprints.read()
            && let Some(list) = guard.get(&m_id)
        {
            if list.iter().any(|f| f == fingerprint) {
                return AttemptStrategyClassification::RepeatedIdentical;
            }
            if !list.is_empty() {
                return AttemptStrategyClassification::ModifiedAttempt;
            }
        }
        AttemptStrategyClassification::GenuinelyNew
    }

    /// Records an attempt's mutation fingerprint in memory for fast repeated attempt detection.
    pub fn record_attempt_fingerprint(&self, task_id: TaskId, fingerprint: impl Into<String>) {
        if let Ok(mut guard) = self.attempt_fingerprints.write() {
            guard.entry(task_id).or_default().push(fingerprint.into());
        }
    }

    /// Records an attempt's mutation fingerprint across both task and mission scopes.
    pub fn record_mission_attempt_fingerprint(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        fingerprint: impl Into<String>,
    ) {
        let fp = fingerprint.into();
        self.record_attempt_fingerprint(task_id, fp.clone());
        if let Ok(mut guard) = self.mission_fingerprints.write() {
            guard.entry(mission_id).or_default().push(fp);
        }
    }

    /// Records a failure signature in the mission's failure history for circuit breaking.
    pub fn record_failure_signature(&self, mission_id: MissionId, sig: SemanticFailureSignature) {
        if let Ok(mut guard) = self.failure_history.write() {
            guard.entry(mission_id).or_default().push(sig);
        }
    }

    /// Checks if repeated semantically equivalent failures have tripped the circuit breaker.
    pub fn is_circuit_broken(&self, mission_id: MissionId, sig: &SemanticFailureSignature) -> bool {
        if let Ok(guard) = self.failure_history.read()
            && let Some(history) = guard.get(&mission_id)
        {
            let count = history
                .iter()
                .filter(|prior| sig.is_equivalent(prior))
                .count();
            return count >= self.circuit_breaker_threshold;
        }
        false
    }

    /// Evaluates a change proposal against layered recovery budgets and prevents repeated identical mutations.
    pub fn evaluate_proposal(
        &self,
        task_id: TaskId,
        proposal: &crate::kernel::change::ChangeProposal,
        failure_class: FailureClassification,
        current_class_attempts: usize,
        current_task_attempts: usize,
        current_mission_attempts: usize,
    ) -> BudgetEvaluation {
        self.evaluate_proposal_for_mission(
            None,
            task_id,
            proposal,
            failure_class,
            current_class_attempts,
            current_task_attempts,
            current_mission_attempts,
        )
    }

    /// Evaluates a change proposal against layered recovery budgets across mission history.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_proposal_for_mission(
        &self,
        mission_id: Option<MissionId>,
        task_id: TaskId,
        proposal: &crate::kernel::change::ChangeProposal,
        failure_class: FailureClassification,
        current_class_attempts: usize,
        current_task_attempts: usize,
        current_mission_attempts: usize,
    ) -> BudgetEvaluation {
        let fp = compute_mutation_fingerprint(proposal);
        let classification = self.classify_attempt_with_mission(mission_id, task_id, &fp);
        if classification == AttemptStrategyClassification::RepeatedIdentical {
            return BudgetEvaluation::RepeatedIdentical {
                reason: format!(
                    "Identical failed mutation proposal rejected (fingerprint: {})",
                    fp
                ),
            };
        }
        self.evaluate(
            failure_class,
            current_class_attempts,
            current_task_attempts,
            current_mission_attempts,
            None,
        )
    }

    /// Evaluates whether another recovery attempt is permitted under layered budgets.
    pub fn evaluate(
        &self,
        failure_class: FailureClassification,
        current_class_attempts: usize,
        current_task_attempts: usize,
        current_mission_attempts: usize,
        retry_after_header: Option<u64>,
    ) -> BudgetEvaluation {
        // 1. Check deterministically non-retryable classes (Permission / Policy)
        if !failure_class.is_retryable() {
            return BudgetEvaluation::NonRetryable {
                reason: format!(
                    "Failure class '{failure_class}' is deterministically non-retryable under D-06 policy"
                ),
            };
        }

        // 2. Mission-level ceiling takes precedence (hard ceiling for entire mission)
        if current_mission_attempts >= self.mission_retry_ceiling {
            return BudgetEvaluation::Exhausted {
                reason: format!(
                    "Mission recovery ceiling exhausted ({current_mission_attempts}/{})",
                    self.mission_retry_ceiling
                ),
                consumed: current_mission_attempts,
                ceiling: self.mission_retry_ceiling,
            };
        }

        // 3. Class-specific limit
        let class_limit = failure_class.default_retry_limit();
        if current_class_attempts >= class_limit {
            return BudgetEvaluation::Exhausted {
                reason: format!(
                    "Class recovery budget exhausted for '{failure_class}' ({current_class_attempts}/{class_limit})"
                ),
                consumed: current_class_attempts,
                ceiling: class_limit,
            };
        }

        // 4. Task-level ceiling
        if current_task_attempts >= self.task_retry_ceiling {
            return BudgetEvaluation::Exhausted {
                reason: format!(
                    "Task recovery ceiling exhausted ({current_task_attempts}/{})",
                    self.task_retry_ceiling
                ),
                consumed: current_task_attempts,
                ceiling: self.task_retry_ceiling,
            };
        }

        let remaining_class = class_limit.saturating_sub(current_class_attempts + 1);
        let remaining_task = self
            .task_retry_ceiling
            .saturating_sub(current_task_attempts + 1);
        let remaining_mission = self
            .mission_retry_ceiling
            .saturating_sub(current_mission_attempts + 1);
        let remaining_overall = remaining_task.min(remaining_mission);

        let delay = self.compute_backoff(current_class_attempts, retry_after_header);

        BudgetEvaluation::Permitted {
            remaining_class_budget: remaining_class,
            remaining_overall_budget: remaining_overall,
            backoff_delay: delay,
        }
    }

    /// Computes exponential backoff with deterministic jitter (D-06).
    pub fn compute_backoff(&self, attempt: usize, retry_after_header: Option<u64>) -> Duration {
        if let Some(retry_after) = retry_after_header {
            let capped_ms = retry_after.min(self.backoff_config.max_delay_ms);
            return Duration::from_millis(capped_ms);
        }

        let base_delay = (self.backoff_config.initial_ms as f64)
            * self.backoff_config.multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.backoff_config.max_delay_ms as f64);

        // Deterministic pseudo-jitter bounded by jitter_percent
        // Generates predictable variation based on attempt without external rng state
        let pseudo_factor = ((attempt * 17 + 7) % 21) as f64 / 10.0 - 1.0; // range [-1.0, 1.0]
        let jitter = capped_delay * (self.backoff_config.jitter_percent / 100.0) * pseudo_factor;
        let final_delay =
            (capped_delay + jitter).clamp(0.0, self.backoff_config.max_delay_ms as f64);

        Duration::from_millis(final_delay as u64)
    }

    /// Durably records a recovery attempt in the SQLite `recovery_attempts` table (FLC-05).
    pub async fn record_attempt(
        &self,
        pool: &SqlitePool,
        record: RecoveryAttemptRecord,
    ) -> Result<Uuid, sqlx::Error> {
        let attempt_id = Uuid::now_v7();
        let created_at = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO recovery_attempts (
                id, mission_id, task_id, failure_class, strategy, attempt_number,
                budget_consumed, remaining_class_budget, remaining_overall_budget,
                backoff_delay_ms, action_taken, result, created_at,
                mutation_fingerprint, semantic_signature
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(attempt_id.as_bytes().as_slice())
        .bind(record.mission_id.as_bytes().as_slice())
        .bind(record.task_id.as_bytes().as_slice())
        .bind(record.failure_class.to_string())
        .bind(&record.strategy)
        .bind(record.attempt_number as i64)
        .bind(record.budget_consumed as i64)
        .bind(record.remaining_class_budget as i64)
        .bind(record.remaining_overall_budget as i64)
        .bind(record.backoff_delay_ms as i64)
        .bind(&record.action_taken)
        .bind(&record.result)
        .bind(&created_at)
        .bind(record.mutation_fingerprint.as_deref())
        .bind(record.semantic_signature.as_deref())
        .execute(pool)
        .await?;

        Ok(attempt_id)
    }

    /// Build a tracker pre-hydrated from durable mission records: attempt
    /// fingerprints repopulate the repeated-identical guard and semantic
    /// signatures rebuild circuit-breaker history, so a restart preserves
    /// duplicate-attempt protection (`fence = Y` before and after).
    pub async fn hydrated(pool: &SqlitePool, mission_id: MissionId) -> Result<Self, sqlx::Error> {
        let tracker = Self::default();
        let rows = sqlx::query(
            r#"
            SELECT mission_id, task_id, failure_class, strategy, attempt_number,
                   budget_consumed, remaining_class_budget, remaining_overall_budget,
                   backoff_delay_ms, action_taken, result,
                   mutation_fingerprint, semantic_signature
            FROM recovery_attempts
            WHERE mission_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(pool)
        .await?;
        let records = Self::map_attempt_rows(rows)?;
        for record in &records {
            if let Some(fp) = record.mutation_fingerprint.clone() {
                tracker.record_mission_attempt_fingerprint(record.mission_id, record.task_id, fp);
            }
            if let Some(sig_json) = record.semantic_signature.clone()
                && let Ok(sig) = serde_json::from_str::<SemanticFailureSignature>(&sig_json)
            {
                tracker.record_failure_signature(record.mission_id, sig);
            }
        }
        Ok(tracker)
    }

    /// Queries the total number of recovery attempts recorded for a given task.
    pub async fn count_task_attempts(
        pool: &SqlitePool,
        task_id: TaskId,
    ) -> Result<usize, sqlx::Error> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM recovery_attempts WHERE task_id = ?")
                .bind(task_id.as_bytes().as_slice())
                .fetch_one(pool)
                .await?;

        Ok(row.0 as usize)
    }

    /// Queries the number of recovery attempts recorded for a given task and failure class.
    pub async fn count_class_attempts(
        pool: &SqlitePool,
        task_id: TaskId,
        failure_class: FailureClassification,
    ) -> Result<usize, sqlx::Error> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM recovery_attempts WHERE task_id = ? AND failure_class = ?",
        )
        .bind(task_id.as_bytes().as_slice())
        .bind(failure_class.to_string())
        .fetch_one(pool)
        .await?;

        Ok(row.0 as usize)
    }

    /// Queries the total number of recovery attempts recorded for a given mission.
    pub async fn count_mission_attempts(
        pool: &SqlitePool,
        mission_id: MissionId,
    ) -> Result<usize, sqlx::Error> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM recovery_attempts WHERE mission_id = ?")
                .bind(mission_id.as_bytes().as_slice())
                .fetch_one(pool)
                .await?;

        Ok(row.0 as usize)
    }

    /// Queries all recovery attempt records for a given task.
    pub async fn get_task_attempt_records(
        pool: &SqlitePool,
        task_id: TaskId,
    ) -> Result<Vec<RecoveryAttemptRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT mission_id, task_id, failure_class, strategy, attempt_number,
                   budget_consumed, remaining_class_budget, remaining_overall_budget,
                   backoff_delay_ms, action_taken, result,
                   mutation_fingerprint, semantic_signature
            FROM recovery_attempts
            WHERE task_id = ?
            ORDER BY attempt_number ASC
            "#,
        )
        .bind(task_id.as_bytes().as_slice())
        .fetch_all(pool)
        .await?;

        Self::map_attempt_rows(rows)
    }

    /// Retrieve all recorded recovery attempts across all tasks (canonical audit query).
    pub async fn query_all_attempts(
        pool: &SqlitePool,
    ) -> Result<Vec<RecoveryAttemptRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT mission_id, task_id, failure_class, strategy, attempt_number,
                   budget_consumed, remaining_class_budget, remaining_overall_budget,
                   backoff_delay_ms, action_taken, result,
                   mutation_fingerprint, semantic_signature
            FROM recovery_attempts
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(pool)
        .await?;

        Self::map_attempt_rows(rows)
    }

    /// Retrieve recent recorded recovery attempts up to limit.
    pub async fn query_recent_attempts(
        pool: &SqlitePool,
        limit: usize,
    ) -> Result<Vec<RecoveryAttemptRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT mission_id, task_id, failure_class, strategy, attempt_number,
                   budget_consumed, remaining_class_budget, remaining_overall_budget,
                   backoff_delay_ms, action_taken, result,
                   mutation_fingerprint, semantic_signature
            FROM recovery_attempts
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

        Self::map_attempt_rows(rows)
    }

    pub fn map_attempt_rows(
        rows: Vec<sqlx::sqlite::SqliteRow>,
    ) -> Result<Vec<RecoveryAttemptRecord>, sqlx::Error> {
        use sqlx::Row;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            let m_bytes: Vec<u8> = row.try_get("mission_id")?;
            let t_bytes: Vec<u8> = row.try_get("task_id")?;
            let fc_str: String = row.try_get("failure_class")?;
            let strategy: String = row.try_get("strategy")?;
            let attempt_number: i64 = row.try_get("attempt_number")?;
            let budget_consumed: i64 = row.try_get("budget_consumed")?;
            let rem_class: i64 = row.try_get("remaining_class_budget")?;
            let rem_overall: i64 = row.try_get("remaining_overall_budget")?;
            let backoff_delay_ms: i64 = row.try_get("backoff_delay_ms")?;
            let action_taken: String = row.try_get("action_taken")?;
            let result: String = row.try_get("result")?;

            let mission_id = if m_bytes.len() == 16 {
                let mut b = [0u8; 16];
                b.copy_from_slice(&m_bytes);
                MissionId::from_bytes(b)
            } else {
                // Fabricated mission identities fail closed.
                return Err(sqlx::Error::Protocol(format!(
                    "corrupt recovery attempt mission identity: expected 16 bytes, got {}",
                    m_bytes.len()
                )));
            };

            let task_id = if t_bytes.len() == 16 {
                let mut b = [0u8; 16];
                b.copy_from_slice(&t_bytes);
                TaskId::from_bytes(b)
            } else {
                // Fabricated task identities fail closed.
                return Err(sqlx::Error::Protocol(format!(
                    "corrupt recovery attempt task identity: expected 16 bytes, got {}",
                    t_bytes.len()
                )));
            };

            // Unknown failure classes become Corrupt (non-retryable, zero
            // retries) instead of Unknown (retryable). The three legacy
            // aliases below are explicit backward compatibility (previously
            // emitted values with documented targets), not silent reinterpretation:
            // concurrency→Transient, lint→Compilation, sandbox→Permission (fail-closed direction).
            let failure_class = match fc_str.as_str() {
                "transient" => FailureClassification::Transient,
                "timeout" => FailureClassification::Timeout,
                "concurrency" => FailureClassification::Transient,
                "compilation" => FailureClassification::Compilation,
                "test" => FailureClassification::Test,
                "lint" => FailureClassification::Compilation,
                "permission" => FailureClassification::Permission,
                "policy" => FailureClassification::Policy,
                "sandbox" => FailureClassification::Permission,
                "dependency" => FailureClassification::Dependency,
                "model" => FailureClassification::Model,
                "context" => FailureClassification::Context,
                "resource_limit" => FailureClassification::ResourceLimit,
                "repository_state" => FailureClassification::RepositoryState,
                "architecture" => FailureClassification::Architecture,
                "permanent" => FailureClassification::Permanent,
                "fatal_violation" => FailureClassification::FatalViolation,
                "configuration" => FailureClassification::Configuration,
                "environment" => FailureClassification::Environment,
                "tool_contract" => FailureClassification::ToolContract,
                "unknown" => FailureClassification::Unknown,
                "corrupt" => FailureClassification::Corrupt,
                _ => FailureClassification::Corrupt,
            };

            // Additive columns read tolerantly (absent on earlier schema
            // rows); present values rehydrate duplicate protection.
            let mutation_fingerprint: Option<String> =
                row.try_get("mutation_fingerprint").ok().flatten();
            let semantic_signature: Option<String> =
                row.try_get("semantic_signature").ok().flatten();

            records.push(RecoveryAttemptRecord {
                mission_id,
                task_id,
                failure_class,
                strategy,
                attempt_number: attempt_number as usize,
                budget_consumed: budget_consumed as usize,
                remaining_class_budget: rem_class as usize,
                remaining_overall_budget: rem_overall as usize,
                backoff_delay_ms: backoff_delay_ms as u64,
                action_taken,
                result,
                mutation_fingerprint,
                semantic_signature,
            });
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_retryable_evaluations() {
        let tracker = RecoveryBudgetTracker::default();
        let eval = tracker.evaluate(FailureClassification::Permission, 0, 0, 0, None);
        assert!(matches!(eval, BudgetEvaluation::NonRetryable { .. }));

        let eval = tracker.evaluate(FailureClassification::Policy, 0, 0, 0, None);
        assert!(matches!(eval, BudgetEvaluation::NonRetryable { .. }));
    }

    #[test]
    fn test_class_budget_exhaustion() {
        let tracker = RecoveryBudgetTracker::default();
        // Transient has budget of 3
        let eval = tracker.evaluate(FailureClassification::Transient, 2, 2, 2, None);
        assert!(matches!(eval, BudgetEvaluation::Permitted { .. }));

        let eval = tracker.evaluate(FailureClassification::Transient, 3, 3, 3, None);
        assert!(matches!(eval, BudgetEvaluation::Exhausted { .. }));
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let tracker = RecoveryBudgetTracker::default();

        let b0 = tracker.compute_backoff(0, None);
        let b1 = tracker.compute_backoff(1, None);
        let b2 = tracker.compute_backoff(2, None);

        // Attempt 0: ~500ms
        assert!(b0.as_millis() >= 450 && b0.as_millis() <= 550);
        // Attempt 1: ~1000ms
        assert!(b1.as_millis() >= 900 && b1.as_millis() <= 1100);
        // Attempt 2: ~2000ms
        assert!(b2.as_millis() >= 1800 && b2.as_millis() <= 2200);

        // Honors Retry-After header
        let b_retry_after = tracker.compute_backoff(0, Some(7500));
        assert_eq!(b_retry_after.as_millis(), 7500);
    }

    #[test]
    fn test_retry_intelligence_and_repeated_attempt_rejection() {
        use crate::kernel::change::{
            ChangeProposal, ChangeSurface, FileMutationOp, FileMutationProposal,
            ImplementationHypothesis,
        };

        let tracker = RecoveryBudgetTracker::default();
        let task_id = TaskId::new();
        let mission_id = MissionId::new();

        let proposal = ChangeProposal {
            id: crate::kernel::change::ChangeProposalId::new(),
            task_id,
            mission_id,
            intent: ImplementationHypothesis::new("prob", "cause", "change", "res", "ver"),
            change_surface: ChangeSurface::new(vec!["src/lib.rs".to_string()]),
            preconditions: vec![],
            mutations: vec![FileMutationProposal::new(
                "src/lib.rs",
                FileMutationOp::CreateNew {
                    content: "pub fn repaired() {}".to_string(),
                },
                "fix",
            )],
            assumptions: vec![],
            verification_plan: vec![],
            risk_level: None,
            timestamp: Utc::now(),
        };

        let fp = compute_mutation_fingerprint(&proposal);
        assert!(!fp.is_empty());

        // First attempt: GenuinelyNew
        assert_eq!(
            tracker.classify_attempt(task_id, &fp),
            AttemptStrategyClassification::GenuinelyNew
        );
        let eval1 = tracker.evaluate_proposal(
            task_id,
            &proposal,
            FailureClassification::Compilation,
            0,
            0,
            0,
        );
        assert!(matches!(eval1, BudgetEvaluation::Permitted { .. }));

        // Record the attempt
        tracker.record_attempt_fingerprint(task_id, &fp);

        // Second identical attempt: RepeatedIdentical -> Rejected!
        assert_eq!(
            tracker.classify_attempt(task_id, &fp),
            AttemptStrategyClassification::RepeatedIdentical
        );
        let eval2 = tracker.evaluate_proposal(
            task_id,
            &proposal,
            FailureClassification::Compilation,
            1,
            1,
            1,
        );
        assert!(matches!(eval2, BudgetEvaluation::RepeatedIdentical { .. }));

        // Different proposal: ModifiedAttempt
        let mut proposal2 = proposal.clone();
        proposal2.mutations[0].operation = FileMutationOp::CreateNew {
            content: "pub fn repaired_v2() {}".to_string(),
        };
        let fp2 = compute_mutation_fingerprint(&proposal2);
        assert_ne!(fp, fp2);
        assert_eq!(
            tracker.classify_attempt(task_id, &fp2),
            AttemptStrategyClassification::ModifiedAttempt
        );
        let eval3 = tracker.evaluate_proposal(
            task_id,
            &proposal2,
            FailureClassification::Compilation,
            1,
            1,
            1,
        );
        assert!(matches!(eval3, BudgetEvaluation::Permitted { .. }));
    }
}
