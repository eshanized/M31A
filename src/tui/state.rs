//! TUI state responsibilities (Principle 5) and typed event flow (Principle 4).
//!
//! The canonical store remains [`crate::tui::model::TuiViewModel`] (a pure
//! in-memory projection; zero render-time I/O). This module does NOT duplicate
//! domain truth. It provides:
//!
//! - [`TuiEvent`]: the single typed bridge from runtime outcomes to UI state.
//! - Responsibility views (`SessionState`, `ConversationState`, …): thin
//!   read-only projections over `TuiViewModel` so renderers and tests reason
//!   about one concern at a time without copying authoritative objects.
//! - [`apply_tui_event`]: incremental reducer — each event touches only its
//!   relevant section (no full reloads, no conversation rebuilds per delta).

use crate::interaction::events::InteractionEvent;
use crate::tui::conversation::TuiConversationItem;
use crate::tui::model::TuiViewModel;

/// Typed TUI event: the only way runtime outcomes enter presentation state.
///
/// `InteractionEvent` is the wire format from `TuiRuntimeBridge`.
/// `TuiEvent` is the UI-local classification used by the reducer below.
/// Rendering never sees the wire directly; it sees reduced state.
#[derive(Debug, Clone)]
pub enum TuiEvent {
    /// An interaction event arrived from the canonical bridge.
    Interaction(InteractionEvent),
    /// Streaming text delta for the in-flight assistant message.
    StreamDelta { message_id: String, delta: String },
    /// The in-flight assistant message finished.
    StreamFinished { message_id: String },
    /// Startup phase advanced (Boot → Loading → Hydrating* → Ready).
    StartupAdvanced {
        phase: crate::tui::model::RuntimeStartupState,
    },
    /// A visible failure occurred (runtime / bridge / hydration / model).
    StartupFailed { error: crate::tui::errors::TuiError },
}

impl From<InteractionEvent> for TuiEvent {
    fn from(ev: InteractionEvent) -> Self {
        Self::Interaction(ev)
    }
}

// ── Responsibility views (read-only, borrow from TuiViewModel) ─────────────

/// Presentation slice: session identity and mode.
#[derive(Debug, Clone, Copy)]
pub struct SessionState<'a> {
    pub session_id: Option<&'a str>,
    pub session_status: &'a str,
    pub view_mode: crate::tui::model::SessionViewMode,
    pub workspace_path: &'a str,
}

/// Presentation slice: conversation timeline.
#[derive(Debug, Clone, Copy)]
pub struct ConversationState<'a> {
    pub items: &'a [TuiConversationItem],
    pub follow: bool,
    pub unseen_count: usize,
}

/// Presentation slice: mission projection.
#[derive(Debug, Clone, Copy)]
pub struct MissionState<'a> {
    pub mission_id: Option<&'a str>,
    pub mission_name: &'a str,
    pub mission_status: &'a str,
    pub objective: &'a str,
}

/// Presentation slice: execution / activity.
#[derive(Debug, Clone)]
pub struct ExecutionState<'a> {
    pub operation: crate::tui::model::UiOperationState,
    pub live_activity: Option<&'a str>,
    pub active_stream_message_id: Option<&'a str>,
}

/// Presentation slice: approvals awaiting operator decision.
#[derive(Debug, Clone, Copy)]
pub struct ApprovalState<'a> {
    pub pending: &'a [crate::tui::model::TuiApprovalRequest],
}

/// Presentation slice: git truth.
#[derive(Debug, Clone, Copy)]
pub struct GitState<'a> {
    pub branch: &'a str,
    pub execution_branch: Option<&'a str>,
}

/// Presentation slice: model authority projection.
#[derive(Debug, Clone, Copy)]
pub struct ModelState<'a> {
    pub active_model: &'a str,
    pub active_provider: &'a str,
    pub active_profile: &'a str,
}

/// Facade: one borrower-friendly entry point over the canonical store.
pub struct TuiState<'a> {
    inner: &'a TuiViewModel,
}

impl<'a> TuiState<'a> {
    pub fn new(inner: &'a TuiViewModel) -> Self {
        Self { inner }
    }

    pub fn session(&self) -> SessionState<'_> {
        SessionState {
            session_id: self.inner.session_id.as_deref(),
            session_status: &self.inner.session_status,
            view_mode: self.inner.session_view_mode,
            workspace_path: &self.inner.workspace_path,
        }
    }

    pub fn conversation(&self) -> ConversationState<'_> {
        ConversationState {
            items: &self.inner.conversation,
            follow: self.inner.follow,
            unseen_count: self.inner.unseen_count,
        }
    }

    pub fn mission(&self) -> MissionState<'_> {
        MissionState {
            mission_id: self.inner.mission_id.as_deref(),
            mission_name: &self.inner.mission_name,
            mission_status: &self.inner.mission_status,
            objective: &self.inner.objective,
        }
    }

    pub fn execution(&self) -> ExecutionState<'_> {
        ExecutionState {
            operation: self.inner.operation_state(),
            live_activity: self.inner.live_activity.as_deref(),
            active_stream_message_id: self.inner.active_stream_message_id.as_deref(),
        }
    }

    pub fn approvals(&self) -> ApprovalState<'_> {
        ApprovalState {
            pending: &self.inner.approvals,
        }
    }

    pub fn git(&self) -> GitState<'_> {
        GitState {
            branch: &self.inner.git_branch,
            execution_branch: self.inner.execution_worktree_branch.as_deref(),
        }
    }

    pub fn model_authority(&self) -> ModelState<'_> {
        ModelState {
            active_model: &self.inner.active_model,
            active_provider: &self.inner.active_provider,
            active_profile: &self.inner.active_profile,
        }
    }

    /// Direct access to task / tool / artifact / verification / job
    /// projections (snapshots owned by the runtime, projected here).
    pub fn tasks(&self) -> &[crate::tui::model::TuiTaskSnapshot] {
        &self.inner.tasks
    }

    pub fn tools(&self) -> &[crate::tui::model::TuiToolSnapshot] {
        &self.inner.tools
    }

    pub fn artifacts(&self) -> &[crate::tui::model::TuiArtifactSnapshot] {
        &self.inner.artifacts
    }

    pub fn verification_checks(&self) -> &[crate::tui::model::TuiVerificationCheck] {
        &self.inner.verification_checks
    }

    pub fn jobs(&self) -> &[crate::tui::model::TuiJobSnapshot] {
        &self.inner.jobs
    }
}

// ── Incremental reducer (Principle 6) ───────────────────────────────────────

/// Apply one typed event to the projection, touching only its section.
///
/// - `StreamDelta` appends to the current assistant message (no reload).
/// - `StreamFinished` finalizes it.
/// - `Interaction` delegates to the existing interaction projector.
/// - Startup events only flip startup state (never domain truth).
pub fn apply_tui_event(model: &mut TuiViewModel, event: &TuiEvent) -> usize {
    match event {
        TuiEvent::Interaction(ie) => {
            model.apply_interaction_event(ie);
            1
        }
        TuiEvent::StreamDelta { message_id, delta } => {
            // Incremental append: find the streaming assistant message.
            let mut touched = 0;
            if let Some(item) = model.conversation.iter_mut().rev().find(|i| {
                matches!(
                    i,
                    TuiConversationItem::Assistant { id, streaming, .. }
                        if id == message_id && *streaming
                )
            }) {
                if let TuiConversationItem::Assistant { text, .. } = item {
                    text.push_str(delta);
                    touched = 1;
                }
            } else {
                model.active_stream_message_id = Some(message_id.clone());
                model.add_conversation_item(TuiConversationItem::Assistant {
                    id: message_id.clone(),
                    sequence: model.conversation.len() as u64 + 1,
                    text: delta.clone(),
                    streaming: true,
                    timestamp: chrono::Utc::now(),
                });
                touched = 1;
            }
            model.mark_dirty();
            touched
        }
        TuiEvent::StreamFinished { message_id } => {
            model.last_settled_request_id = model.active_request_id.take();
            model.active_stream_message_id = None;
            if let Some(TuiConversationItem::Assistant { streaming, .. }) =
                model.conversation.iter_mut().rev().find(
                    |i| matches!(i, TuiConversationItem::Assistant { id, .. } if id == message_id),
                )
            {
                *streaming = false;
            }
            model.mark_dirty();
            1
        }
        TuiEvent::StartupAdvanced { phase } => {
            model.runtime_status = phase.clone();
            model.mark_dirty();
            1
        }
        TuiEvent::StartupFailed { error } => {
            model.set_runtime_failed_with_kind(error.kind, error.message.clone());
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_delta_appends_incrementally_without_rebuild() {
        let mut model = TuiViewModel::new();
        let id = "m1".to_string();
        apply_tui_event(
            &mut model,
            &TuiEvent::StreamDelta {
                message_id: id.clone(),
                delta: "hello".to_string(),
            },
        );
        apply_tui_event(
            &mut model,
            &TuiEvent::StreamDelta {
                message_id: id.clone(),
                delta: " world".to_string(),
            },
        );
        assert_eq!(model.conversation.len(), 1);
        match &model.conversation[0] {
            TuiConversationItem::Assistant { text, .. } => assert_eq!(text, "hello world"),
            other => panic!("unexpected {other:?}"),
        }
        apply_tui_event(&mut model, &TuiEvent::StreamFinished { message_id: id });
        match &model.conversation[0] {
            TuiConversationItem::Assistant { streaming, .. } => assert!(!streaming),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn responsibility_views_borrow_without_copying_truth() {
        let model = TuiViewModel::new();
        let state = TuiState::new(&model);
        assert_eq!(state.session().workspace_path, ".");
        assert!(state.conversation().items.is_empty());
        assert_eq!(state.mission().mission_status, "idle");
        assert!(state.approvals().pending.is_empty());
    }
}
