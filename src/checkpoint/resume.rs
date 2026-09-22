//! Safe resume engine and task completion preservation (CHK-05, D-16).
//!
//! Enforces:
//! - Preservation of completed tasks (`TaskState::Succeeded`) whose repository files and verified artifacts match the checkpoint baseline.
//! - Targeted invalidation of only drifted, missing-artifact, or broken tasks to `TaskState::Pending`.
//! - Rescheduling of in-flight (`TaskState::Running`) tasks to `TaskState::Pending` for clean dispatch.
//! - Avoids blind full-graph re-executions.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::checkpoint::manifest::CheckpointManifest;
use crate::ids::{ArtifactId, MissionId, TaskId};
use crate::persistence::artifacts::fs_store::ArtifactStore;
use crate::repo::drift::RepositoryBaseline;
use crate::state_machine::TaskState;

/// Structured report of task completion preservation and invalidation on resume (CHK-05, D-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeResumeReport {
    pub mission_id: MissionId,
    pub preserved_tasks: HashSet<TaskId>,
    pub invalidated_tasks: HashSet<TaskId>,
    pub rescheduled_tasks: HashSet<TaskId>,
    pub summary: String,
}

/// Errors occurring during safe resume reconciliation.
#[derive(Debug, thiserror::Error)]
pub enum SafeResumeError {
    #[error("Database error during resume: {0}")]
    Database(#[from] sqlx::Error),
    #[error("I/O error during resume: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Safe resume engine preserving verified completed work (CHK-05, D-16).
pub struct SafeResumeEngine {
    pool: SqlitePool,
    artifact_store: Arc<dyn ArtifactStore>,
    workspace_root: PathBuf,
    task_output_files: HashMap<TaskId, Vec<String>>,
}

impl SafeResumeEngine {
    /// Create a new SafeResumeEngine instance.
    pub fn new(
        pool: SqlitePool,
        artifact_store: Arc<dyn ArtifactStore>,
        workspace_root: impl AsRef<Path>,
    ) -> Self {
        Self {
            pool,
            artifact_store,
            workspace_root: workspace_root.as_ref().to_path_buf(),
            task_output_files: HashMap::new(),
        }
    }

    /// Associate a relative output file path with a specific task ID.
    pub fn with_task_output_file(mut self, task_id: TaskId, path: impl Into<String>) -> Self {
        self.task_output_files
            .entry(task_id)
            .or_default()
            .push(path.into());
        self
    }

    /// Associate an output file path on an existing engine instance.
    pub fn associate_task_output_file(&mut self, task_id: TaskId, path: impl Into<String>) {
        self.task_output_files
            .entry(task_id)
            .or_default()
            .push(path.into());
    }

    /// Execute safe resume reconciliation for a mission from a checkpoint manifest (D-16).
    pub async fn resume_mission(
        &self,
        mission_id: MissionId,
        manifest: &CheckpointManifest,
    ) -> Result<SafeResumeReport, SafeResumeError> {
        let mut preserved_tasks = HashSet::new();
        let mut invalidated_tasks = HashSet::new();
        let mut rescheduled_tasks = HashSet::new();
        let task_repo =
            crate::persistence::sqlite::repositories::SqliteTaskRepository::new(self.pool.clone());

        // Load recorded baseline for mission
        let saved_baseline =
            RepositoryBaseline::load_latest_for_mission(&self.pool, mission_id).await?;

        for (&task_id, &recorded_state) in &manifest.task_states {
            match recorded_state {
                TaskState::Succeeded => {
                    let mut is_valid = true;

                    // 1. Verify task's external verification evidence artifacts exist in store.
                    // Only `passed` checks count as evidence. A failed or
                    // blocked check with an attached artifact must never
                    // preserve a Succeeded task across restart.
                    let check_rows = sqlx::query(
                        r#"
                        SELECT evidence_artifact_id
                        FROM verification_checks
                        WHERE task_id = ? AND status = 'passed' AND evidence_artifact_id IS NOT NULL
                        "#,
                    )
                    .bind(task_id.as_bytes().as_slice())
                    .fetch_all(&self.pool)
                    .await?;

                    for row in check_rows {
                        if let Some(artifact_id_bytes) =
                            row.get::<Option<Vec<u8>>, _>("evidence_artifact_id")
                        {
                            let mut art_bytes = [0u8; 16];
                            if artifact_id_bytes.len() == 16 {
                                art_bytes.copy_from_slice(&artifact_id_bytes);
                                let aid = ArtifactId::from_bytes(art_bytes);

                                let mut exists = false;
                                for ext in ["bin", "txt", "json", "log"] {
                                    if self.artifact_store.retrieve(aid, ext).await.is_ok() {
                                        exists = true;
                                        break;
                                    }
                                }

                                if !exists {
                                    is_valid = false;
                                    break;
                                }
                            }
                        }
                    }

                    // 2. Verify workspace files touched by the task match the baseline
                    if is_valid && let Some(files) = self.task_output_files.get(&task_id) {
                        for rel_path in files {
                            let full_path = self.workspace_root.join(rel_path);
                            if !full_path.exists() {
                                is_valid = false;
                                break;
                            }

                            if let Ok(content) = tokio::fs::read(&full_path).await {
                                let mut hasher = Sha256::new();
                                hasher.update(&content);
                                let current_hash = format!("{:x}", hasher.finalize());

                                if let Some(ref baseline) = saved_baseline
                                    && let Some(base_hash) = baseline.file_hashes.get(rel_path)
                                    && base_hash != &current_hash
                                {
                                    is_valid = false;
                                    break;
                                }
                            } else {
                                is_valid = false;
                                break;
                            }
                        }
                    }

                    // 3. Preserve valid tasks, invalidate drifted tasks (CHK-05, D-16)
                    if is_valid {
                        preserved_tasks.insert(task_id);
                        task_repo
                            .update_status(task_id, TaskState::Succeeded)
                            .await
                            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
                    } else {
                        invalidated_tasks.insert(task_id);
                        task_repo
                            .reset_task_to_pending(task_id)
                            .await
                            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
                    }
                }

                TaskState::Running => {
                    // Running tasks from crashed runs transition to Pending for clean dispatch
                    rescheduled_tasks.insert(task_id);
                    task_repo
                        .update_status(task_id, TaskState::Pending)
                        .await
                        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
                }

                TaskState::Pending | TaskState::Blocked | TaskState::Ready => {
                    rescheduled_tasks.insert(task_id);
                    task_repo
                        .update_status(task_id, TaskState::Pending)
                        .await
                        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
                }

                _ => {
                    // Terminal states (Failed, Cancelled, Skipped) remain intact
                }
            }
        }

        let summary = format!(
            "Safe resume reconciled: {} preserved, {} invalidated, {} rescheduled",
            preserved_tasks.len(),
            invalidated_tasks.len(),
            rescheduled_tasks.len()
        );

        Ok(SafeResumeReport {
            mission_id,
            preserved_tasks,
            invalidated_tasks,
            rescheduled_tasks,
            summary,
        })
    }
}
