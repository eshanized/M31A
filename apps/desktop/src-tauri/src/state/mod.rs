//! Desktop application state container.
//!
//! Owns the authoritative `M31aRuntime` instance and synchronizes
//! desktop-specific presentation state (active session and active workspace)
//! safely across Tauri IPC command threads.

use m31a_runtime::M31aRuntime;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct DesktopState {
    runtime: M31aRuntime,
    active_session_id: Arc<RwLock<Option<String>>>,
    active_workspace: Arc<RwLock<PathBuf>>,
    is_shutting_down: Arc<AtomicBool>,
}

impl DesktopState {
    /// Construct a new desktop state wrapping an authoritative runtime instance.
    pub fn new(runtime: M31aRuntime) -> Self {
        let initial_workspace = runtime.config().cwd.clone();
        Self {
            runtime,
            active_session_id: Arc::new(RwLock::new(None)),
            active_workspace: Arc::new(RwLock::new(initial_workspace)),
            is_shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Access the underlying authoritative runtime.
    pub fn runtime(&self) -> &M31aRuntime {
        &self.runtime
    }

    /// Retrieve the currently selected session/thread ID.
    pub fn active_session_id(&self) -> Option<String> {
        self.active_session_id.read().unwrap().clone()
    }

    /// Set the currently active session/thread ID.
    pub fn set_active_session_id(&self, session_id: Option<String>) {
        *self.active_session_id.write().unwrap() = session_id;
    }

    /// Retrieve the currently active workspace directory.
    pub fn active_workspace(&self) -> PathBuf {
        self.active_workspace.read().unwrap().clone()
    }

    /// Set the active workspace directory.
    pub fn set_active_workspace(&self, workspace: PathBuf) {
        *self.active_workspace.write().unwrap() = workspace;
    }

    /// Returns true if the desktop host is currently initiating shutdown.
    pub fn is_shutting_down(&self) -> bool {
        self.is_shutting_down.load(Ordering::SeqCst)
    }

    /// Mark shutdown initiated.
    pub fn mark_shutting_down(&self) {
        self.is_shutting_down.store(true, Ordering::SeqCst);
    }
}
