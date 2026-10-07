//! TUI typed event flow: runtime outcomes enter presentation state through
//! exactly one reducer.
//!
//! The canonical store remains [`crate::tui::model::TuiViewModel`] (a pure
//! in-memory projection; zero render-time I/O). This module provides:
//!
//! - [`TuiEvent`]: the single typed bridge from runtime outcomes to UI state.
//! - [`apply_tui_event`]: incremental reducer — each event touches only its
//!   relevant section (no full reloads, no conversation rebuilds per delta).
//!
//! Production ingress is [`crate::tui::app::TuiApp::poll_updates`], which wraps
//! every drained bridge event as `TuiEvent::Interaction` and reduces it here.
//! There is no second event path.

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

// ── Incremental reducer ───────────────────────────────────────────────────

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
    fn startup_events_flip_only_startup_state() {
        let mut model = TuiViewModel::new();
        apply_tui_event(
            &mut model,
            &TuiEvent::StartupAdvanced {
                phase: crate::tui::model::RuntimeStartupState::HydratingSession,
            },
        );
        assert!(model.runtime_status.is_startup());
    }
}
