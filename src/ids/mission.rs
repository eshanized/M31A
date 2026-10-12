//! MissionId - legacy identifier type aliased to ExecutionRunId.
//!
//! Canonical execution identity is [`ExecutionRunId`]. This alias is preserved
//! for backwards compatibility during migration.

pub use crate::ids::execution_run::ExecutionRunId as MissionId;
