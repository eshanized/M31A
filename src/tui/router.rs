//! Legacy navigation router compatibility shim.
//!
//! The single canonical navigation authority is
//! [`crate::tui::navigation::NavigationRouter`]. This module retains the
//! historical view-first router for backward compatibility only. New code
//! must route through `navigation::{canonical_screen, resolve_view,
//! NavigationRouter::navigate_to_view}`.
//!
//! The shim delegates primary-route decisions to the canonical mapping so the
//! two routers cannot drift apart.

use crate::tui::navigation::{ScreenId, canonical_screen};
use crate::tui::registry::ViewId;

/// Navigation change event for telemetry and breadcrumbs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationEvent {
    pub from: ViewId,
    pub to: ViewId,
}

/// Legacy view-first router.
///
/// Deprecated: prefer [`crate::tui::navigation::NavigationRouter`], which owns
/// the primary route plus contextual detail/overlay state. This shim keeps the
/// previous API surface so existing callers continue to compile and behave.
#[derive(Debug, Clone)]
#[deprecated(
    since = "0.1.0",
    note = "use tui::navigation::NavigationRouter (canonical authority) instead"
)]
pub struct NavigationRouter {
    current_view: ViewId,
    active_detail: Option<ViewId>,
    active_overlay: Option<ViewId>,
    history: Vec<ViewId>,
    max_history: usize,
    last_event: Option<NavigationEvent>,
}

#[allow(deprecated)]
impl Default for NavigationRouter {
    fn default() -> Self {
        Self::new(ViewId::MissionDashboard)
    }
}

#[allow(deprecated)]
impl NavigationRouter {
    pub const DEFAULT_MAX_HISTORY: usize = 50;

    /// Create router with an initial primary view.
    pub fn new(initial: ViewId) -> Self {
        Self {
            current_view: initial,
            active_detail: None,
            active_overlay: None,
            history: Vec::new(),
            max_history: Self::DEFAULT_MAX_HISTORY,
            last_event: None,
        }
    }

    /// Return active primary view ID.
    pub fn current_view(&self) -> ViewId {
        self.current_view
    }

    /// Canonical workspace screen for the active view, via the single mapping.
    pub fn canonical_screen(&self) -> ScreenId {
        canonical_screen(self.current_view)
    }

    /// Return active detail pane if open.
    pub fn active_detail(&self) -> Option<ViewId> {
        self.active_detail
    }

    /// Return active overlay if present.
    pub fn active_overlay(&self) -> Option<ViewId> {
        self.active_overlay
    }

    /// Return history stack.
    pub fn history(&self) -> &[ViewId] {
        &self.history
    }

    /// Return last navigation event.
    pub fn last_event(&self) -> Option<&NavigationEvent> {
        self.last_event.as_ref()
    }

    /// Navigate to a new primary view, recording previous in history.
    pub fn navigate_to(&mut self, view: ViewId) {
        if self.current_view == view {
            return;
        }

        self.history.push(self.current_view);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }

        let event = NavigationEvent {
            from: self.current_view,
            to: view,
        };
        self.current_view = view;
        self.active_detail = None;
        self.active_overlay = None;
        self.last_event = Some(event);
    }

    /// Pop previous view from back-stack.
    pub fn go_back(&mut self) -> Option<ViewId> {
        if let Some(prev) = self.history.pop() {
            let event = NavigationEvent {
                from: self.current_view,
                to: prev,
            };
            self.current_view = prev;
            self.active_detail = None;
            self.active_overlay = None;
            self.last_event = Some(event);
            Some(prev)
        } else {
            None
        }
    }

    /// Open a detail drilldown pane.
    pub fn open_detail(&mut self, detail: ViewId) {
        self.active_detail = Some(detail);
    }

    /// Close the active detail pane.
    pub fn close_detail(&mut self) {
        self.active_detail = None;
    }

    /// Open a transient overlay (e.g. Command Palette).
    pub fn open_overlay(&mut self, overlay: ViewId) {
        self.active_overlay = Some(overlay);
    }

    /// Close active overlay.
    pub fn close_overlay(&mut self) {
        self.active_overlay = None;
    }

    /// Reset router to initial dashboard and clear history.
    pub fn reset(&mut self, root: ViewId) {
        self.current_view = root;
        self.active_detail = None;
        self.active_overlay = None;
        self.history.clear();
        self.last_event = None;
    }
}
