//! Context Headroom Monitor (`m31a-compaction::monitor`).
//!
//! Dynamic context window budgeting and proactive token headroom monitoring:
//! - Tracks token headroom within the model's context window.
//! - Emits advisory warnings when headroom falls below 35% (Warning) and 25% (Critical).
//! - Signals early compaction to prevent degraded subagent context performance.
//! - Renders visual progress bars for TUI / CLI integration.

use serde::{Deserialize, Serialize};

/// 35% remaining headroom threshold.
pub const WARNING_HEADROOM_PCT: f32 = 0.35;
/// 25% remaining headroom threshold.
pub const CRITICAL_HEADROOM_PCT: f32 = 0.25;
/// 5% remaining headroom threshold.
pub const EXHAUSTED_HEADROOM_PCT: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HeadroomSeverity {
    Nominal,
    Warning,
    Critical,
    Exhausted,
}

impl HeadroomSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Nominal => "NOMINAL",
            Self::Warning => "WARNING",
            Self::Critical => "CRITICAL",
            Self::Exhausted => "EXHAUSTED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadroomSnapshot {
    pub context_limit: usize,
    pub used_tokens: usize,
    pub headroom_tokens: usize,
    pub headroom_pct: f32,
    pub used_pct: f32,
    pub severity: HeadroomSeverity,
    pub should_compact: bool,
    pub advisory_message: Option<String>,
}

impl HeadroomSnapshot {
    /// Renders an ASCII/Unicode progress bar suitable for TUI or CLI displays.
    /// Example: `[████████████░░░░] 75.0% used (25.0% headroom - CRITICAL)`
    pub fn render_progress_bar(&self, width: usize) -> String {
        let bar_width = width.max(10);
        let fill_count = ((self.used_pct * bar_width as f32).round() as usize).min(bar_width);
        let empty_count = bar_width.saturating_sub(fill_count);

        let filled = "█".repeat(fill_count);
        let empty = "░".repeat(empty_count);

        format!(
            "[{}{}] {:.1}% used ({:.1}% headroom - {})",
            filled,
            empty,
            self.used_pct * 100.0,
            self.headroom_pct * 100.0,
            self.severity.as_str()
        )
    }
}

#[derive(Debug, Clone)]
pub struct ContextHeadroomMonitor {
    context_limit: usize,
    warning_threshold: f32,
    critical_threshold: f32,
}

impl ContextHeadroomMonitor {
    pub fn new(context_limit: usize) -> Self {
        Self {
            context_limit: context_limit.max(1000),
            warning_threshold: WARNING_HEADROOM_PCT,
            critical_threshold: CRITICAL_HEADROOM_PCT,
        }
    }

    pub fn with_thresholds(
        context_limit: usize,
        warning_threshold: f32,
        critical_threshold: f32,
    ) -> Self {
        Self {
            context_limit: context_limit.max(1000),
            warning_threshold: warning_threshold.clamp(0.0, 1.0),
            critical_threshold: critical_threshold.clamp(0.0, warning_threshold),
        }
    }

    /// Evaluates current token usage and generates a `HeadroomSnapshot`.
    pub fn evaluate(&self, used_tokens: usize) -> HeadroomSnapshot {
        let used = used_tokens.min(self.context_limit);
        let headroom = self.context_limit.saturating_sub(used_tokens);
        let used_pct = used as f32 / self.context_limit as f32;
        let headroom_pct = (headroom as f32 / self.context_limit as f32).max(0.0);

        let severity = if headroom_pct <= EXHAUSTED_HEADROOM_PCT {
            HeadroomSeverity::Exhausted
        } else if headroom_pct <= self.critical_threshold {
            HeadroomSeverity::Critical
        } else if headroom_pct <= self.warning_threshold {
            HeadroomSeverity::Warning
        } else {
            HeadroomSeverity::Nominal
        };

        let should_compact = severity >= HeadroomSeverity::Critical;

        let advisory_message = match severity {
            HeadroomSeverity::Exhausted => Some(format!(
                "Context exhausted ({:.1}% used)! Immediate compaction required before agent context failure.",
                used_pct * 100.0
            )),
            HeadroomSeverity::Critical => Some(format!(
                "Context headroom low ({:.1}% remaining, threshold {:.1}%). Auto-compaction recommended.",
                headroom_pct * 100.0,
                self.critical_threshold * 100.0
            )),
            HeadroomSeverity::Warning => Some(format!(
                "Context headroom advisory: {:.1}% remaining. Consider checkpointing active tasks.",
                headroom_pct * 100.0
            )),
            HeadroomSeverity::Nominal => None,
        };

        HeadroomSnapshot {
            context_limit: self.context_limit,
            used_tokens,
            headroom_tokens: headroom,
            headroom_pct,
            used_pct,
            severity,
            should_compact,
            advisory_message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headroom_severities() {
        let monitor = ContextHeadroomMonitor::new(100_000);

        // 50% usage -> Nominal (50% headroom)
        let s1 = monitor.evaluate(50_000);
        assert_eq!(s1.severity, HeadroomSeverity::Nominal);
        assert!(!s1.should_compact);
        assert!(s1.advisory_message.is_none());

        // 70% usage -> Warning (30% headroom <= 35%)
        let s2 = monitor.evaluate(70_000);
        assert_eq!(s2.severity, HeadroomSeverity::Warning);
        assert!(!s2.should_compact);
        assert!(s2.advisory_message.is_some());

        // 80% usage -> Critical (20% headroom <= 25%)
        let s3 = monitor.evaluate(80_000);
        assert_eq!(s3.severity, HeadroomSeverity::Critical);
        assert!(s3.should_compact);

        // 98% usage -> Exhausted (2% headroom <= 5%)
        let s4 = monitor.evaluate(98_000);
        assert_eq!(s4.severity, HeadroomSeverity::Exhausted);
        assert!(s4.should_compact);
    }

    #[test]
    fn test_render_progress_bar() {
        let monitor = ContextHeadroomMonitor::new(100_000);
        let snap = monitor.evaluate(75_000);
        let bar = snap.render_progress_bar(20);

        assert!(bar.contains("75.0% used"));
        assert!(bar.contains("25.0% headroom - CRITICAL"));
        assert!(bar.starts_with('['));
    }
}
