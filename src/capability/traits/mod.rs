//! The 15 core capability service traits (CTL-01, CTL-02).

pub mod artifacts;
pub mod fs;
pub mod git;
pub mod jobs;
pub mod memory;
pub mod model;
pub mod network;
pub mod process;
pub mod repo;
pub mod sandbox;
pub mod shell;
pub mod telemetry;
pub mod terminal;
pub mod verification;
pub mod web;

pub use artifacts::ArtifactStoreService;
pub use fs::{FileMetadata, FileSystemService};
pub use git::{GitBranchInfo, GitCommitInfo, GitService, GitStatusResult};
pub use jobs::{JobDescriptor, JobOutputChunk, JobService, JobStatusInfo};
pub use memory::MemoryService;
pub use model::{ModelInvocationRequest, ModelInvocationResponse, ModelService};
pub use network::NetworkService;
pub use process::{ProcessOutput, ProcessService};
pub use repo::{CodeSearchResult, RepositoryService};
pub use sandbox::{SandboxConfig, SandboxHandle, SandboxService};
pub use shell::{ShellOutput, ShellService};
pub use telemetry::TelemetryService;
pub use terminal::TerminalService;
pub use verification::{
    VerificationKind, VerificationReport, VerificationService, VerificationTarget,
};
pub use web::{WebFetchResult, WebSearchResult, WebService};
