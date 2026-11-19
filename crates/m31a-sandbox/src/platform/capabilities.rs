//! Platform Capability Discovery & Diagnostics.
//!
//! Evaluates host operating system features and determines available sandbox isolation mechanisms.

use serde::{Deserialize, Serialize};

/// The primary sandbox isolation mechanism available on a platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxBackendKind {
    /// Linux Bubblewrap user namespaces with Landlock and Seccomp filters
    Bubblewrap,
    /// macOS Seatbelt SBPL sandbox via /usr/bin/sandbox-exec
    Seatbelt,
    /// Windows Win32 AppContainer process isolation
    AppContainer,
    /// Host execution without kernel-level isolation boundary (only allowed under DangerFullAccess)
    Unsandboxed,
}

/// Comprehensive platform capabilities and sandbox readiness report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilityReport {
    pub os: &'static str,
    pub primary_backend: SandboxBackendKind,
    pub bwrap_available: bool,
    pub seatbelt_available: bool,
    pub appcontainer_available: bool,
    pub landlock_available: bool,
    pub seccomp_available: bool,
    pub fail_closed_enforced: bool,
    pub diagnostics: Vec<String>,
}

impl PlatformCapabilityReport {
    /// Returns true if a genuine kernel sandbox boundary is functional on this host.
    pub fn has_kernel_sandbox(&self) -> bool {
        self.primary_backend != SandboxBackendKind::Unsandboxed
    }
}
