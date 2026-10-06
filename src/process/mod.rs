//! Shared supervised process and background job execution subsystem (CTL-01, TL-02, per D-13, D-14, D-15, D-16).
//!
//! Submodules:
//! - `tree`: Platform-neutral process-group isolation and two-phase cancellation (`ProcessTreeController`).
//! - `supervisor`: Bounded foreground command runner with timeouts and cooperative cancellation (`ProcessSupervisor`).
//! - `job`: Asynchronous background job coordinator with orphan reaping (`JobSupervisor`).
//! - `spool`: Dual-buffer output stream combining live ring buffer with durable disk spool (`DualBufferOutput`).
//! - `env`: Deny-by-default process environment builder and credential isolation (`EnvironmentBuilder`).

pub mod admission;
pub mod confinement;
pub mod env;
pub mod hardened;
pub mod identity;
pub mod job;
pub mod spool;
pub mod supervisor;
pub mod tree;
pub mod types;

pub use hardened::{HardenedSpawn, HardenedSpawnError};
pub use identity::ProcessIdentity;

pub use types::{JobDescriptor, JobOutputChunk, JobStatusInfo, ProcessOutput};

pub use admission::{JobAdmissionController, JobPermit};
pub use confinement::{ConfinementHandle, ConfinementManager, ConfinementTier};
pub use env::{
    EnvironmentBuilder, FORBIDDEN_GIT_REDIRECT_VARS, ProcessSecurityViolation,
    check_command_safety, validate_working_directory,
};
pub use job::{
    BackgroundJobRecord, JobError, JobManager, JobState, JobSupervisor, SubmitJobRequest,
    read_linux_process_starttime, reconcile_jobs_on_startup,
};
pub use spool::{
    DualBufferOutput, SpoolLifecycleState, SpoolPromotionResult, StreamType, strip_ansi,
};
pub use supervisor::{
    ProcessError, ProcessSupervisor, execute_direct_argv, execute_shell_string,
    try_execute_shell_string,
};
pub use tree::ProcessTreeController;
