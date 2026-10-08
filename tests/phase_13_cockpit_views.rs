//! Integration Tests for Phase 13 Plan 04 (COP-03, COP-04, COP-05).
//!
//! Verifies:
//! 1. All 40 canonical views registry, metadata completeness, and navigation router.
//! 2. Mission Composer View 40 validation and RuntimeCommand dispatch handshake.
//! 3. Startup reconciliation recovery screen triage and Universal Command Palette fuzzy search.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashSet;

use m31a::interaction::action::ApplicationAction;
use m31a::tui::TuiApplication;
use m31a::tui::navigation::ScreenId;
use m31a::tui::navigation::{NavigationRouter as CanonicalRouter, canonical_screen};
use m31a::tui::palette_v2::{PaletteActionV2, UniversalCommandPalette};
use m31a::tui::registry::{ViewId, ViewKind, ViewRegistry};

#[test]
fn test_canonical_views_registry() {
    let registry = ViewRegistry::new();

    // 1. Total views must be strictly 40 (UI_UX_SPEC §113)
    assert_eq!(registry.len(), 40);
    assert_eq!(registry.all().len(), 40);

    // 2. All 40 view numbers from 1 to 40 must be uniquely populated
    let mut numbers = HashSet::new();
    let mut routes = HashSet::new();
    for meta in registry.all() {
        assert!(meta.number >= 1 && meta.number <= 40);
        assert!(
            numbers.insert(meta.number),
            "duplicate view number: {}",
            meta.number
        );
        assert!(
            routes.insert(meta.route_path),
            "duplicate route: {}",
            meta.route_path
        );
        assert!(!meta.name.is_empty());
        assert!(!meta.domain_category.is_empty());

        // Verify lookup by id, number, and route consistency
        let by_id = registry.get(meta.id).expect("lookup by id");
        let by_num = registry.get_by_number(meta.number).expect("lookup by num");
        let by_route = registry
            .get_by_route(meta.route_path)
            .expect("lookup by route");

        assert_eq!(by_id.id, meta.id);
        assert_eq!(by_num.id, meta.id);
        assert_eq!(by_route.id, meta.id);
    }
    assert_eq!(numbers.len(), 40);

    // 3. Verify specific canonical view mappings
    let v1 = registry.get(ViewId::MissionDashboard).unwrap();
    assert_eq!(v1.number, 1);
    assert_eq!(v1.kind, ViewKind::PrimaryRoute);

    let v10 = registry.get(ViewId::ApprovalsQueue).unwrap();
    assert_eq!(v10.number, 10);
    assert_eq!(v10.kind, ViewKind::ModalDialog);

    let v34 = registry.get(ViewId::SettingsConfig).unwrap();
    assert_eq!(v34.number, 34);

    let v37 = registry.get(ViewId::StartupRecovery).unwrap();
    assert_eq!(v37.number, 37);

    let v40 = registry.get(ViewId::MissionCreation).unwrap();
    assert_eq!(v40.number, 40);

    // 4. Substring and domain query search
    let matches = registry.find_by_name("dag");
    assert!(!matches.is_empty());
    assert!(matches.iter().any(|v| v.id == ViewId::DagInspector));

    let security_views = registry.find_by_name("security");
    assert!(!security_views.is_empty());

    // 5. Canonical navigation authority back-stack traversal (Phase 36.4:
    // single route authority; every registered view resolves to a functional
    // parent screen, contextual details open without leaving the session).
    let mut router = CanonicalRouter::new();
    assert_eq!(router.current_screen, ScreenId::Dashboard);
    assert!(router.history.is_empty());

    // Every registered view resolves through the single authority.
    for view in ViewId::all() {
        let _ = canonical_screen(*view);
    }

    router.navigate_to_view(ViewId::DagInspector);
    assert_eq!(router.current_screen, ScreenId::TaskGraph);
    assert_eq!(router.history, &[ScreenId::Dashboard]);

    router.navigate_to_view(ViewId::PolicyLedger);
    assert_eq!(router.current_screen, ScreenId::Approvals);
    assert_eq!(router.history, &[ScreenId::Dashboard, ScreenId::TaskGraph]);

    // Go back once
    let prev = router.pop_history();
    assert_eq!(prev, Some(ScreenId::TaskGraph));
    assert_eq!(router.current_screen, ScreenId::TaskGraph);

    // Go back again
    let root = router.pop_history();
    assert_eq!(root, Some(ScreenId::Dashboard));
    assert_eq!(router.current_screen, ScreenId::Dashboard);

    // Back on empty history returns None
    assert_eq!(router.pop_history(), None);

    // Detail & overlay inspection (contextual, session stays on route)
    router.open_detail(ViewId::TaskDetails);
    assert_eq!(router.active_detail, Some(ViewId::TaskDetails));
    router.close_detail();
    assert_eq!(router.active_detail, None);

    router.open_overlay(ViewId::CommandPalette);
    assert_eq!(router.active_overlay, Some(ViewId::CommandPalette));
    router.close_overlay();
    assert_eq!(router.active_overlay, None);

    // Contextual views resolve without dropping navigation: a modal view
    // opens on its parent route instead of vanishing.
    router.navigate_to_view(ViewId::ApprovalsQueue);
    assert_eq!(router.current_screen, ScreenId::Approvals);
    assert_eq!(router.active_overlay, Some(ViewId::ApprovalsQueue));
}

#[test]
fn test_single_canonical_command_path_via_bridge() {
    // There is exactly one TUI command transport: input → ApplicationAction
    // → bridge sender → runtime. No parallel dispatch bridge exists.
    // Ctrl+C cancellation must travel the bridge (fail-closed when absent).
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = TuiApplication::new()
        .with_bridge_tx(tx)
        .with_composer_focused(true);

    let out = app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(out.is_none(), "bridge owns cancel; no degraded dispatch");

    let got = rx
        .try_recv()
        .expect("cancel must travel the single bridge path");
    assert!(
        matches!(got, ApplicationAction::CancelRequested),
        "unexpected action on bridge: {got:?}"
    );
}

#[test]
fn test_command_palette() {
    // Universal Command Palette (View 38, COP-03, T-13-12)
    let mut palette = UniversalCommandPalette::new();
    assert!(!palette.is_open());

    palette.open();
    assert!(palette.is_open());

    // Initially all 40+ commands are listed
    let initial_count = palette.filtered_items().len();
    assert!(initial_count >= 40);

    // Fuzzy search for "dag"
    let _ = palette.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty()));
    let _ = palette.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
    let _ = palette.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()));

    let filtered = palette.filtered_items();
    assert!(!filtered.is_empty());
    assert_eq!(
        filtered[0].action,
        PaletteActionV2::NavigateView(ViewId::DagInspector)
    );

    // Select with Enter closes palette and returns action
    let action = palette.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_eq!(
        action,
        Some(PaletteActionV2::NavigateView(ViewId::DagInspector))
    );
    assert!(!palette.is_open());
}
