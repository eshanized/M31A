//! Checkpoint subsystem root module (CHK-01–CHK-05, D-13–D-16).
//!
//! Enforces:
//! - Strict two-phase checkpoint commit (staging -> sync -> validation -> atomic SQLite commit)
//! - CheckpointManifest domain model binding all 9 state components with SHA-256 hash
//! - 5-point CheckpointIntegrityValidator
//! - Staged StartupCrashRecoveryScanner with POSIX process group cleanup & spool sealing
//! - SafeResumeEngine preserving completed work while invalidating drifted tasks

pub mod crash_recovery;
pub mod integrity;
pub mod manager;
pub mod manifest;
pub mod resume;

pub use crash_recovery::{
    CrashRecoveryClassification, CrashRecoveryError, CrashRecoveryResult,
    StartupCrashRecoveryScanner,
};
pub use integrity::{CheckpointIntegrityError, CheckpointIntegrityValidator};
pub use manager::{
    CheckpointError, CheckpointManager, CheckpointRestoreResult, StagedArtifactInput,
};
pub use manifest::{CURRENT_CHECKPOINT_SCHEMA_VERSION, CheckpointManifest};
pub use resume::{SafeResumeEngine, SafeResumeError, SafeResumeReport};
