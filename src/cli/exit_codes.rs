//! Stable Documented Exit Code Taxonomy (CLI-03, D-18).

use serde::{Deserialize, Serialize};
use std::process::ExitCode;

/// Stable numeric process exit codes for M31A CLI (CLI-03).
///
/// Guaranteed semantic mappings for CI/CD automation:
/// - 0: Success
/// - 1: General execution or runtime error
/// - 2: Configuration schema or loading error
/// - 3: Verification failure (Nyquist gate / completion evidence rejected)
/// - 4: Security policy violation or unattended approval required
/// - 5: Execution interrupted (SIGINT, SIGTERM, cancel)
/// - 6: Resource budget exhausted (tokens, steps, wall-clock, cost)
/// - 7: Unhandled crash recovered via checkpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum M31aExitCode {
    Success = 0,
    GeneralError = 1,
    ConfigError = 2,
    VerificationFailed = 3,
    PolicyViolation = 4,
    Interrupted = 5,
    ResourceExhausted = 6,
    CrashRecovered = 7,
}

impl M31aExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Success => "Success (0)",
            Self::GeneralError => "General Error (1)",
            Self::ConfigError => "Configuration Error (2)",
            Self::VerificationFailed => "Verification Failed (3)",
            Self::PolicyViolation => "Policy Violation / Unattended Approval (4)",
            Self::Interrupted => "Interrupted (5)",
            Self::ResourceExhausted => "Resource Exhausted (6)",
            Self::CrashRecovered => "Crash Recovered from Checkpoint (7)",
        }
    }
}

impl From<M31aExitCode> for ExitCode {
    fn from(code: M31aExitCode) -> Self {
        ExitCode::from(code.as_i32() as u8)
    }
}

impl From<i32> for M31aExitCode {
    fn from(val: i32) -> Self {
        match val {
            0 => Self::Success,
            2 => Self::ConfigError,
            3 => Self::VerificationFailed,
            4 => Self::PolicyViolation,
            5 => Self::Interrupted,
            6 => Self::ResourceExhausted,
            7 => Self::CrashRecovered,
            _ => Self::GeneralError,
        }
    }
}
