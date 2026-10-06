use crate::capability::error::CapabilityError;
use crate::capability::traits::sandbox::{SandboxConfig, SandboxHandle, SandboxService};
use crate::sandbox::capabilities::SandboxCapabilities;
use crate::sandbox::probe::PlatformProbe;
use crate::sandbox::provider::SandboxProvider;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use crate::sandbox::providers::BubblewrapSandboxProvider;
use crate::sandbox::providers::ProcessIsolationProvider;
#[cfg(target_os = "macos")]
use crate::sandbox::providers::SeatbeltSandboxProvider;
#[cfg(target_os = "windows")]
use crate::sandbox::providers::WindowsSandboxProvider;
use async_trait::async_trait;
use std::sync::Arc;

/// Native local sandbox provider enforcing workspace and OS isolation via probed sandbox provider.
pub struct LocalSandboxProvider {
    provider: Arc<dyn SandboxProvider>,
}

impl Default for LocalSandboxProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalSandboxProvider {
    /// Instantiate provider automatically probing host capabilities across platforms.
    /// - Windows: Windows Sandbox Provider (Job Objects + Path containment)
    /// - macOS: Seatbelt Sandbox Provider (sandbox-exec) if present, else ProcessIsolationProvider
    /// - Linux: Bubblewrap Sandbox Provider (bwrap + user namespaces) if present, else ProcessIsolationProvider
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        {
            let provider: Arc<dyn SandboxProvider> =
                if crate::platform::windows::job_objects_available() {
                    Arc::new(WindowsSandboxProvider::new())
                } else {
                    Arc::new(ProcessIsolationProvider::new())
                };
            return Self { provider };
        }

        #[cfg(target_os = "macos")]
        {
            let provider: Arc<dyn SandboxProvider> =
                if crate::platform::macos::find_sandbox_exec().is_some() {
                    Arc::new(SeatbeltSandboxProvider::new())
                } else {
                    Arc::new(ProcessIsolationProvider::new())
                };
            return Self { provider };
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let bwrap_opt = PlatformProbe::detect_bwrap_path();
            let userns = PlatformProbe::probe_user_namespaces();

            let provider: Arc<dyn SandboxProvider> = if let Some(bwrap) = bwrap_opt {
                if userns {
                    Arc::new(BubblewrapSandboxProvider::new(bwrap))
                } else {
                    Arc::new(ProcessIsolationProvider::new())
                }
            } else {
                Arc::new(ProcessIsolationProvider::new())
            };

            Self { provider }
        }
    }

    /// Instantiate provider with custom underlying sandbox provider.
    pub fn with_provider(provider: Arc<dyn SandboxProvider>) -> Self {
        Self { provider }
    }

    /// Access the underlying pluggable sandbox provider.
    pub fn provider(&self) -> Arc<dyn SandboxProvider> {
        Arc::clone(&self.provider)
    }
}

#[async_trait]
impl SandboxService for LocalSandboxProvider {
    fn capabilities(&self) -> Option<SandboxCapabilities> {
        Some(self.provider.capabilities())
    }

    async fn prepare_environment(
        &self,
        config: &SandboxConfig,
    ) -> Result<SandboxHandle, CapabilityError> {
        let plan = config.to_sandbox_plan();
        let handle =
            self.provider
                .prepare(&plan)
                .await
                .map_err(|e| CapabilityError::ExecutionFailed {
                    exit_code: None,
                    message: e.to_string(),
                })?;

        Ok(SandboxHandle {
            id: handle.id,
            workspace_root: handle.workspace_root,
        })
    }

    async fn cleanup_sandbox(&self, handle: &SandboxHandle) -> Result<(), CapabilityError> {
        let exec_handle = crate::sandbox::provider::SandboxExecutionHandle::new(
            handle.id.clone(),
            handle.workspace_root.clone(),
            None,
        );
        self.provider
            .cleanup(&exec_handle)
            .await
            .map_err(|e| CapabilityError::ExecutionFailed {
                exit_code: None,
                message: e.to_string(),
            })
    }
}
