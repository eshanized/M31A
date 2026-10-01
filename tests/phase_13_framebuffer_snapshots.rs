//! Multi-Resolution Framebuffer Regression Suite with Ratatui TestBackend (QAL-01, D-09).
//!
//! Verifies structural, semantic, and visual framebuffer integrity across:
//! - 80x24 (Compact minimum usability)
//! - 96x24 (Standard classic terminal)
//! - 100x30 (Standard default)
//! - 120x36 (Large developer display)
//! - 160x48 (Ultra-wide multi-pane cockpit)
//!
//! Asserts secret masking and zero cleartext credential exposure (T-13-14).

mod common;

use common::tui_fixtures::{assert_buffer_secret_scrubbing, create_mock_tui_app, render_to_buffer};
use m31a::tui::input::text_input::{InputMasking, TextInput};
use m31a::tui::screens::wizard::SetupWizardScreen;
use m31a::tui::{LayoutTier, classify_terminal_size, compute_layout};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;

#[test]
fn test_framebuffer_compact_80x24() {
    let mut app = create_mock_tui_app();
    let (buffer, content) = render_to_buffer(&mut app, 80, 24);

    // 1. Structural assertions
    assert_eq!(buffer.area.width, 80);
    assert_eq!(buffer.area.height, 24);
    assert_eq!(classify_terminal_size(80, 24), LayoutTier::Compact);

    let (tier, areas) = compute_layout(buffer.area);
    assert_eq!(tier, LayoutTier::Compact);
    assert_eq!(areas.header.height, 2);
    assert_eq!(areas.footer.height, 2);
    assert_eq!(areas.main.height, 20);
    assert!(areas.sidebar.is_none());
    assert!(areas.telemetry.is_none());

    // 2. Semantic assertions
    assert!(content.contains("M31A"));
    assert!(content.contains("EXECUTING"));
    assert!(content.contains("OAuth2") || content.contains("Auth"));
    assert!(content.contains("[1]") || content.contains("Dashboard"));

    // 3. Zero out-of-bounds writes
    for y in 0..24 {
        for x in 0..80 {
            let symbol = buffer[(x, y)].symbol();
            assert!(!symbol.is_empty());
        }
    }
}

#[test]
fn test_framebuffer_classic_96x24() {
    let mut app = create_mock_tui_app();
    let (buffer, content) = render_to_buffer(&mut app, 96, 24);

    assert_eq!(buffer.area.width, 96);
    assert_eq!(buffer.area.height, 24);
    assert_eq!(classify_terminal_size(96, 24), LayoutTier::Compact);

    assert!(content.contains("M31A"));
    assert!(content.contains("MISSION COCKPIT OVERVIEW") || content.contains("Dashboard"));
    assert!(content.contains("Recent Logs"));
}

#[test]
fn test_framebuffer_standard_100x30() {
    let mut app = create_mock_tui_app();
    let (buffer, content) = render_to_buffer(&mut app, 100, 30);

    assert_eq!(buffer.area.width, 100);
    assert_eq!(buffer.area.height, 30);
    assert_eq!(classify_terminal_size(100, 30), LayoutTier::Standard);

    let (tier, areas) = compute_layout(buffer.area);
    assert_eq!(tier, LayoutTier::Standard);
    assert!(areas.sidebar.is_some());

    assert!(content.contains("M31A"));
    assert!(content.contains("EXECUTING"));
    assert!(content.contains("Tasks Total"));
    assert!(content.contains("Active Agents"));
}

#[test]
fn test_framebuffer_large_120x36() {
    let mut app = create_mock_tui_app();
    let (buffer, content) = render_to_buffer(&mut app, 120, 36);

    assert_eq!(buffer.area.width, 120);
    assert_eq!(buffer.area.height, 36);
    assert_eq!(classify_terminal_size(120, 36), LayoutTier::Standard);

    assert!(content.contains("M31A"));
    assert!(content.contains("Tasks Total"));
    assert!(content.contains("Pending Approvals"));
}

#[test]
fn test_framebuffer_ultrawide_160x48() {
    let mut app = create_mock_tui_app();
    let (buffer, content) = render_to_buffer(&mut app, 160, 48);

    assert_eq!(buffer.area.width, 160);
    assert_eq!(buffer.area.height, 48);
    assert_eq!(classify_terminal_size(160, 48), LayoutTier::Large);

    let (tier, areas) = compute_layout(buffer.area);
    assert_eq!(tier, LayoutTier::Large);
    assert!(areas.sidebar.is_some());
    assert!(areas.telemetry.is_some());

    assert!(content.contains("M31A"));
    assert!(content.contains("MISSION COCKPIT OVERVIEW"));
    assert!(content.contains("Active Agents"));
    assert!(content.contains("Events Processed"));
}

#[test]
fn test_framebuffer_secret_scrubbing_and_masking() {
    let sensitive_key = "sk-live-anthropic-super-secret-key-9999";

    // 1. Masked TextInput verification
    let mut input = TextInput::single_line().with_masking(InputMasking::Masked('*'));
    input.set_text(sensitive_key);
    let display = input.display_text();

    assert!(!display.contains(sensitive_key));
    assert!(display.chars().all(|c| c == '*'));
    assert_eq!(display.len(), sensitive_key.len());

    // 2. SetupWizardScreen secret masking verification
    let mut wizard = SetupWizardScreen::new(PathBuf::from("/mock/repo"));
    wizard.api_key_input.set_text(sensitive_key);

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("create terminal");

    terminal
        .draw(|f| {
            wizard.render(f, f.area());
        })
        .expect("draw wizard");

    let buffer = terminal.backend().buffer().clone();
    let content = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");

    assert_buffer_secret_scrubbing(&content, &[sensitive_key]);
}
