//! Responsive Layout Classification & Breakdown (TUI-02).
//!
//! Classifies terminal dimensions into 4 tiers:
//! - Compact (<100w): Single-column stacked layout with tabbed pane switching; optimized for 80x24.
//! - Standard (100–159w): Dual-column layout (sidebar + main cockpit).
//! - Large (160–219w): Three-column layout (nav sidebar + cockpit + telemetry).
//! - UltraWide (>=220w): Full wide-screen layout with expanded inspector panes.

pub mod compact;
pub mod wide;

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Responsive layout tier based on terminal dimensions.
///
/// This is the ONE terminal-classification type in the TUI. Width and height
/// both participate: narrow or short viewports collapse to `Compact`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayoutTier {
    /// Compact tier: < 100 columns or < 30 rows. Single-column stacked layout.
    Compact,
    /// Standard tier: 100–159 columns. Dual-column (sidebar + cockpit).
    Standard,
    /// Large tier: 160–219 columns. Three-column cockpit.
    Large,
    /// UltraWide tier: >= 220 columns. Full wide cockpit with inspectors.
    UltraWide,
}

impl LayoutTier {
    /// Minimum supported terminal width in columns.
    pub const MIN_WIDTH: u16 = 80;
    /// Minimum supported terminal height in rows.
    pub const MIN_HEIGHT: u16 = 24;

    /// Classify terminal dimensions into a `LayoutTier`.
    pub fn from_dimensions(width: u16, height: u16) -> Self {
        if width < 100 || height < Self::MIN_HEIGHT {
            Self::Compact
        } else if width < 160 {
            Self::Standard
        } else if width < 220 {
            Self::Large
        } else {
            Self::UltraWide
        }
    }

    /// Whether the dimensions fall below the absolute 80x24 minimum.
    pub fn is_below_minimum(width: u16, height: u16) -> bool {
        width < Self::MIN_WIDTH || height < Self::MIN_HEIGHT
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Standard => "standard",
            Self::Large => "large",
            Self::UltraWide => "ultrawide",
        }
    }
}

/// Classify terminal dimensions into responsive tier.
///
/// Delegates to `LayoutTier::from_dimensions(width, height)` so width and height
/// participate consistently under the single layout classification authority.
pub fn classify_terminal_size(width: u16, height: u16) -> LayoutTier {
    LayoutTier::from_dimensions(width, height)
}

/// Rectangles allocated for the primary screen components in a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutAreas {
    pub header: Rect,
    pub main: Rect,
    pub sidebar: Option<Rect>,
    pub telemetry: Option<Rect>,
    pub footer: Rect,
}

/// Helper struct for computing responsive layouts and verifying viewport bounds.
#[derive(Debug, Clone)]
pub struct ResponsiveLayout;

impl ResponsiveLayout {
    /// Computes responsive layout areas for a given area, honoring header, body, footer,
    /// and multi-column partitioning.
    pub fn partition(area: Rect) -> (LayoutTier, LayoutAreas) {
        let tier = LayoutTier::from_dimensions(area.width, area.height);

        let areas = match tier {
            LayoutTier::Compact => compact::compute_compact_layout(area),
            LayoutTier::Standard => wide::compute_standard_layout(area),
            LayoutTier::Large => wide::compute_large_layout(area),
            LayoutTier::UltraWide => wide::compute_ultrawide_layout(area),
        };

        (tier, areas)
    }

    /// Splits an area vertically into Header (3 lines), Content (remaining), and Footer (1 line).
    pub fn split_vertical_chrome(area: Rect) -> (Rect, Rect, Rect) {
        if area.height < 5 {
            return (area, Rect::default(), Rect::default());
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        (chunks[0], chunks[1], chunks[2])
    }
}

/// Compute layout partition for the current terminal area.
pub fn compute_layout(area: Rect) -> (LayoutTier, LayoutAreas) {
    ResponsiveLayout::partition(area)
}

/// Target pane classification for mouse routing and hit testing (AUD-22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollTargetPane {
    Conversation,
    ContextRail,
    SidePanel,
    DetailInspector,
    Composer,
    Header,
    Footer,
    ModalOrOverlay,
}

/// Geometry of all rendered panes in the active workspace frame (AUD-22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkspaceLayoutGeometry {
    pub terminal_area: Rect,
    pub header_area: Rect,
    pub footer_area: Rect,
    pub workspace_area: Rect,
    pub rail_area: Option<Rect>,
    pub composer_area: Option<Rect>,
    pub detail_area: Option<Rect>,
    pub conversation_area: Option<Rect>,
    pub side_panel_area: Option<Rect>,
    pub modal_or_overlay_active: bool,
}

impl WorkspaceLayoutGeometry {
    /// Hit-test mouse coordinates against the rendered layout geometry.
    /// Evaluates precedence deterministically: modal/overlay ownership blocks workspace,
    /// followed by header/footer chrome, detail inspector, composer, context rail,
    /// side inspector panel, and main conversation area.
    pub fn target_pane_at(&self, col: u16, row: u16) -> Option<ScrollTargetPane> {
        if self.modal_or_overlay_active {
            return Some(ScrollTargetPane::ModalOrOverlay);
        }
        if !rect_contains(self.terminal_area, col, row) {
            return None;
        }
        if rect_contains(self.header_area, col, row) {
            return Some(ScrollTargetPane::Header);
        }
        if rect_contains(self.footer_area, col, row) {
            return Some(ScrollTargetPane::Footer);
        }
        if let Some(detail) = self.detail_area {
            if rect_contains(detail, col, row) {
                return Some(ScrollTargetPane::DetailInspector);
            }
        }
        if let Some(composer) = self.composer_area {
            if rect_contains(composer, col, row) {
                return Some(ScrollTargetPane::Composer);
            }
        }
        if let Some(rail) = self.rail_area {
            if rect_contains(rail, col, row) {
                return Some(ScrollTargetPane::ContextRail);
            }
        }
        if let Some(side) = self.side_panel_area {
            if rect_contains(side, col, row) {
                return Some(ScrollTargetPane::SidePanel);
            }
        }
        if let Some(convo) = self.conversation_area {
            if rect_contains(convo, col, row) {
                return Some(ScrollTargetPane::Conversation);
            }
        }
        None
    }
}

/// True if `rect` contains the point `(col, row)`.
pub fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

/// Compute canonical workspace layout geometry matching render_workspace exactly (AUD-22).
pub fn compute_workspace_layout_geometry(
    terminal: Rect,
    screen: crate::tui::navigation::ScreenId,
    has_modal_or_overlay: bool,
    is_welcome: bool,
    is_startup_or_failed: bool,
    pending_decision_options: Option<usize>,
    has_active_detail: bool,
) -> WorkspaceLayoutGeometry {
    use crate::tui::navigation::ScreenId;

    let header_h = 2u16.min(terminal.height);
    let footer_h = 1u16.min(terminal.height.saturating_sub(header_h));
    let ws_h = terminal.height.saturating_sub(header_h + footer_h);
    let header_area = Rect::new(terminal.x, terminal.y, terminal.width, header_h);
    let footer_area = Rect::new(
        terminal.x,
        terminal.y + terminal.height.saturating_sub(footer_h),
        terminal.width,
        footer_h,
    );
    let workspace_area = Rect::new(terminal.x, terminal.y + header_h, terminal.width, ws_h);

    if terminal.width < 10 || terminal.height < 6 {
        return WorkspaceLayoutGeometry {
            terminal_area: terminal,
            header_area,
            footer_area,
            workspace_area,
            rail_area: None,
            composer_area: None,
            detail_area: None,
            conversation_area: None,
            side_panel_area: None,
            modal_or_overlay_active: has_modal_or_overlay,
        };
    }

    if is_startup_or_failed || (is_welcome && screen == ScreenId::Dashboard) {
        let bottom_height = 4u16;
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(bottom_height)])
            .split(workspace_area);
        return WorkspaceLayoutGeometry {
            terminal_area: terminal,
            header_area,
            footer_area,
            workspace_area,
            rail_area: None,
            composer_area: Some(chunks[1]),
            detail_area: None,
            conversation_area: Some(chunks[0]),
            side_panel_area: None,
            modal_or_overlay_active: has_modal_or_overlay,
        };
    }

    let show_context_rail = terminal.width >= 120 && screen == ScreenId::Dashboard;
    let rail_width = if show_context_rail {
        28.min(terminal.width / 4).max(24)
    } else {
        0
    };
    let left_width = terminal.width.saturating_sub(rail_width);
    let left_area = Rect::new(
        workspace_area.x,
        workspace_area.y,
        left_width,
        workspace_area.height,
    );
    let rail_area = if show_context_rail {
        Some(Rect::new(
            workspace_area.x + left_width,
            workspace_area.y,
            rail_width,
            workspace_area.height,
        ))
    } else {
        None
    };

    let bottom_height = if let Some(opts) = pending_decision_options {
        (opts as u16 + 5).clamp(6, 12)
    } else {
        4
    };

    let workspace_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(bottom_height)])
        .split(left_area);
    let upper_area = workspace_chunks[0];
    let composer_area = Some(workspace_chunks[1]);

    let (main_area, detail_area) = if has_active_detail {
        if upper_area.height >= 14 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(4), Constraint::Length(10)])
                .split(upper_area);
            (chunks[0], Some(chunks[1]))
        } else {
            (upper_area, Some(upper_area))
        }
    } else {
        (upper_area, None)
    };

    let (conversation_area, side_panel_area) = match screen {
        ScreenId::Dashboard => {
            let convo_area = crate::tui::shell::workspace::constrain_reading_width(main_area, 120);
            (Some(convo_area), None)
        }
        ScreenId::Git => {
            if main_area.width >= 100 {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                    .split(main_area);
                (Some(cols[0]), Some(cols[1]))
            } else {
                (None, Some(main_area))
            }
        }
        ScreenId::TaskGraph
        | ScreenId::Agents
        | ScreenId::Tools
        | ScreenId::Verification
        | ScreenId::Jobs
        | ScreenId::Doctor
        | ScreenId::ModelUsage
        | ScreenId::Artifacts => {
            if main_area.width >= 100 {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(main_area);
                (Some(cols[0]), Some(cols[1]))
            } else {
                (None, Some(main_area))
            }
        }
        ScreenId::Mission | ScreenId::Approvals | ScreenId::Logs | ScreenId::Help => {
            if main_area.width >= 100 {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(main_area);
                (Some(cols[0]), Some(cols[1]))
            } else {
                (None, Some(main_area))
            }
        }
        ScreenId::Replay | ScreenId::Settings => (None, Some(main_area)),
    };

    WorkspaceLayoutGeometry {
        terminal_area: terminal,
        header_area,
        footer_area,
        workspace_area,
        rail_area,
        composer_area,
        detail_area,
        conversation_area,
        side_panel_area,
        modal_or_overlay_active: has_modal_or_overlay,
    }
}
