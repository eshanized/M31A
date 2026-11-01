//! `m31a-tui`: Terminal User Interface built on Ratatui and Crossterm.
//!
//! Delivers an interactive terminal experience with conversation streaming, syntax-highlighted diffs,
//! modal dialogs, mouse navigation, and fuzzy command palettes.

pub mod ansi_escape;
pub mod app;
pub mod bottom_pane;
pub mod chatwidget;
pub mod clipboard;
pub mod composer;
pub mod diff_render;
pub mod icons;
pub mod interrupts;
pub mod keybindings;
pub mod markdown_syntax;
pub mod mermaid;
pub mod modals;
pub mod model_selector;
pub mod mouse;
pub mod panel;
pub mod renderer;
pub mod resume_picker;
pub mod screens;
pub mod scrollable_diff;
pub mod stash;
pub mod subcell;
pub mod terminal;
pub mod terminal_detection;
pub mod theme;
pub mod transcript;
pub mod transcript_view;
pub mod turn;

pub use ansi_escape::{expand_tabs, strip_ansi, visual_len};
pub use app::{App, FocusState};
pub use bottom_pane::{
    BottomPaneMode, BottomPaneState, BottomPaneSubmit, GitStatusBadge, MentionCandidate,
    MentionKind, ModelChoice, SlashCommandCandidate, SubagentStatus, SubagentTab,
    TokenContextMeter,
};
pub use chatwidget::{
    AutoScrollController, ChatWidgetState, StreamMetricsSnapshot, StreamingTokenBuffer,
    TurnExecutionState,
};
pub use clipboard::{ClipboardResult, copy_to_clipboard, copy_to_clipboard_bool};
pub use diff_render::DiffRenderer;
pub use icons::{Icon, IconMode};
pub use interrupts::{DeferredInterrupt, DeferredInterruptQueue, ToolActivityStatus};
pub use keybindings::{CANONICAL_KEYBINDINGS, KeyAction, Keybinding, canonical_keybindings};
pub use mermaid::{MermaidError, render as render_mermaid};
pub use modals::{
    CommandPaletteGeometry, CommandPaletteItem, ConnectivityDetails, ConnectivityStatus,
    ModalStack, ModalState, PendingApproval, ReasoningEffortOption, ReasoningEffortState,
    ScrubTurnItem, SelectPickerItem, SelectPickerState, SetupConfiguration, SetupStep,
    SetupWizardState,
};
pub use model_selector::{
    ModelHealthState, ModelListState, ModelSelectorGeometry, render_model_details,
    render_model_details_with_health, render_model_list,
};
pub use mouse::{MouseSelection, handle_mouse_event};
pub use panel::PanelTab;
pub use renderer::{centered_rect, model_picker_popup_rect};
pub use resume_picker::ResumePickerState;
pub use screens::Screen;
pub use scrollable_diff::{ScrollViewState, ScrollableDiff};
pub use stash::{PromptStash, PromptStashEntry};
pub use subcell::{SubcellCanvas, octant_glyph, smootherstep, tab_pulse_factor};
pub use terminal::{
    execute_palette_action, filter_palette_items, handle_key_event, handle_screen_key_event,
    run_tui_app,
};
pub use terminal_detection::{MultiplexerKind, TerminalCapabilities, TerminalInfo, TerminalKind};
pub use theme::{ColorRole, Theme, ThemeColors, ThemeVariant, init_theme, theme};
pub use transcript_view::{ExtractedCodeBlock, TranscriptCell, TranscriptVirtualizer};
pub use turn::{EventCorrelation, TurnState, TurnStateMachine};
