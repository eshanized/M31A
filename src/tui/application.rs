//! One TUI composition root.
//!
//! ```text
//! AppRuntime → TuiRuntimeBinding → TuiApplication (owns TuiApp engine)
//!     → keymap classification → engine handlers → state → render
//! ```
//!
//! Guarantees:
//! - Renderer never waits for runtime readiness: construction is synchronous,
//!   the first frame draws before any runtime await.
//! - Exactly one canonical `AppRuntime` is consumed (never constructed here).
//! - Events flow `runtime → bridge → TuiEvent → apply_tui_event → render`;
//!   `render` performs zero I/O and never touches the runtime.
//! - Hydration is phased (`Boot → LoadingRuntime → HydratingSession →
//!   HydratingWorkspace → HydratingExecution → Ready`), every phase renders.
//! - Failures are explicit visible state, never blank frames.
//!
//! `TuiApp` is the engine owned by this root (handlers, projection, render).
//! `TuiApplication` is the single production entry point: it owns the engine
//! plus the runtime binding, async assembly, and input classification.

use std::path::PathBuf;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::Backend;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::config::ResolvedConfiguration;
use crate::events::bus::BroadcastEventBus;
use crate::runtime::AppRuntime;
use crate::tui::TuiApp;
use crate::tui::binding::{RuntimeAssemblyOutcome, TuiRuntimeBinding};
use crate::tui::errors::{TuiError, TuiErrorKind};
use crate::tui::keymap::{KeyAction, KeyContext, resolve_key};
use crate::tui::model::RuntimeStartupState;
use crate::tui::state::{TuiEvent, apply_tui_event};

/// Owned TUI application: the single composition boundary.
///
/// Main (`run_tui_or_fallback`) constructs exactly one of these after the
/// terminal guard, draws the first frame synchronously, then drives
/// `poll_runtime()` + `tick()` + input without ever blocking render on the
/// runtime.
pub struct TuiApplication {
    app: TuiApp,
    binding: TuiRuntimeBinding,
    assembly_rx: Option<UnboundedReceiver<RuntimeAssemblyOutcome>>,
    assembly_settled: bool,
}

impl TuiApplication {
    /// Create a valid application synchronously (no runtime, no I/O).
    ///
    /// Sets `InitializingRuntime` presentation state; the caller must draw
    /// the first frame immediately (see [`TuiApplication::render_first_frame`]).
    pub fn new(workspace_root: PathBuf, config: &ResolvedConfiguration) -> Self {
        let mut app = TuiApp::new()
            .with_workspace_root(workspace_root)
            .with_config(config)
            .with_composer_focused(true);
        app.set_runtime_initializing(Some("Preparing execution authorities…".to_string()));
        Self {
            app,
            binding: TuiRuntimeBinding::new(),
            assembly_rx: None,
            assembly_settled: false,
        }
    }

    /// Begin asynchronous canonical runtime assembly (off the first-frame path).
    pub fn spawn_runtime_assembly(
        &mut self,
        pool: sqlx::SqlitePool,
        workspace_root: PathBuf,
        event_bus: Arc<BroadcastEventBus>,
        config: Arc<ResolvedConfiguration>,
    ) {
        self.assembly_rx = Some(TuiRuntimeBinding::spawn_assembly(
            pool,
            workspace_root,
            event_bus,
            config,
        ));
        self.assembly_settled = false;
    }

    /// Draw the first frame synchronously. Must precede every runtime await.
    pub fn render_first_frame<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> std::io::Result<bool> {
        self.app.render_frame(terminal)
    }

    pub fn is_running(&self) -> bool {
        self.app.is_running
    }

    pub fn stop(&mut self) {
        self.app.is_running = false;
    }

    pub fn app(&self) -> &TuiApp {
        &self.app
    }

    pub fn app_mut(&mut self) -> &mut TuiApp {
        &mut self.app
    }

    /// Classify the next key under the single input-priority authority
    /// (`keymap::resolve_key`): replay → dialog → overlay → composer/route.
    ///
    /// The classification is CONSUMED here, not discarded: replay owns its
    /// keys outright (read-only scrub blocks every mutation command), and
    /// every other class delegates to the engine, whose handler order mirrors
    /// this same priority.
    pub fn classify_key(&mut self, key: KeyEvent) -> KeyAction {
        let ctx = KeyContext::new(
            self.app.replay.is_active,
            self.app.approval_modal.is_open,
            self.app.palette.is_open
                || self.app.overlay_manager.is_help_open
                || self.app.navigation.active_overlay.is_some(),
            self.app.is_composer_focused,
        );
        resolve_key(key, ctx)
    }

    /// Poll async runtime assembly without blocking render.
    ///
    /// On `Ready`: walks the phased hydration
    /// (`HydratingSession → HydratingWorkspace → HydratingExecution → Ready`),
    /// drawing each phase before its work so partial data stays visible.
    /// On failure: records a visible typed error (never blank).
    pub async fn poll_runtime<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> bool {
        if self.assembly_settled {
            return false;
        }
        let outcome = match self.assembly_rx.as_mut() {
            Some(rx) => match rx.try_recv() {
                Ok(o) => Some(o),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    Some(RuntimeAssemblyOutcome::Failed(TuiError::runtime(
                        "runtime initialization task terminated without a result",
                    )))
                }
            },
            None => None,
        };
        let Some(outcome) = outcome else {
            return false;
        };
        match outcome {
            RuntimeAssemblyOutcome::Ready(rt) => {
                self.attach_ready_runtime(terminal, rt).await;
                true
            }
            RuntimeAssemblyOutcome::Failed(err) => {
                self.assembly_settled = true;
                self.binding.record_failure(err.clone());
                self.app
                    .set_runtime_failed_with_kind(err.kind, err.message.clone());
                let _ = self.app.render_frame(terminal);
                true
            }
        }
    }

    async fn attach_ready_runtime<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        rt: Arc<AppRuntime>,
    ) {
        self.assembly_settled = true;
        // Announce session hydration first so the operator sees progress
        // before any store await can stall the frame.
        self.app
            .set_runtime_hydrating_session(Some("Loading session state…".to_string()));
        let _ = self.app.render_frame(terminal);
        self.app.hydrate_from_runtime(&rt).await;
        // Announce workspace hydration next: the store call above already
        // populated workspace sections, and naming the step keeps the UI
        // honest about what just landed instead of jumping straight to ready.
        self.app
            .set_runtime_hydrating_workspace(Some("Loading workspace state…".to_string()));
        let _ = self.app.render_frame(terminal);
        // Announce bridge connect last: input stays gated until the single
        // event ingress is attached, so this step must be visible on its own.
        self.app
            .set_runtime_hydrating_execution(Some("Connecting cockpit bridge…".to_string()));
        let _ = self.app.render_frame(terminal);
        match self.binding.attach_runtime(rt.clone(), None).await {
            Ok(()) => {
                // Move bridge channels onto the live (already hydrated)
                // projection. `replace` moves the existing app out so only
                // channels change — model, navigation, composer, and theme
                // are preserved.
                if let Some(tx) = self.binding.bridge_sender() {
                    let live = std::mem::take(&mut self.app).with_bridge_tx(tx);
                    self.app = live;
                }
                if let Some(irx) = self.binding.take_interaction_receiver() {
                    let live = std::mem::take(&mut self.app).with_interaction_rx(irx);
                    self.app = live;
                }
                self.app.set_runtime_ready();
                let _ = self.app.render_frame(terminal);
            }
            Err(err) => {
                self.binding.record_failure(err.clone());
                self.app
                    .set_runtime_failed_with_kind(err.kind, err.message.clone());
                let _ = self.app.render_frame(terminal);
            }
        }
        // Keep the canonical runtime alive via the binding.
        let _ = self.binding.runtime();
    }

    /// Supervise the bridge task: surface termination as visible error.
    pub fn supervise_bridge(&mut self) -> bool {
        if let Some(err) = self.binding.poll_bridge_supervision() {
            if self.app.is_runtime_ready() {
                self.app
                    .set_runtime_failed_with_kind(err.kind, err.message.clone());
                return true;
            }
        }
        false
    }

    /// Apply one typed event incrementally (Principle 6).
    pub fn apply_event(&mut self, event: &TuiEvent) -> usize {
        apply_tui_event(&mut self.app.model, event)
    }

    /// Drain bridge events into the incremental reducer.
    pub fn drain_bridge_events(&mut self) -> usize {
        // Reuse the engine's poll, then count as incremental section updates.
        self.app.poll_updates()
    }

    /// One decoupled tick: bridge events → typed reduce → render.
    pub fn tick<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> std::io::Result<bool> {
        self.app.tick(terminal)
    }

    /// Handle a key through the single input-priority authority.
    ///
    /// `classify_key` decides the owner; replay short-circuits here (no
    /// mutation command may escape read-only scrub), everything else
    /// delegates to the engine handlers in the same priority order.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<crate::cli::RuntimeCommand> {
        match self.classify_key(key) {
            KeyAction::Replay => {
                if self.app.replay.handle_key(key) {
                    self.app.model = self.app.replay.reconstruct_current_view();
                    self.app.model.mark_dirty();
                }
                None
            }
            _ => self.app.handle_key(key),
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        self.app.handle_paste(text);
    }

    pub fn handle_mouse_scroll(&mut self, up: bool) {
        self.app.handle_mouse_scroll(up);
    }

    /// Exit chord handling shared by the event loop (q / Ctrl+C / Ctrl+D).
    pub fn should_exit(&self, key: KeyEvent) -> bool {
        // Mirror the historical loop semantics: q exits only from route mode
        // with no transient UI; Ctrl+C / Ctrl+D exit unless a dialog owns them.
        let no_dialog = !self.app.approval_modal.is_open && !self.app.palette.is_open;
        match key.code {
            KeyCode::Char('q') => {
                key.modifiers.is_empty() && no_dialog && !self.app.is_composer_focused
            }
            KeyCode::Char('c') | KeyCode::Char('d') => {
                key.modifiers.contains(keymap_ctrl()) && no_dialog
            }
            _ => false,
        }
    }

    /// Whether the composer currently owns cancel (Ctrl+C clears instead of exits).
    pub fn composer_owns_cancel(&self) -> bool {
        self.app.model.has_active_animation()
            || (self.app.is_composer_focused && !self.app.composer.text().is_empty())
    }

    pub fn startup_phase(&self) -> &RuntimeStartupState {
        &self.app.model.runtime_status
    }
}

fn keymap_ctrl() -> crossterm::event::KeyModifiers {
    crossterm::event::KeyModifiers::CONTROL
}

/// Classify a mouse event (scroll only; other gestures ignored, as before).
pub fn classify_mouse(kind: MouseEventKind) -> Option<bool> {
    match kind {
        MouseEventKind::ScrollUp => Some(true),
        MouseEventKind::ScrollDown => Some(false),
        _ => None,
    }
}

/// Map a supervision outcome to a typed startup failure (testable helper).
pub fn bridge_termination_error() -> TuiError {
    TuiError::new(
        TuiErrorKind::Bridge,
        "cockpit bridge terminated unexpectedly",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_constructs_synchronously_with_startup_state() {
        let config = test_config();
        let tui = TuiApplication::new(std::path::PathBuf::from("/tmp/ws"), &config);
        assert!(tui.is_running());
        assert!(!tui.app.is_runtime_ready());
        assert!(tui.app.model.runtime_status.is_startup());
    }

    #[test]
    fn first_frame_renders_without_runtime() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let config = test_config();
        let mut tui = TuiApplication::new(std::path::PathBuf::from("/tmp/ws"), &config);
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let drew = tui.render_first_frame(&mut terminal).unwrap();
        assert!(drew, "first frame must draw before runtime exists");
        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(content.contains("M31A"), "first frame must show M31A");
    }

    #[test]
    fn typed_failure_is_visible_state() {
        let config = test_config();
        let mut tui = TuiApplication::new(std::path::PathBuf::from("/tmp/ws"), &config);
        tui.app
            .set_runtime_failed_with_kind(crate::tui::errors::TuiErrorKind::Bridge, "boom");
        assert!(!tui.app.is_runtime_ready());
        assert_eq!(
            tui.app.model.runtime_error_kind,
            Some(crate::tui::errors::TuiErrorKind::Bridge)
        );
    }

    fn test_config() -> ResolvedConfiguration {
        ResolvedConfiguration::build_fallback(std::path::PathBuf::from("/tmp/ws"))
    }
}
