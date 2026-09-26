//! State machine module - lifecycle enums and validated transitions (KRN-02)
//!
//! Per D-15, state_machine/ owns lifecycle enums and validated transitions.
//! Per D-08, enum-based state with transition functions returning Result<NewState, TransitionError>.
//! Per D-10, invalid transitions return Result::Err(TransitionError) — caller handles.

pub mod agent;
pub mod autonomy;
pub mod change_set;
pub mod error;
pub mod lifecycle;
pub mod mission;
pub mod task;

pub use agent::{AgentEvent, AgentRole, AgentState, transition_agent};
pub use autonomy::AutonomyMode;
pub use change_set::{ChangeSetEvent, ChangeSetState, transition_change_set};
pub use error::TransitionError;
pub use lifecycle::{LifecycleEvent, LifecycleStage, LifecycleState, transition_lifecycle};
pub use mission::{MissionEvent, MissionState, transition_mission};
pub use task::{TaskEvent, TaskState, transition_task};
