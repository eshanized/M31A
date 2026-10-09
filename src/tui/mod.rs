//! Terminal TUI Cockpit & Projection Subsystem.
//!
//! Delivers the operational cockpit. Enforces Law 9: "The TUI is a
//! projection, never authoritative state."
//!
//! Public surface is deliberately small: the application root, the runtime
//! binding, the event reducer, the navigation authority, and the primitives
//! the binary and integration tests drive directly. Everything else is
//! reached through its module path (`tui::model::TuiViewModel`, …).

pub mod app;
pub mod approval;
pub mod binding;
pub mod channel;
pub mod component;
pub mod composer;
pub mod conversation;
pub mod errors;
pub mod focus;
pub mod guard;
pub mod icons;
pub mod input;
pub mod keymap;
pub mod layout;
pub mod lifecycle;
pub mod model;
pub mod navigation;
pub mod overlay;
pub mod palette_v2;
pub mod registry;
pub mod replay;
pub mod routes;
pub mod runtime_bridge;
pub mod sanitizer;
pub mod screens;
pub mod shell;
pub mod state;
pub mod status;
pub mod surface;
pub mod theme;

pub use app::{TuiApplication, classify_mouse};
pub use binding::{RuntimeAssemblyOutcome, TuiRuntimeBinding};
pub use channel::{ActionSendError, TuiActionSender, TuiInteractionReceiver, TuiInteractionSender};
pub use errors::{TuiError, TuiErrorKind};
pub use guard::{TerminalGuard, install_panic_hook};
pub use keymap::{KeyAction, KeyContext, resolve_key};
pub use layout::{LayoutTier, classify_terminal_size, compute_layout};
pub use navigation::{
    NavigationAction, NavigationRouter, RouteResolution, ScreenId, canonical_screen, resolve_view,
};
pub use replay::{ReplayController, ReplaySpeed};
pub use runtime_bridge::TuiRuntimeBridge;
pub use sanitizer::{sanitize_diff, sanitize_terminal_text, sanitize_tool_spool};
pub use state::{TuiEvent, apply_tui_event};
