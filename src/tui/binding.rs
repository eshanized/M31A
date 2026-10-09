//! One TUI composition boundary (Principle 2): `TuiRuntimeBinding`.
//!
//! ```text
//! AppRuntime → TuiRuntimeBinding → TuiApplication → state/routing/input/render
//! ```
//!
//! The binding consumes the canonical `AppRuntime` (built once by the real
//! composition root). It never constructs a production runtime or any
//! production authority (capability/tool registries, pipeline, policy,
//! budget, approvals, artifacts, model caller, context compiler, auth, git,
//! jobs). It owns only channels and task handles: the bridge sender, the
//! interaction receiver, and the bridge supervisor handle.

use std::sync::Arc;

use crate::ids::SessionId;
use crate::runtime::AppRuntime;
use crate::tui::channel::{TuiActionSender, TuiInteractionReceiver};
use crate::tui::errors::{TuiError, TuiErrorKind};

/// Outcome of the asynchronous canonical runtime assembly.
pub enum RuntimeAssemblyOutcome {
    Ready(Arc<AppRuntime>),
    Failed(TuiError),
}

/// Binding between the canonical runtime and the TUI projection.
///
/// Created empty (no runtime). `attach_runtime` binds exactly one canonical
/// runtime and spawns exactly one `TuiRuntimeBridge`. No second runtime is
/// ever created here.
pub struct TuiRuntimeBinding {
    runtime: Option<Arc<AppRuntime>>,
    bridge_tx: Option<TuiActionSender>,
    interaction_rx: Option<TuiInteractionReceiver>,
    bridge_handle: Option<tokio::task::JoinHandle<()>>,
    session_id: Option<SessionId>,
    last_error: Option<TuiError>,
}

impl Default for TuiRuntimeBinding {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiRuntimeBinding {
    pub fn new() -> Self {
        Self {
            runtime: None,
            bridge_tx: None,
            interaction_rx: None,
            bridge_handle: None,
            session_id: None,
            last_error: None,
        }
    }

    /// True once a canonical runtime has been attached.
    pub fn has_runtime(&self) -> bool {
        self.runtime.is_some()
    }

    /// True once bridge channels exist.
    pub fn has_bridge(&self) -> bool {
        self.bridge_tx.is_some()
    }

    pub fn runtime(&self) -> Option<Arc<AppRuntime>> {
        self.runtime.clone()
    }

    pub fn last_error(&self) -> Option<&TuiError> {
        self.last_error.as_ref()
    }

    pub fn bridge_sender(&self) -> Option<TuiActionSender> {
        self.bridge_tx.clone()
    }

    pub fn take_interaction_receiver(&mut self) -> Option<TuiInteractionReceiver> {
        self.interaction_rx.take()
    }

    /// Attach the ONE canonical runtime and spawn its bridge.
    ///
    /// Pure consumer: `runtime` comes from the caller (the real composition
    /// root in `main.rs`). Hydration of durable state is performed by the
    /// caller via `TuiApplication::hydrate_from_runtime` (async, outside render);
    /// bridge startup here only wires channels and emits initial events.
    pub async fn attach_runtime(
        &mut self,
        runtime: Arc<AppRuntime>,
        resume_session_id: Option<SessionId>,
    ) -> Result<(), TuiError> {
        let (mut bridge, handle) =
            crate::tui::runtime_bridge::TuiRuntimeBridge::spawn(runtime.clone(), resume_session_id)
                .await
                .map_err(|e| {
                    TuiError::new(TuiErrorKind::Bridge, format!("bridge startup failed: {e}"))
                })?;
        self.session_id = bridge.session_id;
        let tx = bridge.sender();
        self.interaction_rx = bridge.take_event_receiver();
        self.bridge_tx = Some(tx);
        self.bridge_handle = Some(handle);
        self.runtime = Some(runtime);
        self.last_error = None;
        Ok(())
    }

    /// Record a terminal assembly failure as visible state.
    pub fn record_failure(&mut self, error: TuiError) {
        self.last_error = Some(error);
    }

    /// Non-blocking supervisor: `Some(error)` when the bridge task finished
    /// unexpectedly and the UI must surface it instead of going stale/blank.
    pub fn poll_bridge_supervision(&mut self) -> Option<TuiError> {
        if let Some(ref handle) = self.bridge_handle {
            if handle.is_finished() {
                self.bridge_handle = None;
                let err = TuiError::bridge("cockpit bridge terminated unexpectedly");
                self.last_error = Some(err.clone());
                return Some(err);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_starts_empty_and_records_failures_visibly() {
        let mut b = TuiRuntimeBinding::new();
        assert!(!b.has_runtime());
        assert!(!b.has_bridge());
        b.record_failure(TuiError::runtime("boom"));
        assert_eq!(b.last_error().unwrap().message, "boom");
    }
}
