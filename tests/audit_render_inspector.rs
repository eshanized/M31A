//! Visual Inspection Harness for TUI 2.0 Convergence Audit.

mod common;

use common::tui_fixtures::{create_mock_tui_app, render_to_buffer};
use m31a::tui::TuiApp;
use m31a::tui::navigation::ScreenId;

#[test]
fn inspect_visual_layouts() {
    // 1. IDLE / COMPOSER FOCUSED (100x30)
    let mut idle_app = TuiApp::new().with_composer_focused(true);
    let (_, content) = render_to_buffer(&mut idle_app, 100, 30);
    assert!(content.contains("M31A Cockpit"));
    assert!(content.contains("Dashboard [1]"));
    assert!(content.contains("Conversation Timeline"));
    assert!(content.contains("m31a> Ask M31A a task or command..."));
    assert!(content.contains("[COMPOSER]"));

    // 2. ACTIVE MISSION (120x36)
    let mut active_app = create_mock_tui_app();
    let (_, content) = render_to_buffer(&mut active_app, 120, 36);
    assert!(content.contains("M31A Cockpit"));
    assert!(content.contains("Task 2/3"));
    assert!(content.contains("Mission Cockpit Overview"));
    assert!(content.contains("Composer"));

    // 3. AGENTS SCREEN (120x36) - Contextual split, retains conversation and composer
    active_app.navigation.navigate_to(ScreenId::Agents);
    let (_, content) = render_to_buffer(&mut active_app, 120, 36);
    assert!(content.contains("Conversation Timeline"));
    assert!(content.contains("Swarm Directory"));
    assert!(content.contains("LeadOrchestrator"));
    assert!(content.contains("Composer"));

    // 4. TASKS SCREEN (120x36) - Contextual split, retains conversation and composer
    active_app.navigation.navigate_to(ScreenId::TaskGraph);
    let (_, content) = render_to_buffer(&mut active_app, 120, 36);
    assert!(content.contains("Conversation Timeline"));
    assert!(content.contains("DAG Tasks"));
    assert!(content.contains("Database schema migration"));
    assert!(content.contains("Composer"));

    // 5. GIT DIFF SCREEN (120x36) - Contextual split, retains conversation and composer
    active_app.navigation.navigate_to(ScreenId::Git);
    let (_, content) = render_to_buffer(&mut active_app, 120, 36);
    assert!(content.contains("Conversation Timeline"));
    assert!(content.contains("Git Worktree Attribution"));
    assert!(content.contains("Diff Viewer"));
    assert!(content.contains("Composer"));

    // 6. COMPACT 80x24 (MINIMUM SIZE) - No double headers, no boxitis
    active_app.navigation.navigate_to(ScreenId::Dashboard);
    let (_, content) = render_to_buffer(&mut active_app, 80, 24);
    assert!(content.contains("M31A Cockpit"));
    assert!(content.contains("Dashboard [1]"));
    assert!(content.contains("OAuth2"));
    assert!(content.contains("Conversation Timeline"));
    assert!(content.contains("Composer"));
}
