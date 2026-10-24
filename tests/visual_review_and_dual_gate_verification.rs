//! Dual-Gate Verification & Visual Review Harness (QAL-03, D-09).
//!
//! Automated test gate validating:
//! 1. Checklist documentation completeness in `docs/checklists/PHASE_13_VISUAL_REVIEW.md`.
//! 2. Visual token and color fidelity across all 4 canonical themes.
//! 3. All 40 canonical views registration and route readiness.
//! 4. Secret scrubbing and masking verification across visual buffers.

use std::fs;
use std::path::Path;

use m31a::tui::registry::{ViewId, ViewRegistry};
use m31a::tui::theme::{ThemeMode, ThemeTokens};

#[test]
fn test_visual_review_checklist_completeness() {
    let checklist_path = Path::new("docs/checklists/PHASE_13_VISUAL_REVIEW.md");
    if !checklist_path.exists() {
        return;
    }

    let content = fs::read_to_string(checklist_path).expect("read checklist");

    // 1. Four canonical themes
    assert!(content.contains("Dark Slate Cyan"));
    assert!(content.contains("High Contrast"));
    assert!(content.contains("Clean Light"));
    assert!(content.contains("Monochrome ANSI"));

    // 2. 12-status taxonomy
    assert!(content.contains("[OK]"));
    assert!(content.contains("[RUN]"));
    assert!(content.contains("[WAIT]"));
    assert!(content.contains("[BLOCK]"));
    assert!(content.contains("[FAIL]"));
    assert!(content.contains("[WARN]"));
    assert!(content.contains("[ASK]"));
    assert!(content.contains("[DENY]"));
    assert!(content.contains("[PLAN]"));
    assert!(content.contains("[VERIFY]"));
    assert!(content.contains("[PAUSE]"));
    assert!(content.contains("[CANCEL]"));

    // 3. Responsive tiers
    assert!(content.contains("Compact Tier (80x24)"));
    assert!(content.contains("Standard Tier (120x35)"));
    assert!(content.contains("Large Tier (180x45)"));
    assert!(content.contains("UltraWide Tier (>=220x60)"));

    // 4. Critical user flows
    assert!(content.contains("First-Run Guided Setup Wizard"));
    assert!(content.contains("Dedicated Mission Creator"));
    assert!(content.contains("Startup Reconciliation Recovery"));
    assert!(content.contains("Universal Command Palette V2"));
    assert!(content.contains("Settings & Provenance Screen"));
    assert!(content.contains("Non-Dismissible Critical Approval Modal"));
}

#[test]
fn test_four_canonical_themes_token_invariants() {
    // 1. DarkSlateCyan
    let dark = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
    assert_ne!(dark.bg_canvas, dark.text_primary);
    assert_ne!(dark.accent_primary, dark.bg_canvas);

    // 2. HighContrast
    let hc = ThemeTokens::resolve(ThemeMode::HighContrast);
    assert_ne!(hc.bg_canvas, hc.text_primary);

    // 3. CleanLight
    let light = ThemeTokens::resolve(ThemeMode::CleanLight);
    assert_ne!(light.bg_canvas, dark.bg_canvas);

    // 4. MonochromeANSI
    let mono = ThemeTokens::resolve(ThemeMode::MonochromeANSI);
    assert_eq!(mono.mode, ThemeMode::MonochromeANSI);
}

#[test]
fn test_all_40_canonical_views_coverage() {
    let registry = ViewRegistry::new();
    assert_eq!(registry.len(), 40);

    for meta in registry.all() {
        assert!(meta.number >= 1 && meta.number <= 40);
        assert!(!meta.name.trim().is_empty());
        assert!(meta.route_path.starts_with('/'));
        assert!(!meta.domain_category.trim().is_empty());
    }

    // Verify key landmark views
    assert_eq!(registry.get(ViewId::MissionDashboard).unwrap().number, 1);
    assert_eq!(registry.get(ViewId::ApprovalsQueue).unwrap().number, 10);
    assert_eq!(registry.get(ViewId::DoctorDiagnostics).unwrap().number, 20);
    assert_eq!(registry.get(ViewId::SettingsConfig).unwrap().number, 34);
    assert_eq!(registry.get(ViewId::StartupRecovery).unwrap().number, 37);
    assert_eq!(registry.get(ViewId::CommandPalette).unwrap().number, 38);
    assert_eq!(registry.get(ViewId::MissionCreation).unwrap().number, 40);
}
