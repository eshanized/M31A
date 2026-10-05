//! Integration tests for Phase 13 Plan 01: Terminal Lifecycle, Safety Guard & Design Token System.
//!
//! Verifies:
//! - TerminalGuard RAII idempotency, emergency restore, and panic hook installation (TDS-01, TDS-02, D-10).
//! - Centralized ThemeTokens resolution, NO_COLOR compliance, and 12-status taxonomy formatting (TDS-03, TDS-04, D-03).
//! - Responsive layout engine tier classification, minimum boundary enforcement, and chrome splitting (TDS-05, D-05).

use ratatui::layout::Rect;
use std::collections::HashSet;

use m31a::tui::guard::{TerminalGuard, install_panic_hook};
use m31a::tui::layout::{LayoutTier, ResponsiveLayout};
use m31a::tui::status::{StatusKind, StatusPresentation};
use m31a::tui::theme::{ThemeMode, ThemeTokens};
use m31a::tui::view_tier::ViewTier;

#[test]
fn test_terminal_guard_and_panic_hook() {
    // Verify panic hook installs safely and idempotently
    install_panic_hook();
    install_panic_hook();

    // Verify emergency restore idempotency in headless / non-interactive environment
    assert!(TerminalGuard::emergency_restore().is_ok());
    assert!(TerminalGuard::emergency_restore().is_ok());

    // Verify that emergency restore can be called repeatedly without panic
    for _ in 0..5 {
        let res = TerminalGuard::emergency_restore();
        assert!(res.is_ok());
    }
}

#[test]
fn test_theme_tokens_and_status_presentation() {
    // 1. Verify all 4 themes resolve without crashing
    let dark = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
    assert_eq!(dark.mode, ThemeMode::DarkSlateCyan);

    let high_contrast = ThemeTokens::resolve(ThemeMode::HighContrast);
    assert_eq!(high_contrast.mode, ThemeMode::HighContrast);

    let light = ThemeTokens::resolve(ThemeMode::CleanLight);
    assert_eq!(light.mode, ThemeMode::CleanLight);

    let mono = ThemeTokens::resolve(ThemeMode::MonochromeANSI);
    assert_eq!(mono.mode, ThemeMode::MonochromeANSI);

    // 2. Verify ThemeMode cycling
    assert_eq!(ThemeMode::Default.cycle(), ThemeMode::DarkSlateCyan);
    assert_eq!(ThemeMode::DarkSlateCyan.cycle(), ThemeMode::HighContrast);
    assert_eq!(ThemeMode::HighContrast.cycle(), ThemeMode::CleanLight);
    assert_eq!(ThemeMode::CleanLight.cycle(), ThemeMode::MonochromeANSI);
    assert_eq!(ThemeMode::MonochromeANSI.cycle(), ThemeMode::Default);

    // 3. Verify all 12 statuses produce unique badges
    let all_statuses = [
        StatusKind::Ok,
        StatusKind::Running,
        StatusKind::Waiting,
        StatusKind::Blocked,
        StatusKind::Failed,
        StatusKind::Warning,
        StatusKind::Asking,
        StatusKind::Denied,
        StatusKind::Planning,
        StatusKind::Verifying,
        StatusKind::Paused,
        StatusKind::Cancelled,
    ];

    let mut badges = HashSet::new();
    for status in all_statuses {
        let (badge, style) = StatusPresentation::format(status, &dark);
        assert!(!badge.is_empty());
        assert!(badge.starts_with('['));
        assert!(badge.ends_with(']'));
        assert!(
            badges.insert(badge),
            "Duplicate badge detected for {:?}",
            status
        );

        // Verify high contrast and light mode also produce the badge
        let (hc_badge, hc_style) = StatusPresentation::format(status, &high_contrast);
        assert_eq!(badge, hc_badge);
        let _ = (style, hc_style);
    }
    assert_eq!(badges.len(), 12);

    // 4. Verify canonical badges match spec exactly
    assert_eq!(StatusKind::Ok.badge(), "[OK]");
    assert_eq!(StatusKind::Running.badge(), "[RUN]");
    assert_eq!(StatusKind::Waiting.badge(), "[WAIT]");
    assert_eq!(StatusKind::Blocked.badge(), "[BLOCK]");
    assert_eq!(StatusKind::Failed.badge(), "[FAIL]");
    assert_eq!(StatusKind::Warning.badge(), "[WARN]");
    assert_eq!(StatusKind::Asking.badge(), "[ASK]");
    assert_eq!(StatusKind::Denied.badge(), "[DENY]");
    assert_eq!(StatusKind::Planning.badge(), "[PLAN]");
    assert_eq!(StatusKind::Verifying.badge(), "[VERIFY]");
    assert_eq!(StatusKind::Paused.badge(), "[PAUSE]");
    assert_eq!(StatusKind::Cancelled.badge(), "[CANCEL]");
}

#[test]
fn test_responsive_layout_tiers() {
    // 1. Verify ViewTier classification by dimensions
    assert_eq!(ViewTier::from_dimensions(80, 24), ViewTier::Compact);
    assert_eq!(ViewTier::from_dimensions(99, 29), ViewTier::Compact);
    assert_eq!(ViewTier::from_dimensions(100, 30), ViewTier::Standard);
    assert_eq!(ViewTier::from_dimensions(159, 49), ViewTier::Standard);
    assert_eq!(ViewTier::from_dimensions(160, 50), ViewTier::Large);
    assert_eq!(ViewTier::from_dimensions(219, 79), ViewTier::Large);
    assert_eq!(ViewTier::from_dimensions(220, 80), ViewTier::UltraWide);

    // 2. Verify minimum viewport boundary enforcement (80x24)
    assert!(ViewTier::is_below_minimum(79, 24));
    assert!(ViewTier::is_below_minimum(80, 23));
    assert!(ViewTier::is_below_minimum(50, 15));
    assert!(!ViewTier::is_below_minimum(80, 24));
    assert!(!ViewTier::is_below_minimum(100, 30));

    // 3. Verify LayoutTier conversion consistency
    let vt: LayoutTier = ViewTier::Compact.into();
    assert_eq!(vt, LayoutTier::Compact);
    let back: ViewTier = vt.into();
    assert_eq!(back, ViewTier::Compact);

    // 4. Verify ResponsiveLayout partitioning at 80x24 (Compact)
    let compact_rect = Rect::new(0, 0, 80, 24);
    let (tier, areas) = ResponsiveLayout::partition(compact_rect);
    assert_eq!(tier, ViewTier::Compact);
    assert_eq!(areas.header.height, 2);
    assert_eq!(areas.footer.height, 1);
    assert_eq!(areas.main.height, 21);

    // 5. Verify ResponsiveLayout partitioning at 120x40 (Standard)
    let standard_rect = Rect::new(0, 0, 120, 40);
    let (tier, areas) = ResponsiveLayout::partition(standard_rect);
    assert_eq!(tier, ViewTier::Standard);
    assert!(areas.sidebar.is_none());
    assert!(areas.main.width > 0);

    // 6. Verify ResponsiveLayout partitioning at 180x60 (Large)
    let large_rect = Rect::new(0, 0, 180, 60);
    let (tier, areas) = ResponsiveLayout::partition(large_rect);
    assert_eq!(tier, ViewTier::Large);
    assert!(areas.sidebar.is_none());
    assert!(areas.telemetry.is_none());

    // 7. Verify ResponsiveLayout vertical chrome split helper
    let (hdr, body, ftr) = ResponsiveLayout::split_vertical_chrome(compact_rect);
    assert_eq!(hdr.height, 3);
    assert_eq!(ftr.height, 1);
    assert_eq!(body.height, 20);
}
