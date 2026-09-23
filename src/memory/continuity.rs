//! Session Continuity Engine.
//!
//! Reconstructs active engineering state after process restart, terminal closure,
//! or model session replacement without replaying conversational transcripts.
//!
//! Restores:
//! - Current mission & plan revision
//! - Task partitions (active, completed, failed)
//! - Open review findings & active assumptions
//! - Authoritative architectural decisions
//! - Latest checkpoint and verification obligations

use sqlx::SqlitePool;

use crate::error::M31AError;
use crate::ids::{CheckpointId, MissionId};
use crate::memory::repository::{EngineeringMemoryStore, SqliteEngineeringMemoryRepository};
use crate::memory::types::{
    EngineeringAssumption, EngineeringDecision, ReviewFindingRecord, VerificationRecord,
};
use crate::persistence::sqlite::repositories::{
    SqliteMissionRepository, SqliteTaskGraphRepository, SqliteTaskRepository, TaskGraphRepository,
};
use crate::state::mission::Mission;
use crate::state::task::Task;
use crate::state_machine::TaskState;

/// Reconstructed engineering state preserved across session restarts.
#[derive(Debug, Clone)]
pub struct EngineeringContinuityState {
    pub mission: Mission,
    pub plan_revision: u32,
    pub plan_id: String,
    pub active_task: Option<Task>,
    pub completed_tasks: Vec<Task>,
    pub failed_tasks: Vec<Task>,
    pub pending_tasks: Vec<Task>,
    pub open_findings: Vec<ReviewFindingRecord>,
    pub active_assumptions: Vec<EngineeringAssumption>,
    pub authoritative_decisions: Vec<EngineeringDecision>,
    pub latest_checkpoint_id: Option<CheckpointId>,
    pub verification_records: Vec<VerificationRecord>,
}

impl EngineeringContinuityState {
    /// Render a concise, authoritative continuity briefing for model or operator.
    pub fn format_continuity_briefing(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Engineering Continuity Briefing:\n\
             - Mission: {} (Status: {:?})\n\
             - Objective: {}\n\
             - Current Plan: {} (Revision {})\n\
             - Tasks: {} completed, {} failed, {} pending\n",
            self.mission.id,
            self.mission.status,
            self.mission.objective,
            self.plan_id,
            self.plan_revision,
            self.completed_tasks.len(),
            self.failed_tasks.len(),
            self.pending_tasks.len()
        ));

        if let Some(ref at) = self.active_task {
            out.push_str(&format!(" - Active Task: {} ({})\n", at.id, at.title));
        }

        if !self.authoritative_decisions.is_empty() {
            out.push_str(" - Authoritative Decisions:\n");
            for d in &self.authoritative_decisions {
                out.push_str(&format!("   * [{}] {}: {}\n", d.id, d.title, d.decision));
            }
        }

        if !self.active_assumptions.is_empty() {
            out.push_str(" - Active Assumptions:\n");
            for a in &self.active_assumptions {
                out.push_str(&format!("   * {}\n", a.statement));
            }
        }

        if !self.open_findings.is_empty() {
            out.push_str(" - Open Review Findings:\n");
            for f in &self.open_findings {
                out.push_str(&format!(
                    "   * [{}]: {} ({})\n",
                    f.severity.as_str(),
                    f.description,
                    f.file_path
                ));
            }
        }

        out
    }
}

/// Session continuity engine restoring active engineering state from durable persistence.
pub struct SessionContinuityEngine {
    pool: SqlitePool,
    mission_repo: SqliteMissionRepository,
    task_repo: SqliteTaskRepository,
    graph_repo: SqliteTaskGraphRepository,
    memory_repo: SqliteEngineeringMemoryRepository,
}

impl SessionContinuityEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool: pool.clone(),
            mission_repo: SqliteMissionRepository::new(pool.clone()),
            task_repo: SqliteTaskRepository::new(pool.clone()),
            graph_repo: SqliteTaskGraphRepository::new(pool.clone()),
            memory_repo: SqliteEngineeringMemoryRepository::new(pool),
        }
    }

    /// Restore full engineering continuity for a mission.
    pub async fn restore_session(
        &self,
        mission_id: MissionId,
    ) -> Result<EngineeringContinuityState, M31AError> {
        // 1. Mission Aggregate
        let mission =
            self.mission_repo.get(mission_id).await?.ok_or_else(|| {
                M31AError::validation(format!("Mission '{}' not found", mission_id))
            })?;

        // 2. Plan Revision & Graph
        let (plan_revision, plan_id) = if let Some(graph_id) = mission.task_graph_id {
            if let Some(graph) = self.graph_repo.get_graph_by_id(graph_id).await? {
                (graph.revision, graph.plan_id)
            } else {
                (1, "plan_initial".to_string())
            }
        } else {
            (1, "plan_initial".to_string())
        };

        // 3. Partition Tasks
        let all_tasks = self.task_repo.list_by_mission(mission_id).await?;
        let mut active_task = None;
        let mut completed_tasks = Vec::new();
        let mut failed_tasks = Vec::new();
        let mut pending_tasks = Vec::new();

        for t in all_tasks {
            match t.status {
                TaskState::Running => {
                    active_task = Some(t);
                }
                TaskState::Succeeded => {
                    completed_tasks.push(t);
                }
                TaskState::Failed => {
                    failed_tasks.push(t);
                }
                _ => {
                    pending_tasks.push(t);
                }
            }
        }

        // 4. Memory Entities
        let authoritative_decisions = self
            .memory_repo
            .list_decisions(
                None,
                Some(mission_id),
                Some(crate::memory::types::DecisionStatus::Accepted),
            )
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        let active_assumptions = self
            .memory_repo
            .list_assumptions(
                mission_id,
                None,
                Some(crate::memory::types::AssumptionStatus::Active),
            )
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        let open_findings = self
            .memory_repo
            .list_review_findings(
                mission_id,
                None,
                Some(crate::memory::types::ReviewFindingStatus::Open),
            )
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        let verification_records = self
            .memory_repo
            .list_verification_records(mission_id, None)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        // 5. Latest Checkpoint
        let row = sqlx::query(
            "SELECT id FROM checkpoints WHERE mission_id = ? ORDER BY sequence DESC LIMIT 1",
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        let latest_checkpoint_id = row.and_then(|r| {
            let id_bytes: Vec<u8> = sqlx::Row::get(&r, "id");
            if id_bytes.len() == 16 {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&id_bytes);
                Some(CheckpointId::from_bytes(arr))
            } else {
                None
            }
        });

        Ok(EngineeringContinuityState {
            mission,
            plan_revision,
            plan_id,
            active_task,
            completed_tasks,
            failed_tasks,
            pending_tasks,
            open_findings,
            active_assumptions,
            authoritative_decisions,
            latest_checkpoint_id,
            verification_records,
        })
    }
}
