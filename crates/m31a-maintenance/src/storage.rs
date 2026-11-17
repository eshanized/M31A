//! Durable SQLite Persistence Engine for Maintenance Subsystem (`m31a-maintenance::storage`).
//!
//! Provides transactional persistence, deterministic deduplication, atomic state transitions,
//! schedule tracking, and immutable audit events across system restarts.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::domain::{
    AuditTrailEvent, AutonomyTier, CaseApprovalState, CaseStatus, ExecutionLifecycleState,
    MaintenanceCase, MaintenanceCategory, MaintenanceExecution, MaintenancePlan,
    MaintenancePriority, MaintenanceRiskLevel, MaintenanceSchedule, MaintenanceSignal,
    PlanApprovalState, SignalProvider, SignalSeverity, SignalTrustStatus,
};

/// Persistent SQLite maintenance storage manager.
#[derive(Debug, Clone)]
pub struct MaintenanceStore {
    conn: Arc<Mutex<Connection>>,
}

impl MaintenanceStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    // ---------------------------------------------------------------------------------------------
    // SIGNAL OPERATIONS
    // ---------------------------------------------------------------------------------------------

    /// Records an incoming signal. Returns `Ok(true)` if newly inserted, or `Ok(false)` if duplicate.
    pub fn record_signal(&self, signal: &MaintenanceSignal) -> Result<bool> {
        let conn = self.conn.lock().unwrap();

        // 1. Check for duplicate deduplication fingerprint
        let mut check_stmt = conn.prepare_cached(
            "SELECT id FROM maintenance_signals WHERE dedup_fingerprint = ?1 LIMIT 1",
        )?;
        let exists = check_stmt.exists(params![signal.dedup_fingerprint])?;
        if exists {
            return Ok(false);
        }

        // 2. Insert signal
        let mut insert_stmt = conn.prepare_cached(
            r#"
            INSERT INTO maintenance_signals (
                id, repo_id, source_provider, category, provider_event_id,
                external_url, event_timestamp_ms, observed_revision, severity,
                dedup_fingerprint, trust_status, metadata_json, evidence_refs_json,
                ingested_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
        )?;

        let metadata_str = serde_json::to_string(&signal.metadata)?;
        let evidence_str = serde_json::to_string(&signal.evidence_refs)?;

        insert_stmt.execute(params![
            signal.id,
            signal.repo_id,
            signal.source_provider.as_str(),
            signal.category.as_str(),
            signal.provider_event_id,
            signal.external_url,
            signal.event_timestamp_ms,
            signal.observed_revision,
            signal.severity.as_str(),
            signal.dedup_fingerprint,
            signal.trust_status.as_str(),
            metadata_str,
            evidence_str,
            signal.ingested_at_epoch_ms,
        ])?;

        Ok(true)
    }

    /// Retrieves signal by stable ID.
    pub fn get_signal(&self, id: &str) -> Result<Option<MaintenanceSignal>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, repo_id, source_provider, category, provider_event_id,
                   external_url, event_timestamp_ms, observed_revision, severity,
                   dedup_fingerprint, trust_status, metadata_json, evidence_refs_json,
                   ingested_at_epoch_ms
            FROM maintenance_signals WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_signal(row)?))
        } else {
            Ok(None)
        }
    }

    /// Retrieves signal by deduplication fingerprint.
    pub fn get_signal_by_fingerprint(&self, fp: &str) -> Result<Option<MaintenanceSignal>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, repo_id, source_provider, category, provider_event_id,
                   external_url, event_timestamp_ms, observed_revision, severity,
                   dedup_fingerprint, trust_status, metadata_json, evidence_refs_json,
                   ingested_at_epoch_ms
            FROM maintenance_signals WHERE dedup_fingerprint = ?1 LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query(params![fp])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_signal(row)?))
        } else {
            Ok(None)
        }
    }

    /// Lists recent signals for a repository.
    pub fn list_signals_for_repo(
        &self,
        repo_id: &str,
        limit: usize,
    ) -> Result<Vec<MaintenanceSignal>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, repo_id, source_provider, category, provider_event_id,
                   external_url, event_timestamp_ms, observed_revision, severity,
                   dedup_fingerprint, trust_status, metadata_json, evidence_refs_json,
                   ingested_at_epoch_ms
            FROM maintenance_signals WHERE repo_id = ?1
            ORDER BY event_timestamp_ms DESC LIMIT ?2
            "#,
        )?;

        let rows = stmt.query_map(params![repo_id, limit as i64], |row| {
            Self::row_to_signal(row)
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Lists signals associated with a specific case.
    pub fn list_signals_for_case(&self, case_id: &str) -> Result<Vec<MaintenanceSignal>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT s.id, s.repo_id, s.source_provider, s.category, s.provider_event_id,
                   s.external_url, s.event_timestamp_ms, s.observed_revision, s.severity,
                   s.dedup_fingerprint, s.trust_status, s.metadata_json, s.evidence_refs_json,
                   s.ingested_at_epoch_ms
            FROM maintenance_signals s
            INNER JOIN maintenance_case_signals cs ON s.id = cs.signal_id
            WHERE cs.case_id = ?1
            ORDER BY cs.associated_at_epoch_ms ASC
            "#,
        )?;

        let rows = stmt.query_map(params![case_id], |row| Self::row_to_signal(row))?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // ---------------------------------------------------------------------------------------------
    // CASE OPERATIONS
    // ---------------------------------------------------------------------------------------------

    /// Creates and persists a new maintenance case, optionally linking an initial signal.
    pub fn create_case(
        &self,
        case: &MaintenanceCase,
        initial_signal_id: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let affected_pkgs = serde_json::to_string(&case.affected_packages)?;
        let affected_files = serde_json::to_string(
            &case
                .affected_files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
        )?;
        let affected_syms = serde_json::to_string(&case.affected_symbols)?;
        let evidence_refs = serde_json::to_string(&case.evidence_refs)?;
        let related_issues = serde_json::to_string(&case.related_issues)?;
        let related_prs = serde_json::to_string(&case.related_prs)?;
        let remediation_history = serde_json::to_string(&case.remediation_history)?;

        tx.execute(
            r#"
            INSERT INTO maintenance_cases (
                id, repo_id, category, title, problem_statement, dedup_key,
                status, priority, severity, confidence_score, affected_packages_json,
                affected_files_json, affected_symbols_json, evidence_refs_json,
                related_issues_json, related_prs_json, workflow_id, active_plan_id,
                remediation_history_json, human_approval_state, created_at_epoch_ms,
                updated_at_epoch_ms, last_observed_at_epoch_ms, resolved_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)
            "#,
            params![
                case.id,
                case.repo_id,
                case.category.as_str(),
                case.title,
                case.problem_statement,
                case.dedup_key,
                case.status.as_str(),
                case.priority.as_str(),
                case.severity.as_str(),
                case.confidence_score,
                affected_pkgs,
                affected_files,
                affected_syms,
                evidence_refs,
                related_issues,
                related_prs,
                case.workflow_id,
                case.active_plan_id,
                remediation_history,
                case.human_approval_state.as_str(),
                case.created_at_epoch_ms,
                case.updated_at_epoch_ms,
                case.last_observed_at_epoch_ms,
                case.resolved_at_epoch_ms,
            ],
        )?;

        if let Some(sig_id) = initial_signal_id {
            tx.execute(
                r#"
                INSERT OR IGNORE INTO maintenance_case_signals (case_id, signal_id, associated_at_epoch_ms)
                VALUES (?1, ?2, ?3)
                "#,
                params![case.id, sig_id, Utc::now().timestamp_millis()],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Retrieves maintenance case by ID.
    pub fn get_case(&self, id: &str) -> Result<Option<MaintenanceCase>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, repo_id, category, title, problem_statement, dedup_key,
                   status, priority, severity, confidence_score, affected_packages_json,
                   affected_files_json, affected_symbols_json, evidence_refs_json,
                   related_issues_json, related_prs_json, workflow_id, active_plan_id,
                   remediation_history_json, human_approval_state, created_at_epoch_ms,
                   updated_at_epoch_ms, last_observed_at_epoch_ms, resolved_at_epoch_ms
            FROM maintenance_cases WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_case(row)?))
        } else {
            Ok(None)
        }
    }

    /// Finds open or active case by repo ID and deduplication key.
    pub fn find_case_by_dedup_key(
        &self,
        repo_id: &str,
        dedup_key: &str,
    ) -> Result<Option<MaintenanceCase>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, repo_id, category, title, problem_statement, dedup_key,
                   status, priority, severity, confidence_score, affected_packages_json,
                   affected_files_json, affected_symbols_json, evidence_refs_json,
                   related_issues_json, related_prs_json, workflow_id, active_plan_id,
                   remediation_history_json, human_approval_state, created_at_epoch_ms,
                   updated_at_epoch_ms, last_observed_at_epoch_ms, resolved_at_epoch_ms
            FROM maintenance_cases
            WHERE repo_id = ?1 AND dedup_key = ?2 AND status NOT IN ('resolved', 'failed', 'cancelled')
            LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query(params![repo_id, dedup_key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_case(row)?))
        } else {
            Ok(None)
        }
    }

    /// Updates existing maintenance case state.
    pub fn update_case(&self, case: &MaintenanceCase) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let affected_pkgs = serde_json::to_string(&case.affected_packages)?;
        let affected_files = serde_json::to_string(
            &case
                .affected_files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
        )?;
        let affected_syms = serde_json::to_string(&case.affected_symbols)?;
        let evidence_refs = serde_json::to_string(&case.evidence_refs)?;
        let related_issues = serde_json::to_string(&case.related_issues)?;
        let related_prs = serde_json::to_string(&case.related_prs)?;
        let remediation_history = serde_json::to_string(&case.remediation_history)?;

        conn.execute(
            r#"
            UPDATE maintenance_cases SET
                status = ?2,
                priority = ?3,
                severity = ?4,
                confidence_score = ?5,
                affected_packages_json = ?6,
                affected_files_json = ?7,
                affected_symbols_json = ?8,
                evidence_refs_json = ?9,
                related_issues_json = ?10,
                related_prs_json = ?11,
                workflow_id = ?12,
                active_plan_id = ?13,
                remediation_history_json = ?14,
                human_approval_state = ?15,
                updated_at_epoch_ms = ?16,
                last_observed_at_epoch_ms = ?17,
                resolved_at_epoch_ms = ?18
            WHERE id = ?1
            "#,
            params![
                case.id,
                case.status.as_str(),
                case.priority.as_str(),
                case.severity.as_str(),
                case.confidence_score,
                affected_pkgs,
                affected_files,
                affected_syms,
                evidence_refs,
                related_issues,
                related_prs,
                case.workflow_id,
                case.active_plan_id,
                remediation_history,
                case.human_approval_state.as_str(),
                case.updated_at_epoch_ms,
                case.last_observed_at_epoch_ms,
                case.resolved_at_epoch_ms,
            ],
        )?;

        Ok(())
    }

    /// Lists cases matching optional repo filter and status.
    pub fn list_cases(
        &self,
        repo_id: Option<&str>,
        status: Option<CaseStatus>,
        limit: usize,
    ) -> Result<Vec<MaintenanceCase>> {
        let conn = self.conn.lock().unwrap();
        let mut query = String::from(
            r#"
            SELECT id, repo_id, category, title, problem_statement, dedup_key,
                   status, priority, severity, confidence_score, affected_packages_json,
                   affected_files_json, affected_symbols_json, evidence_refs_json,
                   related_issues_json, related_prs_json, workflow_id, active_plan_id,
                   remediation_history_json, human_approval_state, created_at_epoch_ms,
                   updated_at_epoch_ms, last_observed_at_epoch_ms, resolved_at_epoch_ms
            FROM maintenance_cases WHERE 1=1
            "#,
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(r) = repo_id {
            params.push(Box::new(r.to_string()));
            query.push_str(&format!(" AND repo_id = ?{}", params.len()));
        }
        if let Some(s) = status {
            params.push(Box::new(s.as_str().to_string()));
            query.push_str(&format!(" AND status = ?{}", params.len()));
        }
        params.push(Box::new(limit as i64));
        query.push_str(&format!(
            " ORDER BY updated_at_epoch_ms DESC LIMIT ?{}",
            params.len()
        ));

        let mut stmt = conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| Self::row_to_case(row))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Associates an incoming signal with an existing case.
    pub fn associate_signal_with_case(&self, case_id: &str, signal_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT OR IGNORE INTO maintenance_case_signals (case_id, signal_id, associated_at_epoch_ms)
            VALUES (?1, ?2, ?3)
            "#,
            params![case_id, signal_id, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------
    // PLAN OPERATIONS
    // ---------------------------------------------------------------------------------------------

    /// Saves or updates a maintenance plan.
    pub fn save_plan(&self, plan: &MaintenancePlan) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let hypotheses_json = serde_json::to_string(&plan.hypotheses)?;
        let expected_files = serde_json::to_string(
            &plan
                .expected_files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
        )?;
        let expected_pkgs = serde_json::to_string(&plan.expected_packages)?;
        let relevant_tests = serde_json::to_string(&plan.relevant_tests)?;
        let verif_reqs = serde_json::to_string(&plan.verification_requirements)?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO maintenance_plans (
                id, case_id, repo_id, baseline_sha, category, problem_classification,
                selected_strategy, hypotheses_json, expected_files_json, expected_packages_json,
                relevant_tests_json, verification_requirements_json, estimated_risk,
                required_autonomy_tier, requires_approval, approval_state, workflow_id,
                plan_hash, created_at_epoch_ms, updated_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
            "#,
            params![
                plan.id,
                plan.case_id,
                plan.repo_id,
                plan.baseline_sha,
                plan.category.as_str(),
                plan.problem_classification,
                plan.selected_strategy,
                hypotheses_json,
                expected_files,
                expected_pkgs,
                relevant_tests,
                verif_reqs,
                plan.estimated_risk.as_str(),
                plan.required_autonomy_tier.level() as i64,
                if plan.requires_approval { 1 } else { 0 },
                plan.approval_state.as_str(),
                plan.workflow_id,
                plan.plan_hash,
                plan.created_at_epoch_ms,
                plan.updated_at_epoch_ms,
            ],
        )?;

        Ok(())
    }

    /// Retrieves a maintenance plan by ID.
    pub fn get_plan(&self, plan_id: &str) -> Result<Option<MaintenancePlan>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, case_id, repo_id, baseline_sha, category, problem_classification,
                   selected_strategy, hypotheses_json, expected_files_json, expected_packages_json,
                   relevant_tests_json, verification_requirements_json, estimated_risk,
                   required_autonomy_tier, requires_approval, approval_state, workflow_id,
                   plan_hash, created_at_epoch_ms, updated_at_epoch_ms
            FROM maintenance_plans WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![plan_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_plan(row)?))
        } else {
            Ok(None)
        }
    }

    /// Retrieves active or latest maintenance plan for a case.
    pub fn get_plan_for_case(&self, case_id: &str) -> Result<Option<MaintenancePlan>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, case_id, repo_id, baseline_sha, category, problem_classification,
                   selected_strategy, hypotheses_json, expected_files_json, expected_packages_json,
                   relevant_tests_json, verification_requirements_json, estimated_risk,
                   required_autonomy_tier, requires_approval, approval_state, workflow_id,
                   plan_hash, created_at_epoch_ms, updated_at_epoch_ms
            FROM maintenance_plans WHERE case_id = ?1
            ORDER BY updated_at_epoch_ms DESC LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query(params![case_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_plan(row)?))
        } else {
            Ok(None)
        }
    }

    /// Updates approval status on a plan.
    pub fn update_plan_approval(
        &self,
        plan_id: &str,
        approval_state: PlanApprovalState,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            r#"
            UPDATE maintenance_plans
            SET approval_state = ?2, updated_at_epoch_ms = ?3
            WHERE id = ?1
            "#,
            params![plan_id, approval_state.as_str(), now],
        )?;
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------
    // EXECUTION OPERATIONS
    // ---------------------------------------------------------------------------------------------

    /// Records or updates a maintenance workflow execution attempt.
    pub fn record_execution(&self, exec: &MaintenanceExecution) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO maintenance_executions (
                id, case_id, workflow_id, plan_id, manifest_hash, attempt_id,
                worker_id, execution_state, verification_report_hash, promotion_state,
                pr_number, pr_url, failure_classification, duration_ms, tokens_used,
                created_at_epoch_ms, completed_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            "#,
            params![
                exec.id,
                exec.case_id,
                exec.workflow_id,
                exec.plan_id,
                exec.manifest_hash,
                exec.attempt_id,
                exec.worker_id,
                exec.execution_state.as_str(),
                exec.verification_report_hash,
                exec.promotion_state,
                exec.pr_number,
                exec.pr_url,
                exec.failure_classification,
                exec.duration_ms as i64,
                exec.tokens_used as i64,
                exec.created_at_epoch_ms,
                exec.completed_at_epoch_ms,
            ],
        )?;
        Ok(())
    }

    /// Retrieves execution attempt by ID.
    pub fn get_execution(&self, id: &str) -> Result<Option<MaintenanceExecution>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, case_id, workflow_id, plan_id, manifest_hash, attempt_id,
                   worker_id, execution_state, verification_report_hash, promotion_state,
                   pr_number, pr_url, failure_classification, duration_ms, tokens_used,
                   created_at_epoch_ms, completed_at_epoch_ms
            FROM maintenance_executions WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_execution(row)?))
        } else {
            Ok(None)
        }
    }

    /// Lists executions for a case.
    pub fn list_executions_for_case(&self, case_id: &str) -> Result<Vec<MaintenanceExecution>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, case_id, workflow_id, plan_id, manifest_hash, attempt_id,
                   worker_id, execution_state, verification_report_hash, promotion_state,
                   pr_number, pr_url, failure_classification, duration_ms, tokens_used,
                   created_at_epoch_ms, completed_at_epoch_ms
            FROM maintenance_executions WHERE case_id = ?1
            ORDER BY created_at_epoch_ms ASC
            "#,
        )?;

        let rows = stmt.query_map(params![case_id], |row| Self::row_to_execution(row))?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // ---------------------------------------------------------------------------------------------
    // SCHEDULE OPERATIONS
    // ---------------------------------------------------------------------------------------------

    /// Saves or updates a maintenance schedule.
    pub fn save_schedule(&self, sched: &MaintenanceSchedule) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let override_json = sched.policy_override.as_ref().map(|v| v.to_string());
        conn.execute(
            r#"
            INSERT OR REPLACE INTO maintenance_schedules (
                id, repo_id, job_kind, cron_or_interval_secs, is_enabled,
                last_run_epoch_ms, next_run_epoch_ms, consecutive_failures,
                policy_override_json, created_at_epoch_ms, updated_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                sched.id,
                sched.repo_id,
                sched.job_kind,
                sched.cron_or_interval_secs,
                if sched.is_enabled { 1 } else { 0 },
                sched.last_run_epoch_ms,
                sched.next_run_epoch_ms,
                sched.consecutive_failures,
                override_json,
                sched.created_at_epoch_ms,
                sched.updated_at_epoch_ms,
            ],
        )?;
        Ok(())
    }

    /// Retrieves schedule by ID.
    pub fn get_schedule(&self, id: &str) -> Result<Option<MaintenanceSchedule>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, repo_id, job_kind, cron_or_interval_secs, is_enabled,
                   last_run_epoch_ms, next_run_epoch_ms, consecutive_failures,
                   policy_override_json, created_at_epoch_ms, updated_at_epoch_ms
            FROM maintenance_schedules WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_schedule(row)?))
        } else {
            Ok(None)
        }
    }

    /// Lists schedules, optionally filtered by repo.
    pub fn list_schedules(&self, repo_id: Option<&str>) -> Result<Vec<MaintenanceSchedule>> {
        let conn = self.conn.lock().unwrap();
        let mut query = String::from(
            r#"
            SELECT id, repo_id, job_kind, cron_or_interval_secs, is_enabled,
                   last_run_epoch_ms, next_run_epoch_ms, consecutive_failures,
                   policy_override_json, created_at_epoch_ms, updated_at_epoch_ms
            FROM maintenance_schedules WHERE 1=1
            "#,
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(r) = repo_id {
            params.push(Box::new(r.to_string()));
            query.push_str(&format!(" AND repo_id = ?{}", params.len()));
        }
        query.push_str(" ORDER BY next_run_epoch_ms ASC");

        let mut stmt = conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| Self::row_to_schedule(row))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Updates schedule after execution.
    pub fn update_schedule_run(
        &self,
        sched_id: &str,
        next_run_epoch_ms: i64,
        success: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        if success {
            conn.execute(
                r#"
                UPDATE maintenance_schedules
                SET last_run_epoch_ms = ?2, next_run_epoch_ms = ?3, consecutive_failures = 0, updated_at_epoch_ms = ?2
                WHERE id = ?1
                "#,
                params![sched_id, now, next_run_epoch_ms],
            )?;
        } else {
            conn.execute(
                r#"
                UPDATE maintenance_schedules
                SET last_run_epoch_ms = ?2, next_run_epoch_ms = ?3, consecutive_failures = consecutive_failures + 1, updated_at_epoch_ms = ?2
                WHERE id = ?1
                "#,
                params![sched_id, now, next_run_epoch_ms],
            )?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------
    // AUDIT TRAIL OPERATIONS
    // ---------------------------------------------------------------------------------------------

    /// Records tamper-evident audit trail event.
    pub fn record_audit_event(&self, event: &AuditTrailEvent) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let details_str = serde_json::to_string(&event.details)?;
        conn.execute(
            r#"
            INSERT INTO maintenance_audit_trail (
                id, case_id, repo_id, event_type, actor, policy_version,
                details_json, timestamp_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                event.id,
                event.case_id,
                event.repo_id,
                event.event_type,
                event.actor,
                event.policy_version,
                details_str,
                event.timestamp_epoch_ms,
            ],
        )?;
        Ok(())
    }

    /// Lists audit trail events for a case.
    pub fn list_audit_events_for_case(&self, case_id: &str) -> Result<Vec<AuditTrailEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, case_id, repo_id, event_type, actor, policy_version,
                   details_json, timestamp_epoch_ms
            FROM maintenance_audit_trail WHERE case_id = ?1
            ORDER BY timestamp_epoch_ms ASC
            "#,
        )?;

        let rows = stmt.query_map(params![case_id], |row| {
            let details_str: String = row.get(6)?;
            let details: serde_json::Value =
                serde_json::from_str(&details_str).unwrap_or(serde_json::Value::Null);
            Ok(AuditTrailEvent {
                id: row.get(0)?,
                case_id: row.get(1)?,
                repo_id: row.get(2)?,
                event_type: row.get(3)?,
                actor: row.get(4)?,
                policy_version: row.get(5)?,
                details,
                timestamp_epoch_ms: row.get(7)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // ---------------------------------------------------------------------------------------------
    // HELPER DESERIALIZERS
    // ---------------------------------------------------------------------------------------------

    fn row_to_signal(row: &rusqlite::Row) -> rusqlite::Result<MaintenanceSignal> {
        let provider_str: String = row.get(2)?;
        let provider = match provider_str.as_str() {
            "github" => SignalProvider::GitHub,
            "local_analysis" => SignalProvider::LocalAnalysis,
            "vibe_loop" => SignalProvider::VibeEngine,
            "scheduled" => SignalProvider::Scheduled,
            other => SignalProvider::Custom(other.to_string()),
        };

        let cat_str: String = row.get(3)?;
        let category =
            MaintenanceCategory::from_str(&cat_str).unwrap_or(MaintenanceCategory::BrokenCi);

        let sev_str: String = row.get(8)?;
        let severity = SignalSeverity::from_str(&sev_str).unwrap_or(SignalSeverity::Medium);

        let trust_str: String = row.get(10)?;
        let trust_status = match trust_str.as_str() {
            "verified" => SignalTrustStatus::Verified,
            "internal" => SignalTrustStatus::Internal,
            _ => SignalTrustStatus::Unverified,
        };

        let meta_str: String = row.get(11)?;
        let metadata = serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null);

        let evidence_str: String = row.get(12)?;
        let evidence_refs = serde_json::from_str(&evidence_str).unwrap_or_default();

        Ok(MaintenanceSignal {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            source_provider: provider,
            category,
            provider_event_id: row.get(4)?,
            external_url: row.get(5)?,
            event_timestamp_ms: row.get(6)?,
            observed_revision: row.get(7)?,
            severity,
            dedup_fingerprint: row.get(9)?,
            trust_status,
            metadata,
            evidence_refs,
            ingested_at_epoch_ms: row.get(13)?,
        })
    }

    fn row_to_case(row: &rusqlite::Row) -> rusqlite::Result<MaintenanceCase> {
        let cat_str: String = row.get(2)?;
        let category =
            MaintenanceCategory::from_str(&cat_str).unwrap_or(MaintenanceCategory::BrokenCi);

        let status_str: String = row.get(6)?;
        let status = CaseStatus::from_str(&status_str).unwrap_or(CaseStatus::Open);

        let prio_str: String = row.get(7)?;
        let priority =
            MaintenancePriority::from_str(&prio_str).unwrap_or(MaintenancePriority::Medium);

        let sev_str: String = row.get(8)?;
        let severity = SignalSeverity::from_str(&sev_str).unwrap_or(SignalSeverity::Medium);

        let pkgs_str: String = row.get(10)?;
        let affected_packages = serde_json::from_str(&pkgs_str).unwrap_or_default();

        let files_str: String = row.get(11)?;
        let files_raw: Vec<String> = serde_json::from_str(&files_str).unwrap_or_default();
        let affected_files = files_raw.into_iter().map(PathBuf::from).collect();

        let syms_str: String = row.get(12)?;
        let affected_symbols = serde_json::from_str(&syms_str).unwrap_or_default();

        let ev_str: String = row.get(13)?;
        let evidence_refs = serde_json::from_str(&ev_str).unwrap_or_default();

        let issues_str: String = row.get(14)?;
        let related_issues = serde_json::from_str(&issues_str).unwrap_or_default();

        let prs_str: String = row.get(15)?;
        let related_prs = serde_json::from_str(&prs_str).unwrap_or_default();

        let history_str: String = row.get(18)?;
        let remediation_history = serde_json::from_str(&history_str).unwrap_or_default();

        let app_str: String = row.get(19)?;
        let human_approval_state =
            CaseApprovalState::from_str(&app_str).unwrap_or(CaseApprovalState::None);

        Ok(MaintenanceCase {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            category,
            title: row.get(3)?,
            problem_statement: row.get(4)?,
            dedup_key: row.get(5)?,
            status,
            priority,
            severity,
            confidence_score: row.get(9)?,
            affected_packages,
            affected_files,
            affected_symbols,
            evidence_refs,
            related_issues,
            related_prs,
            workflow_id: row.get(16)?,
            active_plan_id: row.get(17)?,
            remediation_history,
            human_approval_state,
            created_at_epoch_ms: row.get(20)?,
            updated_at_epoch_ms: row.get(21)?,
            last_observed_at_epoch_ms: row.get(22)?,
            resolved_at_epoch_ms: row.get(23)?,
        })
    }

    fn row_to_plan(row: &rusqlite::Row) -> rusqlite::Result<MaintenancePlan> {
        let cat_str: String = row.get(4)?;
        let category =
            MaintenanceCategory::from_str(&cat_str).unwrap_or(MaintenanceCategory::BrokenCi);

        let hyp_str: String = row.get(7)?;
        let hypotheses = serde_json::from_str(&hyp_str).unwrap_or_default();

        let files_str: String = row.get(8)?;
        let files_raw: Vec<String> = serde_json::from_str(&files_str).unwrap_or_default();
        let expected_files = files_raw.into_iter().map(PathBuf::from).collect();

        let pkgs_str: String = row.get(9)?;
        let expected_packages = serde_json::from_str(&pkgs_str).unwrap_or_default();

        let tests_str: String = row.get(10)?;
        let relevant_tests = serde_json::from_str(&tests_str).unwrap_or_default();

        let reqs_str: String = row.get(11)?;
        let verification_requirements = serde_json::from_str(&reqs_str).unwrap_or_default();

        let risk_str: String = row.get(12)?;
        let estimated_risk =
            MaintenanceRiskLevel::from_str(&risk_str).unwrap_or(MaintenanceRiskLevel::Low);

        let tier_raw: i64 = row.get(13)?;
        let required_autonomy_tier = AutonomyTier::from_u8(tier_raw as u8);

        let req_app_raw: i64 = row.get(14)?;
        let requires_approval = req_app_raw != 0;

        let app_str: String = row.get(15)?;
        let approval_state =
            PlanApprovalState::from_str(&app_str).unwrap_or(PlanApprovalState::Pending);

        Ok(MaintenancePlan {
            id: row.get(0)?,
            case_id: row.get(1)?,
            repo_id: row.get(2)?,
            baseline_sha: row.get(3)?,
            category,
            problem_classification: row.get(5)?,
            selected_strategy: row.get(6)?,
            hypotheses,
            expected_files,
            expected_packages,
            relevant_tests,
            verification_requirements,
            estimated_risk,
            required_autonomy_tier,
            requires_approval,
            approval_state,
            workflow_id: row.get(16)?,
            plan_hash: row.get(17)?,
            created_at_epoch_ms: row.get(18)?,
            updated_at_epoch_ms: row.get(19)?,
        })
    }

    fn row_to_execution(row: &rusqlite::Row) -> rusqlite::Result<MaintenanceExecution> {
        let state_str: String = row.get(7)?;
        let execution_state = ExecutionLifecycleState::from_str(&state_str)
            .unwrap_or(ExecutionLifecycleState::Created);

        let dur_raw: i64 = row.get(13)?;
        let tok_raw: i64 = row.get(14)?;

        Ok(MaintenanceExecution {
            id: row.get(0)?,
            case_id: row.get(1)?,
            workflow_id: row.get(2)?,
            plan_id: row.get(3)?,
            manifest_hash: row.get(4)?,
            attempt_id: row.get(5)?,
            worker_id: row.get(6)?,
            execution_state,
            verification_report_hash: row.get(8)?,
            promotion_state: row.get(9)?,
            pr_number: row.get(10)?,
            pr_url: row.get(11)?,
            failure_classification: row.get(12)?,
            duration_ms: dur_raw as u64,
            tokens_used: tok_raw as u64,
            created_at_epoch_ms: row.get(15)?,
            completed_at_epoch_ms: row.get(16)?,
        })
    }

    fn row_to_schedule(row: &rusqlite::Row) -> rusqlite::Result<MaintenanceSchedule> {
        let is_en_raw: i64 = row.get(4)?;
        let override_str: Option<String> = row.get(8)?;
        let policy_override = override_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(MaintenanceSchedule {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            job_kind: row.get(2)?,
            cron_or_interval_secs: row.get(3)?,
            is_enabled: is_en_raw != 0,
            last_run_epoch_ms: row.get(5)?,
            next_run_epoch_ms: row.get(6)?,
            consecutive_failures: row.get(7)?,
            policy_override,
            created_at_epoch_ms: row.get(9)?,
            updated_at_epoch_ms: row.get(10)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_storage::{BUILTIN_MIGRATIONS, MigrationEngine};

    fn setup_test_store() -> MaintenanceStore {
        let mut conn = Connection::open_in_memory().unwrap();
        MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        MaintenanceStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn test_signal_idempotent_ingestion() {
        let store = setup_test_store();
        let sig = MaintenanceSignal::new(
            "repo_abc",
            SignalProvider::GitHub,
            MaintenanceCategory::BrokenCi,
            Some("run_101".to_string()),
            Some("https://github.com/org/repo/actions/runs/101".to_string()),
            1700000000,
            Some("commit_sha_1".to_string()),
            SignalSeverity::High,
            "cargo test failure in tests/integration.rs",
            SignalTrustStatus::Verified,
            serde_json::json!({"job_name": "test-linux"}),
        );

        let inserted = store.record_signal(&sig).unwrap();
        assert!(inserted, "First insertion should succeed");

        // Replayed signal must be idempotently rejected
        let second_inserted = store.record_signal(&sig).unwrap();
        assert!(!second_inserted, "Duplicate signal should be rejected");

        let fetched = store.get_signal(&sig.id).unwrap().unwrap();
        assert_eq!(fetched.id, sig.id);
        assert_eq!(fetched.repo_id, "repo_abc");
    }

    #[test]
    fn test_case_lifecycle_and_plan_persistence() {
        let store = setup_test_store();
        let sig = MaintenanceSignal::new(
            "repo_abc",
            SignalProvider::GitHub,
            MaintenanceCategory::BrokenCi,
            Some("run_102".to_string()),
            None,
            1700000000,
            Some("sha_123".to_string()),
            SignalSeverity::High,
            "test_broken",
            SignalTrustStatus::Verified,
            serde_json::json!({}),
        );
        store.record_signal(&sig).unwrap();

        let mut case = MaintenanceCase::new(
            "repo_abc",
            MaintenanceCategory::BrokenCi,
            "Fix test_broken failure",
            "Broken test on commit sha_123",
            "ci_run_102",
            SignalSeverity::High,
        );

        store.create_case(&case, Some(&sig.id)).unwrap();

        let signals = store.list_signals_for_case(&case.id).unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].id, sig.id);

        // Update state through valid lifecycle transitions
        case.transition_to(CaseStatus::Investigating).unwrap();
        case.transition_to(CaseStatus::Planning).unwrap();
        store.update_case(&case).unwrap();

        let fetched_case = store.get_case(&case.id).unwrap().unwrap();
        assert_eq!(fetched_case.status, CaseStatus::Planning);

        // Save plan
        let plan = MaintenancePlan::new(
            &case.id,
            "repo_abc",
            "sha_123",
            MaintenanceCategory::BrokenCi,
            "test_regression",
            "minimal_test_correction",
            MaintenanceRiskLevel::Low,
            AutonomyTier::Tier2GuardedRemediation,
            false,
        );
        store.save_plan(&plan).unwrap();

        let fetched_plan = store.get_plan_for_case(&case.id).unwrap().unwrap();
        assert_eq!(fetched_plan.id, plan.id);
        assert_eq!(fetched_plan.plan_hash, plan.plan_hash);
    }
}
