//! Terminal TUI Cockpit & Projection Subsystem (TUI-01–TUI-06, TUI 2.0).
//!
//! Delivers the full operational cockpit defined by UI_UX_SPEC_M31A.md and TUI 2.0.
//! Enforces Law 9: "The TUI is a projection, never authoritative state."

pub mod app;
pub mod approval;
pub mod channel;
pub mod component;
pub mod composer;
pub mod conversation;
pub mod dispatch_bridge;
pub mod focus;
pub mod guard;
pub mod icons;
pub mod input;
pub mod layout;
pub mod lifecycle;
pub mod model;
pub mod navigation;
pub mod overlay;
pub mod palette_v2;
pub mod registry;
pub mod replay;
pub mod router;
pub mod runtime_bridge;
pub mod sanitizer;
pub mod screens;
pub mod shell;
pub mod status;
pub mod surface;
pub mod theme;
pub mod view_tier;

pub use app::TuiApp;
pub use approval::{ApprovalDecision, ApprovalModal};
pub use channel::{TuiUpdateReceiver, TuiUpdateSender, create_tui_channel};
pub use component::{
    DiffParser, DiffStats, KeyHint, ParsedDiffLine, format_key_hints, format_progress_bar,
    render_agent_tag, render_bounded_output, render_diff_lines, render_key_chip,
    render_progress_line, render_risk_badge, render_status_badge,
};
pub use composer::{ComposerAction, TuiComposer};
pub use conversation::TuiConversationItem;
pub use dispatch_bridge::{CreateMissionRequest, DispatchBridge, DispatchError};
pub use focus::{FocusManager, FocusTarget};
pub use guard::{TerminalGuard, install_panic_hook};
pub use layout::{
    LayoutAreas, LayoutTier, ResponsiveLayout, classify_terminal_size, compute_layout,
};
pub use lifecycle::{TuiLifecycleProjection, TuiLifecycleStage};
pub use model::{
    RuntimeStartupState, SessionViewMode, TuiAgentSnapshot, TuiApprovalRequest, TuiJobSnapshot,
    TuiLogLine, TuiModelUsage, TuiSystemStats, TuiTaskSnapshot, TuiToolSnapshot, TuiViewModel,
    UiOperationState,
};
pub use navigation::{
    NavigationAction, NavigationRouter, RouteResolution, ScreenId, canonical_screen, resolve_view,
};
pub use overlay::{OverlayKind, OverlayManager, OverlayPriority, render_help_overlay};
pub use palette_v2::{PaletteActionV2, PaletteItemV2, UniversalCommandPalette};
pub use registry::{ViewId, ViewKind, ViewMetadata, ViewRegistry};
pub use replay::{ReplayController, ReplaySpeed};
#[allow(deprecated)]
pub use router::NavigationEvent;
// Legacy compatibility alias. New code must use
// `navigation::NavigationRouter`, the single canonical authority.
pub use icons::{IconKey, IconMode, IconRegistry, Spinner};
#[allow(deprecated)]
pub use router::NavigationRouter as CanonicalRouter;
pub use runtime_bridge::TuiRuntimeBridge;
pub use sanitizer::{sanitize_diff, sanitize_terminal_text, sanitize_tool_spool};
pub use shell::{render_context_rail, render_footer, render_header, render_workspace};
pub use status::{StatusKind, StatusPresentation};
pub use surface::{
    render_agents_surface, render_artifacts_surface, render_conversation_surface,
    render_doctor_surface, render_git_surface, render_jobs_surface, render_replay_surface,
    render_tasks_surface, render_telemetry_surface, render_tools_surface,
    render_verification_surface,
};
pub use theme::{ThemeMode, ThemeTokens};
pub use view_tier::ViewTier;
