//! Dedicated integration tests for `m31a-compaction` (src/monitor.rs).
//!
//! Validates:
//! - ContextHeadroomMonitor initialization and threshold configuration
//! - Severity classification (Nominal, Warning, Critical, Exhausted)
//! - Advisory message and should_compact recommendation logic
//! - Progress bar rendering geometry and formatting

use m31a_compaction::{ContextHeadroomMonitor, HeadroomSeverity, WARNING_HEADROOM_PCT};

#[test]
fn test_headroom_monitor_severity_transitions() {
    let limit = 100_000;
    let monitor = ContextHeadroomMonitor::new(limit);

    // 1. Nominal: used 50k (50% headroom remaining)
    let snap_nominal = monitor.evaluate(50_000);
    assert_eq!(snap_nominal.severity, HeadroomSeverity::Nominal);
    assert!(!snap_nominal.should_compact);
    assert!(snap_nominal.headroom_pct >= WARNING_HEADROOM_PCT);

    // 2. Warning: used 70k (30% headroom remaining, below 35%)
    let snap_warning = monitor.evaluate(70_000);
    assert_eq!(snap_warning.severity, HeadroomSeverity::Warning);
    assert!(!snap_warning.should_compact);
    assert!(snap_warning.advisory_message.is_some());
    assert!(snap_warning.advisory_message.unwrap().contains("advisory"));

    // 3. Critical: used 80k (20% headroom remaining, below 25%)
    let snap_critical = monitor.evaluate(80_000);
    assert_eq!(snap_critical.severity, HeadroomSeverity::Critical);
    assert!(snap_critical.should_compact);
    assert!(snap_critical.advisory_message.is_some());
    assert!(snap_critical.advisory_message.unwrap().contains("headroom low"));

    // 4. Exhausted: used 97k (3% headroom remaining, below 5%)
    let snap_exhausted = monitor.evaluate(97_000);
    assert_eq!(snap_exhausted.severity, HeadroomSeverity::Exhausted);
    assert!(snap_exhausted.should_compact);
    assert!(snap_exhausted.advisory_message.is_some());
    assert!(snap_exhausted.advisory_message.unwrap().contains("exhausted"));
}

#[test]
fn test_custom_thresholds_configuration() {
    let monitor = ContextHeadroomMonitor::with_thresholds(10_000, 0.40, 0.15);
    // 35% headroom remaining -> in custom config (warning=40%), this should be Warning
    let snap = monitor.evaluate(6_500);
    assert_eq!(snap.severity, HeadroomSeverity::Warning);
}

#[test]
fn test_render_progress_bar() {
    let monitor = ContextHeadroomMonitor::new(10_000);
    let snap = monitor.evaluate(7_500); // 75% used, 25% headroom -> Critical

    let bar = snap.render_progress_bar(20);
    assert!(bar.starts_with('['));
    assert!(bar.contains("75.0% used"));
    assert!(bar.contains("25.0% headroom - CRITICAL"));
    assert!(bar.contains('█'));
    assert!(bar.contains('░'));
}
