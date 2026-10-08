//! Visual Inspection Harness for conversation-first TUI convergence audit.

mod common;

use common::tui_fixtures::{create_mock_tui_app, render_to_buffer};
use m31a::tui::TuiApplication;
use m31a::tui::navigation::ScreenId;

#[test]
fn inspect_visual_layouts() {
    // 1. IDLE / COMPOSER FOCUSED (100x30) — quiet onboarding.
    let mut idle_app = TuiApplication::new().with_composer_focused(true);
    let (_, content) = render_to_buffer(&mut idle_app, 100, 30);
    assert!(content.contains("M31A"));
    assert!(content.contains("Your autonomous software engineering workspace"));
    assert!(content.contains("/ for commands"));
    assert!(content.contains("@ for files"));
    assert!(!content.contains("M31A Cockpit"));
    assert!(!content.contains("[COMPOSER]"));

    // 2. ACTIVE MISSION (120x36) — conversation-first, quiet working state.
    let mut active_app = create_mock_tui_app();
    let (_, content) = render_to_buffer(&mut active_app, 120, 36);
    assert!(content.contains("M31A"));
    assert!(content.contains("Working") || content.contains("Waiting"));
    assert!(!content.contains("Mission Cockpit Overview"));
    assert!(!content.contains("M31A Cockpit"));

    // 3. AGENTS SCREEN (120x36) — contextual detail with live inventory.
    active_app.navigation.navigate_to(ScreenId::Agents);
    let (_, content) = render_to_buffer(&mut active_app, 120, 36);
    assert!(content.contains("LeadOrchestrator"));

    // 4. TASKS SCREEN (120x36) — task inventory preserved.
    active_app.navigation.navigate_to(ScreenId::TaskGraph);
    let (_, content) = render_to_buffer(&mut active_app, 120, 36);
    assert!(content.contains("Database schema migration"));

    // 5. GIT SCREEN (120x36) — git surface reachable.
    active_app.navigation.navigate_to(ScreenId::Git);
    let (_, content) = render_to_buffer(&mut active_app, 120, 36);
    assert!(content.contains("M31A"));

    // 6. COMPACT 80x24 (MINIMUM SIZE) — quiet, no boxitis.
    active_app.navigation.navigate_to(ScreenId::Dashboard);
    let (buffer, content) = render_to_buffer(&mut active_app, 80, 24);
    assert!(content.contains("M31A"));
    assert!(buffer.area.width == 80);
    let corners = content.chars().filter(|c| *c == '┌' || *c == '┘').count();
    assert_eq!(corners, 0, "default cockpit must not use boxed panels");
}
