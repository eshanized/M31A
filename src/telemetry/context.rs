//! Distributed & Hierarchical Correlation Context (OBS-01, D-03).
//!
//! Provides W3C-compatible trace and span identifiers coupled with
//! strongly typed M31A domain entity references (mission, task, agent, job, tool).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ids::{AgentId, JobId, MissionId, SessionId, TaskId};

use std::sync::atomic::{AtomicU64, Ordering};

static SPAN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a 32-character lowercase hex string for W3C trace ID.
fn generate_trace_id() -> String {
    Uuid::now_v7().simple().to_string()
}

/// Generate a 16-character lowercase hex string for W3C span ID.
fn generate_span_id() -> String {
    let count = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let rand_part = &Uuid::now_v7().simple().to_string()[24..32];
    format!("{:08x}{}", count as u32, rand_part)
}

/// Strongly typed correlation context propagated across execution boundaries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorrelationContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub mission_id: Option<MissionId>,
    pub session_id: Option<SessionId>,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
    pub job_id: Option<JobId>,
    pub tool_call_id: Option<String>,
}

impl CorrelationContext {
    /// Create a new root correlation context bound to a mission.
    pub fn new_root(mission_id: MissionId) -> Self {
        Self {
            trace_id: generate_trace_id(),
            span_id: generate_span_id(),
            parent_span_id: None,
            mission_id: Some(mission_id),
            session_id: None,
            task_id: None,
            agent_id: None,
            job_id: None,
            tool_call_id: None,
        }
    }

    /// Derive a child correlation context representing a nested operation span.
    ///
    /// Preserves trace ID, sets parent span ID to this span's ID, and generates a new span ID.
    pub fn child_span(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: generate_span_id(),
            parent_span_id: Some(self.span_id.clone()),
            mission_id: self.mission_id,
            session_id: self.session_id,
            task_id: self.task_id,
            agent_id: self.agent_id,
            job_id: self.job_id,
            tool_call_id: self.tool_call_id.clone(),
        }
    }

    /// Attach task scope to context.
    pub fn with_task(&self, task_id: TaskId) -> Self {
        let mut ctx = self.clone();
        ctx.task_id = Some(task_id);
        ctx
    }

    /// Attach agent scope to context.
    pub fn with_agent(&self, agent_id: AgentId) -> Self {
        let mut ctx = self.clone();
        ctx.agent_id = Some(agent_id);
        ctx
    }

    /// Attach tool call scope to context.
    pub fn with_tool_call(&self, call_id: impl Into<String>) -> Self {
        let mut ctx = self.clone();
        ctx.tool_call_id = Some(call_id.into());
        ctx
    }

    /// Attach job scope to context.
    pub fn with_job(&self, job_id: JobId) -> Self {
        let mut ctx = self.clone();
        ctx.job_id = Some(job_id);
        ctx
    }

    /// Attach session scope to context.
    pub fn with_session(&self, session_id: SessionId) -> Self {
        let mut ctx = self.clone();
        ctx.session_id = Some(session_id);
        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_and_child_span_correlation() {
        let mission_id = MissionId::new();
        let root = CorrelationContext::new_root(mission_id);

        assert_eq!(root.trace_id.len(), 32);
        assert_eq!(root.span_id.len(), 16);
        assert!(root.parent_span_id.is_none());
        assert_eq!(root.mission_id, Some(mission_id));

        let child = root.child_span();
        assert_eq!(child.trace_id, root.trace_id);
        assert_eq!(child.span_id.len(), 16);
        assert_ne!(child.span_id, root.span_id);
        assert_eq!(child.parent_span_id, Some(root.span_id));
        assert_eq!(child.mission_id, Some(mission_id));

        let task_id = TaskId::new();
        let task_ctx = child.with_task(task_id);
        assert_eq!(task_ctx.task_id, Some(task_id));
        assert_eq!(task_ctx.trace_id, root.trace_id);
    }
}
