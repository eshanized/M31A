//! Strongly typed UUIDv7 domain identifiers.
//!
//! Per D-13, this module owns all domain ID types. Each ID is a newtype wrapper
//! over UUIDv7 providing compile-time type safety and dual serialization
pub mod agent;
pub mod approval;
pub mod artifact;
pub mod check;
pub mod checkpoint;
pub mod event;
pub mod execution;
pub mod handoff;
pub mod job;
pub mod mission;
pub mod policy;
pub mod session;
pub mod task;
pub mod task_graph;
pub mod tool;
pub mod workflow;

pub use agent::AgentId;
pub use approval::ApprovalRequestId;
pub use artifact::ArtifactId;
pub use check::CheckId;
pub use checkpoint::CheckpointId;
pub use event::EventId;
pub use execution::ExecutionId;
pub use handoff::HandoffId;
pub use job::JobId;
pub use mission::MissionId;
pub use policy::PolicyEvaluationId;
pub use session::SessionId;
pub use task::{RequirementId, TaskId};
pub use task_graph::TaskGraphId;
pub use tool::ToolCallId;
pub use workflow::{WorkflowRunId, WorkflowStepRunId};
