//! Verification runner adapters for executing concrete verification tiers (VER-01, VER-03).

use async_trait::async_trait;
use std::path::Path;

use crate::ids::{MissionId, TaskId};
use crate::verification::types::VerificationCheck;

pub mod compiler;
pub mod deterministic;
pub mod diff_invariants;
pub mod static_analysis;
pub mod tests;

pub use compiler::CompilerRunner;
pub use deterministic::DeterministicRunner;
pub use diff_invariants::DiffInvariantsRunner;
pub use static_analysis::StaticAnalysisRunner;
pub use tests::TestRunner;

/// Abstract verification runner trait executing an individual verification tier.
#[async_trait]
pub trait VerificationRunner: Send + Sync {
    async fn execute(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        workspace_root: &Path,
        snapshot_hash: &str,
    ) -> Result<VerificationCheck, String>;
}
