//! SQLite persistence repository for human-governed lifecycle state.
//!
//! Stores and loads:
//! - `session_lifecycle_state`: current stage, plan revision, task revision, authorization pointer
//! - `plan_revisions`: versioned plan proposals, author attribution, status
//! - `task_revisions`: versioned candidate task graphs, plan revision bindings
//! - `discovery_questions`: dynamic questions, status, user answers
//! - `execution_authorizations`: explicit operator authorizations, invalidation tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;

use crate::ids::MissionId;
use crate::kernel::plan::{CandidatePlan, CandidateTask};
use crate::planning::review::{
    AuthorizationDecision, ExecutionAuthorization, PlanReviewStatus, PlanRevision,
    RevisionAuthorType, TaskReviewStatus, TaskRevision,
};
use crate::state_machine::error::TransitionError;
use crate::state_machine::lifecycle::{LifecycleEvent, LifecycleStage, transition_lifecycle};
use crate::workflow::genesis::discovery::DynamicQuestion;

/// Canonical lifecycle ownership.
///
/// The durable lifecycle has ONE effective owner: the
/// `PreExecutionCoordinator` (`planning/review.rs`) for pre-execution
/// stages, plus the single StartExecution boundary (runner ReadyToExecute
/// arm + TUI bridge) for the `ExecutionAuthorized → Executing` handoff.
/// `transition_lifecycle` remains the pure transition law; this repository
/// is the durable writer, and [`SqliteLifecycleRepository::save_validated_transition`]
/// is the enforced seam binding the two: every durable write through it is
/// a validated transition, then persistence, then (by the caller) event
/// emission. Direct `save_lifecycle_state` remains for the coordinator's
/// internal stage writes, which already route through `transition_lifecycle`
/// at each call site; new writers MUST use the validated seam.
///
/// Failure taxonomy for the validated durable seam.
#[derive(Debug, thiserror::Error)]
pub enum ValidatedTransitionError {
    #[error("no persisted lifecycle state for session '{0}'")]
    NoPersistedState(String),
    #[error(
        "persisted stage {persisted:?} does not match expected current {expected:?} for session '{session}'"
    )]
    StageMismatch {
        session: String,
        expected: LifecycleStage,
        persisted: LifecycleStage,
    },
    #[error("invalid lifecycle transition: {0}")]
    InvalidTransition(#[from] TransitionError),
    #[error("lifecycle persistence failure: {0}")]
    Persistence(#[from] sqlx::Error),
}

/// Current persisted lifecycle state for a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedLifecycleState {
    pub session_id: String,
    pub stage: LifecycleStage,
    pub plan_revision: u32,
    pub task_revision: u32,
    pub authorization_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Durable record of a dynamic discovery question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedQuestionRecord {
    pub id: Uuid,
    pub session_id: String,
    pub question_id: String,
    pub target_unknown: String,
    pub reason: String,
    pub text: String,
    pub options: Vec<String>,
    pub allow_freeform: bool,
    pub blocking: bool,
    pub status: String,
    pub answer: Option<String>,
    pub answered_by: Option<String>,
    pub answered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// SQLite-backed persistence repository for lifecycle governance.
#[derive(Clone)]
pub struct SqliteLifecycleRepository {
    pool: SqlitePool,
}

impl SqliteLifecycleRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn session_bytes(session_str: &str) -> Vec<u8> {
        if let Ok(u) = Uuid::parse_str(session_str) {
            u.as_bytes().to_vec()
        } else {
            session_str.as_bytes().to_vec()
        }
    }

    // ── Lifecycle State ────────────────────────────────────────────────────────

    /// Validated durable transition seam.
    ///
    /// Canonical invariant:
    /// ```text
    /// input → canonical lifecycle owner → validated transition
    ///       → durable persistence → event
    /// ```
    /// Loads the persisted state, rejects when it does not match
    /// `expected_current` (fail-closed on concurrent or out-of-order
    /// writers), validates `(current, event)` through the pure
    /// `transition_lifecycle` law (no second state machine), persists the
    /// resulting stage with preserved revisions/authorization pointer, and
    /// returns the new persisted state for event emission by the caller.
    pub async fn save_validated_transition(
        &self,
        session_id: &str,
        expected_current: LifecycleStage,
        event: LifecycleEvent,
    ) -> Result<PersistedLifecycleState, ValidatedTransitionError> {
        let current = self
            .load_lifecycle_state(session_id)
            .await?
            .ok_or_else(|| ValidatedTransitionError::NoPersistedState(session_id.to_string()))?;
        if current.stage != expected_current {
            return Err(ValidatedTransitionError::StageMismatch {
                session: session_id.to_string(),
                expected: expected_current,
                persisted: current.stage,
            });
        }
        let next_stage = transition_lifecycle(current.stage, event)?;
        let next = PersistedLifecycleState {
            session_id: session_id.to_string(),
            stage: next_stage,
            plan_revision: current.plan_revision,
            task_revision: current.task_revision,
            authorization_id: current.authorization_id,
            created_at: current.created_at,
            updated_at: Utc::now(),
        };
        self.save_lifecycle_state(&next).await?;
        Ok(next)
    }

    pub async fn save_lifecycle_state(
        &self,
        state: &PersistedLifecycleState,
    ) -> Result<(), sqlx::Error> {
        let sbytes = Self::session_bytes(&state.session_id);
        let stage_str = state.stage.to_string();
        let auth_bytes = state.authorization_id.map(|id| id.as_bytes().to_vec());
        let created_at = state.created_at.to_rfc3339();
        let updated_at = state.updated_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO session_lifecycle_state
                (session_id, stage, plan_revision, task_revision, authorization_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(session_id) DO UPDATE SET
                stage = excluded.stage,
                plan_revision = excluded.plan_revision,
                task_revision = excluded.task_revision,
                authorization_id = excluded.authorization_id,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(sbytes.as_slice())
        .bind(&stage_str)
        .bind(state.plan_revision as i64)
        .bind(state.task_revision as i64)
        .bind(auth_bytes)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_lifecycle_state(
        &self,
        session_id: &str,
    ) -> Result<Option<PersistedLifecycleState>, sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);

        let row = sqlx::query(
            r#"
            SELECT stage, plan_revision, task_revision, authorization_id, created_at, updated_at
            FROM session_lifecycle_state
            WHERE session_id = ?
            "#,
        )
        .bind(sbytes.as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => Ok(None),
            Some(r) => {
                let stage_str: String = r.get("stage");
                // A corrupt persisted stage NEVER becomes a legitimate
                // semantic state. Surface it as a typed persistence-integrity
                // error (fail-closed) instead of silently coercing to
                // IntentActive — same doctrine as the verification-status
                // mapping. Callers treat load errors as unrestorable (no
                // execution from corrupt governance state).
                let stage = LifecycleStage::from_str(&stage_str).map_err(|e| {
                    sqlx::Error::Protocol(format!(
                        "corrupt session_lifecycle_state.stage value '{stage_str}': {e}"
                    ))
                })?;
                let plan_rev: i64 = r.get("plan_revision");
                let task_rev: i64 = r.get("task_revision");
                let auth_bytes: Option<Vec<u8>> = r.get("authorization_id");
                let auth_id = auth_bytes.and_then(|b| {
                    if b.len() == 16 {
                        let mut arr = [0u8; 16];
                        arr.copy_from_slice(&b);
                        Some(Uuid::from_bytes(arr))
                    } else {
                        None
                    }
                });
                let created_str: String = r.get("created_at");
                let updated_str: String = r.get("updated_at");

                Ok(Some(PersistedLifecycleState {
                    session_id: session_id.to_string(),
                    stage,
                    plan_revision: plan_rev as u32,
                    task_revision: task_rev as u32,
                    authorization_id: auth_id,
                    created_at: DateTime::parse_from_rfc3339(&created_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(&updated_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                }))
            }
        }
    }

    // ── Plan Revisions ─────────────────────────────────────────────────────────

    pub async fn save_plan_revision(&self, revision: &PlanRevision) -> Result<(), sqlx::Error> {
        let sbytes = Self::session_bytes(&revision.session_id);
        let id_bytes = revision.id.as_bytes().to_vec();
        let content_json = serde_json::to_string(&revision.content)
            .map_err(|e| sqlx::Error::Protocol(format!("Plan JSON serialization error: {e}")))?;
        let author_type = revision.author_type.to_string();
        let status = format!("{:?}", revision.status).to_lowercase();
        let created_at = revision.created_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO plan_revisions
                (id, session_id, revision, plan_id, content_json, created_by, author_type, supersedes_revision, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(session_id, revision) DO UPDATE SET
                content_json = excluded.content_json,
                status = excluded.status
            "#,
        )
        .bind(id_bytes.as_slice())
        .bind(sbytes.as_slice())
        .bind(revision.revision as i64)
        .bind(&revision.plan_id)
        .bind(&content_json)
        .bind(&revision.created_by)
        .bind(&author_type)
        .bind(revision.supersedes_revision.map(|r| r as i64))
        .bind(&status)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_plan_revisions(
        &self,
        session_id: &str,
    ) -> Result<Vec<PlanRevision>, sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);

        let rows = sqlx::query(
            r#"
            SELECT id, revision, plan_id, content_json, created_by, author_type, supersedes_revision, status, created_at
            FROM plan_revisions
            WHERE session_id = ?
            ORDER BY revision ASC
            "#,
        )
        .bind(sbytes.as_slice())
        .fetch_all(&self.pool)
        .await?;

        let mut list = Vec::new();
        for r in rows {
            let id_bytes: Vec<u8> = r.get("id");
            // Fabricated identities fail closed.
            if id_bytes.len() != 16 {
                return Err(sqlx::Error::Protocol(format!(
                    "corrupt plan revision identity: expected 16 bytes, got {}",
                    id_bytes.len()
                )));
            }
            let mut arr = [0u8; 16];
            arr.copy_from_slice(&id_bytes);
            let id = Uuid::from_bytes(arr);
            let rev: i64 = r.get("revision");
            let plan_id: String = r.get("plan_id");
            let content_json: String = r.get("content_json");
            let content: CandidatePlan = serde_json::from_str(&content_json).map_err(|e| {
                sqlx::Error::Protocol(format!("Plan JSON deserialization error: {e}"))
            })?;
            let created_by: String = r.get("created_by");
            let author_type_str: String = r.get("author_type");
            // Unknown authorship fails closed with typed corruption instead of
            // silently becoming Runtime (privilege misattribution). The writer
            // emits only user/model/runtime.
            let author_type = match author_type_str.as_str() {
                "user" => RevisionAuthorType::User,
                "model" => RevisionAuthorType::Model,
                "runtime" => RevisionAuthorType::Runtime,
                other => {
                    return Err(sqlx::Error::Protocol(format!(
                        "corrupt plan revision author_type '{other}'"
                    )));
                }
            };
            let supersedes: Option<i64> = r.get("supersedes_revision");
            let status_str: String = r.get("status");
            let status = match status_str.as_str() {
                "accepted" => PlanReviewStatus::Accepted,
                "rejected" => PlanReviewStatus::Rejected,
                "superseded" => PlanReviewStatus::Superseded,
                "inreview" | "in_review" => PlanReviewStatus::InReview,
                "draft" => PlanReviewStatus::Draft,
                other => {
                    return Err(sqlx::Error::Protocol(format!(
                        "corrupt plan revision status '{other}'"
                    )));
                }
            };
            let created_str: String = r.get("created_at");

            list.push(PlanRevision {
                id,
                session_id: session_id.to_string(),
                revision: rev as u32,
                plan_id,
                content,
                created_by,
                author_type,
                supersedes_revision: supersedes.map(|s| s as u32),
                status,
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| {
                        sqlx::Error::Protocol(format!("corrupt plan revision created_at: {e}"))
                    })?,
            });
        }

        Ok(list)
    }

    pub async fn load_latest_plan_revision(
        &self,
        session_id: &str,
    ) -> Result<Option<PlanRevision>, sqlx::Error> {
        let revs = self.load_plan_revisions(session_id).await?;
        Ok(revs.into_iter().max_by_key(|r| r.revision))
    }

    /// Load an exact plan revision by revision number (INVARIANT 8).
    pub async fn load_plan_revision(
        &self,
        session_id: &str,
        revision: u32,
    ) -> Result<Option<PlanRevision>, sqlx::Error> {
        let revs = self.load_plan_revisions(session_id).await?;
        Ok(revs.into_iter().find(|r| r.revision == revision))
    }

    pub async fn update_plan_revision_status(
        &self,
        session_id: &str,
        revision: u32,
        status: PlanReviewStatus,
    ) -> Result<(), sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);
        let status_str = format!("{:?}", status).to_lowercase();

        sqlx::query("UPDATE plan_revisions SET status = ? WHERE session_id = ? AND revision = ?")
            .bind(&status_str)
            .bind(sbytes.as_slice())
            .bind(revision as i64)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ── Task Revisions ─────────────────────────────────────────────────────────

    pub async fn save_task_revision(&self, revision: &TaskRevision) -> Result<(), sqlx::Error> {
        let sbytes = Self::session_bytes(&revision.session_id);
        let id_bytes = revision.id.as_bytes().to_vec();
        let tasks_json = serde_json::to_string(&revision.tasks)
            .map_err(|e| sqlx::Error::Protocol(format!("Task JSON serialization error: {e}")))?;
        let author_type = revision.author_type.to_string();
        let status = format!("{:?}", revision.status).to_lowercase();
        let created_at = revision.created_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO task_revisions
                (id, session_id, revision, plan_revision, tasks_json, created_by, author_type, supersedes_revision, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(session_id, revision) DO UPDATE SET
                tasks_json = excluded.tasks_json,
                status = excluded.status
            "#,
        )
        .bind(id_bytes.as_slice())
        .bind(sbytes.as_slice())
        .bind(revision.revision as i64)
        .bind(revision.plan_revision as i64)
        .bind(&tasks_json)
        .bind(&revision.created_by)
        .bind(&author_type)
        .bind(revision.supersedes_revision.map(|r| r as i64))
        .bind(&status)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_task_revisions(
        &self,
        session_id: &str,
    ) -> Result<Vec<TaskRevision>, sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);

        let rows = sqlx::query(
            r#"
            SELECT id, revision, plan_revision, tasks_json, created_by, author_type, supersedes_revision, status, created_at
            FROM task_revisions
            WHERE session_id = ?
            ORDER BY revision ASC
            "#,
        )
        .bind(sbytes.as_slice())
        .fetch_all(&self.pool)
        .await?;

        let mut list = Vec::new();
        for r in rows {
            let id_bytes: Vec<u8> = r.get("id");
            // Fabricated identities fail closed (never mint a fresh UUID for
            // a corrupt row).
            if id_bytes.len() != 16 {
                return Err(sqlx::Error::Protocol(format!(
                    "corrupt task revision identity: expected 16 bytes, got {}",
                    id_bytes.len()
                )));
            }
            let mut arr = [0u8; 16];
            arr.copy_from_slice(&id_bytes);
            let id = Uuid::from_bytes(arr);
            let rev: i64 = r.get("revision");
            let plan_rev: i64 = r.get("plan_revision");
            let tasks_json: String = r.get("tasks_json");
            let tasks: Vec<CandidateTask> = serde_json::from_str(&tasks_json).map_err(|e| {
                sqlx::Error::Protocol(format!("Task JSON deserialization error: {e}"))
            })?;
            let created_by: String = r.get("created_by");
            let author_type_str: String = r.get("author_type");
            let author_type = match author_type_str.as_str() {
                "user" => RevisionAuthorType::User,
                "model" => RevisionAuthorType::Model,
                "runtime" => RevisionAuthorType::Runtime,
                other => {
                    return Err(sqlx::Error::Protocol(format!(
                        "corrupt task revision author_type '{other}'"
                    )));
                }
            };
            let supersedes: Option<i64> = r.get("supersedes_revision");
            let status_str: String = r.get("status");
            let status = match status_str.as_str() {
                "accepted" => TaskReviewStatus::Accepted,
                "rejected" => TaskReviewStatus::Rejected,
                "superseded" => TaskReviewStatus::Superseded,
                "inreview" | "in_review" => TaskReviewStatus::InReview,
                "draft" => TaskReviewStatus::Draft,
                other => {
                    return Err(sqlx::Error::Protocol(format!(
                        "corrupt task revision status '{other}'"
                    )));
                }
            };
            let created_str: String = r.get("created_at");

            list.push(TaskRevision {
                id,
                session_id: session_id.to_string(),
                revision: rev as u32,
                plan_revision: plan_rev as u32,
                tasks,
                created_by,
                author_type,
                supersedes_revision: supersedes.map(|s| s as u32),
                status,
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| {
                        sqlx::Error::Protocol(format!("corrupt task revision created_at: {e}"))
                    })?,
            });
        }

        Ok(list)
    }

    pub async fn load_latest_task_revision(
        &self,
        session_id: &str,
    ) -> Result<Option<TaskRevision>, sqlx::Error> {
        let revs = self.load_task_revisions(session_id).await?;
        Ok(revs.into_iter().max_by_key(|r| r.revision))
    }

    /// Load an exact task revision by revision number (INVARIANT 8).
    pub async fn load_task_revision(
        &self,
        session_id: &str,
        revision: u32,
    ) -> Result<Option<TaskRevision>, sqlx::Error> {
        let revs = self.load_task_revisions(session_id).await?;
        Ok(revs.into_iter().find(|r| r.revision == revision))
    }

    pub async fn update_task_revision_status(
        &self,
        session_id: &str,
        revision: u32,
        status: TaskReviewStatus,
    ) -> Result<(), sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);
        let status_str = format!("{:?}", status).to_lowercase();

        sqlx::query("UPDATE task_revisions SET status = ? WHERE session_id = ? AND revision = ?")
            .bind(&status_str)
            .bind(sbytes.as_slice())
            .bind(revision as i64)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ── Discovery Questions ────────────────────────────────────────────────────

    pub async fn save_discovery_question(
        &self,
        session_id: &str,
        question: &DynamicQuestion,
    ) -> Result<(), sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);
        let qid = Uuid::now_v7();
        let options_json = serde_json::to_string(&question.options).unwrap_or_default();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO discovery_questions
                (id, session_id, question_id, target_unknown, reason, text, options_json, allow_freeform, blocking, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)
            "#,
        )
        .bind(qid.as_bytes().as_slice())
        .bind(sbytes.as_slice())
        .bind(&question.question_id)
        .bind(&question.target_unknown)
        .bind(&question.reason)
        .bind(&question.text)
        .bind(&options_json)
        .bind(if question.allow_freeform { 1 } else { 0 })
        .bind(if question.blocking { 1 } else { 0 })
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn record_question_answer(
        &self,
        session_id: &str,
        question_id: &str,
        answer: &str,
        answered_by: &str,
    ) -> Result<(), sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);
        let now = Utc::now().to_rfc3339();

        let res = sqlx::query(
            r#"
            UPDATE discovery_questions
            SET status = 'answered', answer = ?, answered_by = ?, answered_at = ?
            WHERE session_id = ? AND question_id = ? AND status = 'pending'
            "#,
        )
        .bind(answer)
        .bind(answered_by)
        .bind(&now)
        .bind(sbytes.as_slice())
        .bind(question_id)
        .execute(&self.pool)
        .await?;

        if res.rows_affected() != 1 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    pub async fn load_discovery_questions(
        &self,
        session_id: &str,
    ) -> Result<Vec<PersistedQuestionRecord>, sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);

        let rows = sqlx::query(
            r#"
            SELECT id, question_id, target_unknown, reason, text, options_json, allow_freeform, blocking, status, answer, answered_by, answered_at, created_at
            FROM discovery_questions
            WHERE session_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(sbytes.as_slice())
        .fetch_all(&self.pool)
        .await?;

        let mut list = Vec::new();
        for r in rows {
            let id_bytes: Vec<u8> = r.get("id");
            let id = if id_bytes.len() == 16 {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&id_bytes);
                Uuid::from_bytes(arr)
            } else {
                Uuid::now_v7()
            };
            let question_id: String = r.get("question_id");
            let target_unknown: String = r.get("target_unknown");
            let reason: String = r.get("reason");
            let text: String = r.get("text");
            let options_json: String = r.get("options_json");
            let options: Vec<String> = serde_json::from_str(&options_json).unwrap_or_default();
            let allow_freeform: i64 = r.get("allow_freeform");
            let blocking: i64 = r.get("blocking");
            let status: String = r.get("status");
            let answer: Option<String> = r.get("answer");
            let answered_by: Option<String> = r.get("answered_by");
            let answered_at_str: Option<String> = r.get("answered_at");
            let answered_at = answered_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });
            let created_str: String = r.get("created_at");

            list.push(PersistedQuestionRecord {
                id,
                session_id: session_id.to_string(),
                question_id,
                target_unknown,
                reason,
                text,
                options,
                allow_freeform: allow_freeform != 0,
                blocking: blocking != 0,
                status,
                answer,
                answered_by,
                answered_at,
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            });
        }

        Ok(list)
    }

    // ── Execution Authorization ────────────────────────────────────────────────

    pub async fn save_execution_authorization(
        &self,
        auth: &ExecutionAuthorization,
    ) -> Result<(), sqlx::Error> {
        let sbytes = Self::session_bytes(&auth.session_id);
        let id_bytes = auth.id.as_bytes().to_vec();
        let decision_str = format!("{:?}", auth.decision).to_lowercase();
        let authorized_at = auth.authorized_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO execution_authorizations
                (id, session_id, plan_revision, task_revision, decision, authorized_by, authorized_at, invalidation_reason, plan_content_hash, task_content_hash)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id_bytes.as_slice())
        .bind(sbytes.as_slice())
        .bind(auth.plan_revision as i64)
        .bind(auth.task_revision as i64)
        .bind(&decision_str)
        .bind(&auth.authorized_by)
        .bind(&authorized_at)
        .bind(&auth.invalidation_reason)
        .bind(&auth.plan_content_hash)
        .bind(&auth.task_content_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_latest_execution_authorization(
        &self,
        session_id: &str,
    ) -> Result<Option<ExecutionAuthorization>, sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);

        let row = sqlx::query(
            r#"
            SELECT id, plan_revision, task_revision, decision, authorized_by, authorized_at, invalidation_reason, plan_content_hash, task_content_hash
            FROM execution_authorizations
            WHERE session_id = ?
            ORDER BY authorized_at DESC
            LIMIT 1
            "#,
        )
        .bind(sbytes.as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => Ok(None),
            Some(r) => {
                let id_bytes: Vec<u8> = r.get("id");
                // Fabricated authorization identities fail closed.
                if id_bytes.len() != 16 {
                    return Err(sqlx::Error::Protocol(format!(
                        "corrupt execution authorization identity: expected 16 bytes, got {}",
                        id_bytes.len()
                    )));
                }
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&id_bytes);
                let id = Uuid::from_bytes(arr);
                let plan_rev: i64 = r.get("plan_revision");
                let task_rev: i64 = r.get("task_revision");
                let decision_str: String = r.get("decision");
                let decision = match decision_str.as_str() {
                    "authorized" => AuthorizationDecision::Authorized,
                    "rejected" => AuthorizationDecision::Rejected,
                    "invalidated" => AuthorizationDecision::Invalidated,
                    other => {
                        return Err(sqlx::Error::Protocol(format!(
                            "corrupt execution authorization decision '{other}'"
                        )));
                    }
                };
                let authorized_by: String = r.get("authorized_by");
                let authorized_at_str: String = r.get("authorized_at");
                let invalidation_reason: Option<String> = r.get("invalidation_reason");
                let plan_content_hash: Option<String> =
                    r.try_get("plan_content_hash").ok().flatten();
                let task_content_hash: Option<String> =
                    r.try_get("task_content_hash").ok().flatten();

                Ok(Some(ExecutionAuthorization {
                    id,
                    session_id: session_id.to_string(),
                    plan_revision: plan_rev as u32,
                    task_revision: task_rev as u32,
                    decision,
                    authorized_by,
                    authorized_at: DateTime::parse_from_rfc3339(&authorized_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| {
                            sqlx::Error::Protocol(format!(
                                "corrupt execution authorization timestamp: {e}"
                            ))
                        })?,
                    invalidation_reason,
                    plan_content_hash,
                    task_content_hash,
                }))
            }
        }
    }

    pub async fn invalidate_authorizations(
        &self,
        session_id: &str,
        reason: &str,
    ) -> Result<(), sqlx::Error> {
        let sbytes = Self::session_bytes(session_id);

        sqlx::query(
            r#"
            UPDATE execution_authorizations
            SET decision = 'invalidated', invalidation_reason = ?
            WHERE session_id = ? AND decision = 'authorized'
            "#,
        )
        .bind(reason)
        .bind(sbytes.as_slice())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

// ── Resume authorization revalidation ─────────────────────────────────────────

/// Verdict of resume-time execution-authorization revalidation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeAuthVerdict {
    /// A live, exactly-bound authorization governs this mission: resume may
    /// proceed under it.
    Valid {
        session_id: String,
        authorization_id: uuid::Uuid,
        plan_revision: u32,
        task_revision: u32,
    },
    /// No governed execution to revalidate (headless/internal lane without a
    /// session, or a lifecycle stage that never entered execution). Resume
    /// proceeds under non-governed semantics; nothing executing is replayed
    /// under a stale grant because no grant exists.
    NotRequired { reason: String },
}

/// Typed resume-revalidation failures. Every variant halts resume; resume
/// never replays execution on a stale, corrupt, or unauthorized binding.
#[derive(Debug, thiserror::Error)]
pub enum ResumeAuthError {
    #[error("resume halted: {0}")]
    Stale(String),
    #[error("resume halted: unauthorized: {0}")]
    Unauthorized(String),
    #[error("resume halted: corrupt durable record: {0}")]
    Corrupt(String),
    #[error("resume persistence failure: {0}")]
    Persistence(#[from] sqlx::Error),
}

impl SqliteLifecycleRepository {
    /// Find the governed session string for a mission, if any.
    async fn session_for_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            "SELECT id FROM sessions WHERE active_mission_id = ? OR mission_id = ? LIMIT 1",
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;
        match row {
            None => Ok(None),
            Some((id_bytes,)) => {
                if id_bytes.len() != 16 {
                    return Err(sqlx::Error::Protocol(
                        "corrupt session identity length".to_string(),
                    ));
                }
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&id_bytes);
                Ok(Some(Uuid::from_bytes(arr).to_string()))
            }
        }
    }

    /// Revalidate the execution authorization before resuming a mission.
    ///
    /// ```text
    /// restart
    ///   ↓ load mission session
    ///   ↓ load lifecycle (must be ExecutionAuthorized | Executing)
    ///   ↓ load current plan/task revisions
    ///   ↓ load latest execution authorization
    ///   ↓ verify decision == Authorized, revisions bind, exact hashes bind
    ///   ↓ ONLY THEN resume
    /// ```
    ///
    /// Any binding failure halts resume (`Stale` / `Unauthorized` /
    /// `Corrupt`). Pre-execution stages and session-less (headless) lanes
    /// return `NotRequired` instead of replaying execution.
    pub async fn revalidate_authorization_for_resume(
        &self,
        mission_id: MissionId,
    ) -> Result<ResumeAuthVerdict, ResumeAuthError> {
        let Some(session_id) = self
            .session_for_mission(mission_id)
            .await
            .map_err(ResumeAuthError::Persistence)?
        else {
            return Ok(ResumeAuthVerdict::NotRequired {
                reason: "no governed session for mission (headless/internal lane)".to_string(),
            });
        };

        let lifecycle = self
            .load_lifecycle_state(&session_id)
            .await
            .map_err(ResumeAuthError::Persistence)?;
        let Some(state) = lifecycle else {
            return Ok(ResumeAuthVerdict::NotRequired {
                reason: "no persisted lifecycle state: nothing executing to resume".to_string(),
            });
        };
        if !matches!(
            state.stage,
            LifecycleStage::ExecutionAuthorized | LifecycleStage::Executing
        ) {
            return Err(ResumeAuthError::Stale(format!(
                "lifecycle stage {:?} is not resumable execution state",
                state.stage
            )));
        }

        let plan_rev = self
            .load_latest_plan_revision(&session_id)
            .await
            .map_err(ResumeAuthError::Persistence)?
            .ok_or_else(|| {
                ResumeAuthError::Stale("executing session has no plan revision".to_string())
            })?;
        let task_rev = self
            .load_latest_task_revision(&session_id)
            .await
            .map_err(ResumeAuthError::Persistence)?
            .ok_or_else(|| {
                ResumeAuthError::Stale("executing session has no task revision".to_string())
            })?;
        let auth = self
            .load_latest_execution_authorization(&session_id)
            .await
            .map_err(ResumeAuthError::Persistence)?
            .ok_or_else(|| {
                ResumeAuthError::Stale(
                    "executing session has no execution authorization".to_string(),
                )
            })?;

        if auth.decision == AuthorizationDecision::Rejected {
            return Err(ResumeAuthError::Unauthorized(
                "latest execution authorization was rejected".to_string(),
            ));
        }
        if auth.decision != AuthorizationDecision::Authorized {
            return Err(ResumeAuthError::Stale(format!(
                "latest execution authorization is not Authorized ({:?})",
                auth.decision
            )));
        }
        if auth.invalidation_reason.is_some() {
            return Err(ResumeAuthError::Stale(
                "execution authorization carries an invalidation".to_string(),
            ));
        }
        if auth.plan_revision != plan_rev.revision || auth.task_revision != task_rev.revision {
            return Err(ResumeAuthError::Stale(format!(
                "authorization binds plan rev {} / task rev {} but current is {} / {}",
                auth.plan_revision, auth.task_revision, plan_rev.revision, task_rev.revision
            )));
        }
        let Some(expected_plan_hash) = auth.plan_content_hash.clone() else {
            return Err(ResumeAuthError::Stale(
                "authorization lacks a bound plan content hash".to_string(),
            ));
        };
        let Some(expected_task_hash) = auth.task_content_hash.clone() else {
            return Err(ResumeAuthError::Stale(
                "authorization lacks a bound task content hash".to_string(),
            ));
        };
        let actual_plan_hash = PlanRevision::compute_content_hash(&plan_rev.content);
        if actual_plan_hash != expected_plan_hash {
            return Err(ResumeAuthError::Stale(
                "current plan content hash does not match the authorized hash".to_string(),
            ));
        }
        let actual_task_hash = TaskRevision::compute_tasks_hash(&task_rev.tasks);
        if actual_task_hash != expected_task_hash {
            return Err(ResumeAuthError::Stale(
                "current task content hash does not match the authorized hash".to_string(),
            ));
        }

        Ok(ResumeAuthVerdict::Valid {
            session_id,
            authorization_id: auth.id,
            plan_revision: auth.plan_revision,
            task_revision: auth.task_revision,
        })
    }
}
