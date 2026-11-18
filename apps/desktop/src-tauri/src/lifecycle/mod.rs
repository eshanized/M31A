//! Application lifecycle management and runtime initialization for M31A Desktop.

use crate::events::start_event_bridge;
use crate::state::DesktopState;
use m31a_runtime::{M31aRuntime, RuntimeBuilder, RuntimeConfig, RuntimeError};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

/// Initializes the authoritative M31aRuntime from configuration.
pub async fn init_runtime(custom_cwd: Option<PathBuf>) -> Result<M31aRuntime, RuntimeError> {
    // A packaged GUI process may not have a useful current directory. Always
    // prefer the explicit home workspace supplied by the desktop host.
    let cwd = match custom_cwd {
        Some(path) => path,
        None => std::env::current_dir().map_err(RuntimeError::Io)?,
    };

    // Configuration failures must remain visible; never discard model,
    // credentials, or administrator-enforced security settings.
    let core = m31a_core::Config::load()
        .map_err(|error| RuntimeError::Config(error.to_string()))?;
    let config = RuntimeConfig::new(core, cwd);
    config.validate()?;

    info!(
        "Building authoritative runtime for workspace '{}' with active model '{}'",
        config.cwd.display(),
        config.core.model
    );

    RuntimeBuilder::new(config).build().await
}

/// Sets up desktop managed state and connects the event bridge.
pub fn setup_desktop_application(
    app: &tauri::App,
    runtime: M31aRuntime,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = DesktopState::new(runtime.clone());
    app.manage(state);

    let handle: AppHandle = app.handle().clone();
    start_event_bridge(handle, runtime);

    info!("Desktop runtime and event bridge initialized successfully");
    Ok(())
}

/// Cleanly shuts down the runtime and flushes resources.
pub async fn teardown_desktop_application(state: &DesktopState) {
    if !state.is_shutting_down() {
        state.mark_shutting_down();
        info!("Initiating desktop runtime graceful shutdown...");
        match state.runtime().shutdown().await {
            Ok(report) => {
                info!(
                    cleanly_stopped = report.cleanly_stopped,
                    stopped_procs = report.background_processes_stopped,
                    cancelled_turns = report.active_turns_cancelled,
                    "Desktop runtime shutdown complete"
                );
            }
            Err(err) => {
                warn!("Desktop runtime shutdown reported error: {err}");
            }
        }
    }
}
