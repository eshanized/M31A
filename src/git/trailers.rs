//! RFC-compliant Git commit trailers for provenance and attribution (GST-03, D-02, D-03).

use std::str::FromStr;

use crate::git::GitError;
use crate::ids::{MissionId, TaskId};
use crate::state_machine::agent::AgentRole;

/// RFC 2822 / git-interpret-trailers commit trailers for M31A attribution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommitTrailers {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub agent_role: AgentRole,
    pub model_id: String,
    pub verification_run_id: Option<String>,
}

impl CommitTrailers {
    pub fn new(
        mission_id: MissionId,
        task_id: TaskId,
        agent_role: AgentRole,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            mission_id,
            task_id,
            agent_role,
            model_id: model_id.into(),
            verification_run_id: None,
        }
    }

    pub fn with_verification(mut self, run_id: impl Into<String>) -> Self {
        self.verification_run_id = Some(run_id.into());
        self
    }

    /// Formats trailers block according to RFC 2822 standard.
    pub fn format_trailers(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("M31A-Mission: {}", self.mission_id));
        lines.push(format!("M31A-Task: {}", self.task_id));
        lines.push(format!("M31A-Agent: {}", self.agent_role));
        lines.push(format!("M31A-Model: {}", self.model_id));
        if let Some(ref v) = self.verification_run_id {
            lines.push(format!("M31A-Verification: {}", v));
        }
        lines.join("\n")
    }

    /// Embeds trailers into an existing commit message with a preceding blank line.
    pub fn embed_trailers(commit_msg: &str, trailers: &CommitTrailers) -> Result<String, GitError> {
        let formatted = trailers.format_trailers();
        let trimmed = commit_msg.trim_end();
        if trimmed.is_empty() {
            return Ok(formatted);
        }
        Ok(format!("{trimmed}\n\n{formatted}\n"))
    }

    /// Parses M31A trailers from a commit message.
    pub fn parse_trailers(commit_msg: &str) -> Result<CommitTrailers, GitError> {
        let mut mission_id: Option<MissionId> = None;
        let mut task_id: Option<TaskId> = None;
        let mut agent_role: Option<AgentRole> = None;
        let mut model_id: Option<String> = None;
        let mut verification_run_id: Option<String> = None;

        for line in commit_msg.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("M31A-Mission:") {
                let id_str = val.trim();
                let m_id = MissionId::from_str(id_str).map_err(|e| {
                    GitError::Attribution(format!("Invalid MissionId in trailer: {e}"))
                })?;
                mission_id = Some(m_id);
            } else if let Some(val) = line.strip_prefix("M31A-Task:") {
                let id_str = val.trim();
                let t_id = TaskId::from_str(id_str).map_err(|e| {
                    GitError::Attribution(format!("Invalid TaskId in trailer: {e}"))
                })?;
                task_id = Some(t_id);
            } else if let Some(val) = line.strip_prefix("M31A-Agent:") {
                let role_str = val.trim();
                let role = AgentRole::from_str(role_str).map_err(|e| {
                    GitError::Attribution(format!("Invalid AgentRole in trailer: {e}"))
                })?;
                agent_role = Some(role);
            } else if let Some(val) = line.strip_prefix("M31A-Model:") {
                model_id = Some(val.trim().to_string());
            } else if let Some(val) = line.strip_prefix("M31A-Verification:") {
                verification_run_id = Some(val.trim().to_string());
            }
        }

        let mission_id = mission_id
            .ok_or_else(|| GitError::Attribution("Missing M31A-Mission trailer".into()))?;
        let task_id =
            task_id.ok_or_else(|| GitError::Attribution("Missing M31A-Task trailer".into()))?;
        let agent_role =
            agent_role.ok_or_else(|| GitError::Attribution("Missing M31A-Agent trailer".into()))?;
        let model_id =
            model_id.ok_or_else(|| GitError::Attribution("Missing M31A-Model trailer".into()))?;

        Ok(CommitTrailers {
            mission_id,
            task_id,
            agent_role,
            model_id,
            verification_run_id,
        })
    }
}
