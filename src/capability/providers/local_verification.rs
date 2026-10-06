//! Local verification and QA capability provider (CTL-02, D-01, D-03).
//!
//! Verification commands execute through the same production security
//! boundaries as agent process execution — workspace containment, command
//! safety, environment scrubbing, timeout with kill, and bounded output.
//! There is no "verification is trusted" exemption: verification commands
//! operate on potentially model-modified state and must remain strictly contained.

use crate::capability::VerificationService;
use crate::capability::error::CapabilityError;
use crate::capability::traits::verification::{
    VerificationKind, VerificationReport, VerificationTarget,
};
use crate::process::hardened::HardenedSpawn;
use async_trait::async_trait;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Ceiling for a single verification run (bounded; killed on expiry).
pub const VERIFICATION_TIMEOUT: Duration =
    Duration::from_secs(crate::config::canonical::DEFAULT_LOCAL_VERIFICATION_TIMEOUT_SECS);

/// Cap on captured verification output (bounded; excess truncated honestly).
pub const VERIFICATION_MAX_OUTPUT_BYTES: usize =
    crate::config::canonical::DEFAULT_TOOL_MAX_OUTPUT_BYTES;

/// Native provider executing tests, linters, and formatters in the workspace.
pub struct LocalVerificationProvider {
    workspace_root: PathBuf,
}

impl LocalVerificationProvider {
    /// Create a new verification provider for the given workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

#[async_trait]
impl VerificationService for LocalVerificationProvider {
    async fn run_verification(
        &self,
        target: &VerificationTarget,
    ) -> Result<VerificationReport, CapabilityError> {
        let start = Instant::now();

        // Fixed argv per kind; caller-supplied args are validated, never trusted.
        let (program, base_args): (&str, &[&str]) = match &target.kind {
            VerificationKind::Test => ("cargo", &["test"]),
            VerificationKind::Lint => ("cargo", &["clippy"]),
            VerificationKind::Format => ("cargo", &["fmt", "--check"]),
            VerificationKind::Custom(program) => {
                // Custom programs are the untrusted edge: a path-like program
                // must resolve inside the workspace; bare names go through
                // command-safety validation below.
                if program.contains('/') {
                    let candidate = if std::path::Path::new(program).is_absolute() {
                        PathBuf::from(program)
                    } else {
                        self.workspace_root.join(program)
                    };
                    let canonical_ws = self.workspace_root.canonicalize().map_err(|e| {
                        CapabilityError::PermissionDenied(format!(
                            "workspace root not resolvable: {e}"
                        ))
                    })?;
                    let canonical_prog = candidate.canonicalize().map_err(|e| {
                        CapabilityError::PermissionDenied(format!(
                            "custom verification program not resolvable: {e}"
                        ))
                    })?;
                    if !canonical_prog.starts_with(&canonical_ws) {
                        return Err(CapabilityError::PermissionDenied(format!(
                            "custom verification program escapes workspace: '{program}'"
                        )));
                    }
                }
                (program.as_str(), &[])
            }
        };
        let mut args: Vec<String> = base_args.iter().map(|s| s.to_string()).collect();
        args.extend(target.args.iter().cloned());

        // Canonical hardened spawn: containment + safety + scrubbed env.
        // Enforce minimal resource consumption during automated verification.
        let spawn = HardenedSpawn::new(&self.workspace_root);
        let spawn = spawn
            .with_env_var("RUST_TEST_THREADS", "2")
            .and_then(|s| s.with_env_var("CARGO_BUILD_JOBS", "2"))
            .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        let mut cmd = spawn
            .build_command(program, &args, Some(&self.workspace_root))
            .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;

        // Bounded execution: timeout kills the child (kill_on_drop is set by
        // the hardened builder, so expiry cannot leave a detached process).
        let output = tokio::time::timeout(VERIFICATION_TIMEOUT, cmd.output())
            .await
            .map_err(|_| {
                CapabilityError::InfrastructureFault(format!(
                    "verification timed out after {:?}; child killed",
                    VERIFICATION_TIMEOUT
                ))
            })?
            .map_err(|e| CapabilityError::InfrastructureFault(format!("spawn failed: {e}")))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let passed = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut combined_output = format!("{stdout}\n{stderr}");
        // Bounded output: truncate honestly instead of buffering without limit.
        if combined_output.len() > VERIFICATION_MAX_OUTPUT_BYTES {
            combined_output.truncate(VERIFICATION_MAX_OUTPUT_BYTES);
            combined_output.push_str("\n[output truncated at 1MiB]");
        }

        Ok(VerificationReport {
            passed,
            exit_code,
            output: combined_output.trim().to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
