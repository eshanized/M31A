//! Agent aggregate model

use crate::ids::{AgentId, MissionId, TaskId};
use crate::state_machine::AgentState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent aggregate containing authoritative agent state and execution tracking (AGT-05).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub id: AgentId,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub role: String,
    pub status: AgentState,
    pub profile_fingerprint: String,
    pub max_steps: u32,
    pub steps_consumed: u32,
    pub model_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Agent {
    pub fn new(id: AgentId, mission_id: MissionId, role: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            mission_id,
            task_id: None,
            role,
            status: AgentState::Starting,
            profile_fingerprint: String::new(),
            max_steps: 50,
            steps_consumed: 0,
            model_name: String::new(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    pub fn with_runtime_details(
        mut self,
        task_id: Option<TaskId>,
        profile_fingerprint: String,
        max_steps: u32,
        model_name: String,
    ) -> Self {
        self.task_id = task_id;
        self.profile_fingerprint = profile_fingerprint;
        self.max_steps = max_steps;
        self.model_name = model_name;
        self
    }

    pub fn update_status(&mut self, new_status: AgentState) {
        self.status = new_status;
        self.updated_at = Utc::now();
    }

    pub fn record_step_consumed(&mut self) {
        self.steps_consumed = self.steps_consumed.saturating_add(1);
        self.updated_at = Utc::now();
    }

    pub fn complete(&mut self, status: AgentState) {
        self.status = status;
        let now = Utc::now();
        self.updated_at = now;
        self.completed_at = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{AgentId, MissionId};

    #[test]
    fn test_agent_construction() {
        let id = AgentId::new();
        let mission_id = MissionId::new();
        let agent = Agent::new(id, mission_id, "implementer".to_string());
        assert_eq!(agent.id, id);
        assert_eq!(agent.mission_id, mission_id);
        assert_eq!(agent.role, "implementer");
        assert_eq!(agent.status, AgentState::Starting);
        assert_eq!(agent.steps_consumed, 0);
        assert_eq!(agent.max_steps, 50);
        assert!(agent.completed_at.is_none());
    }

    #[test]
    fn test_agent_status_update() {
        let id = AgentId::new();
        let mission_id = MissionId::new();
        let mut agent = Agent::new(id, mission_id, "test".to_string());
        agent.update_status(AgentState::Running);
        assert_eq!(agent.status, AgentState::Running);

        agent.record_step_consumed();
        assert_eq!(agent.steps_consumed, 1);

        agent.complete(AgentState::Completed);
        assert_eq!(agent.status, AgentState::Completed);
        assert!(agent.completed_at.is_some());
    }

    #[test]
    fn test_agent_serde_roundtrip() {
        let id = AgentId::new();
        let mission_id = MissionId::new();
        let agent = Agent::new(id, mission_id, "test".to_string()).with_runtime_details(
            Some(TaskId::new()),
            "abc123fp".to_string(),
            30,
            "claude-3-7-sonnet".to_string(),
        );
        let json = serde_json::to_string(&agent).unwrap();
        let parsed: Agent = serde_json::from_str(&json).unwrap();
        assert_eq!(agent.id, parsed.id);
        assert_eq!(agent.mission_id, parsed.mission_id);
        assert_eq!(agent.role, parsed.role);
        assert_eq!(agent.status, parsed.status);
        assert_eq!(agent.profile_fingerprint, parsed.profile_fingerprint);
        assert_eq!(agent.max_steps, parsed.max_steps);
        assert_eq!(agent.model_name, parsed.model_name);
    }
}
