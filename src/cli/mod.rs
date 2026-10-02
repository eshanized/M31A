//! CLI Command Suite & Application Runtime Interface (CLI-01–CLI-04).

pub mod args;
pub mod dispatch;
pub mod doctor;
pub mod exit_codes;
pub mod output;

pub use args::{Cli, Commands, OutputFormat};
pub use dispatch::{CliDispatcher, CliError, CliOutput, RuntimeCommand};
pub use doctor::{
    DeploymentProbe, DoctorProbe, DoctorReport, DoctorRunner, EnvironmentProbe, GitProbe,
    ModelsProbe, NetworkMcpProbe, ProbeCategory, ProbeResult, ProbeStatus, SandboxProbe,
    StorageProbe,
};
pub use exit_codes::M31aExitCode;
pub use output::{CliStreamMessage, NdjsonStreamWriter, TerminalFrame};
