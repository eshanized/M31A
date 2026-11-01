//! System health gauges, process metrics, and diagnostic counters.
//!
//! Tracks active subagents, running processes, memory usage, and runtime diagnostics.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Once};

static REGISTERED_GAUGES: Mutex<Vec<&'static Gauge>> = Mutex::new(Vec::new());

/// Process-wide diagnostic gauge tracking a real-time atomic numeric indicator.
pub struct Gauge {
    name: &'static str,
    value: AtomicU64,
    registered: Once,
}

impl Gauge {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            value: AtomicU64::new(0),
            registered: Once::new(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn set(&'static self, val: u64) {
        self.register();
        self.value.store(val, Ordering::Relaxed);
    }

    pub fn increment(&'static self) {
        self.register();
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add(&'static self, delta: u64) {
        self.register();
        self.value.fetch_add(delta, Ordering::Relaxed);
    }

    pub fn decrement(&self) {
        let _ = self
            .value
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                Some(v.saturating_sub(1))
            });
    }

    /// Increments the gauge now and decrements when the returned guard is dropped.
    pub fn track(&'static self) -> GaugeGuard {
        self.increment();
        GaugeGuard { gauge: self }
    }

    fn register(&'static self) {
        self.registered.call_once(|| {
            if let Ok(mut lock) = REGISTERED_GAUGES.lock() {
                lock.push(self);
            }
        });
    }
}

pub struct GaugeGuard {
    gauge: &'static Gauge,
}

impl Drop for GaugeGuard {
    fn drop(&mut self) {
        self.gauge.decrement();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GaugeSnapshot {
    pub name: &'static str,
    pub value: u64,
}

// Core diagnostic gauges tracking subagents, background jobs, and active processes
pub static ACTIVE_SUBAGENTS: Gauge = Gauge::new("active_subagents");
pub static ACTIVE_BACKGROUND_PROCESSES: Gauge = Gauge::new("active_background_processes");
pub static ACTIVE_MCP_SERVERS: Gauge = Gauge::new("active_mcp_servers");
pub static TOTAL_PROMPT_TOKENS: Gauge = Gauge::new("total_prompt_tokens");
pub static TOTAL_COMPLETION_TOKENS: Gauge = Gauge::new("total_completion_tokens");
pub static TOOL_CALLS_EXECUTED: Gauge = Gauge::new("tool_calls_executed");
pub static STORAGE_READS: Gauge = Gauge::new("storage_reads");
pub static STORAGE_WRITES: Gauge = Gauge::new("storage_writes");

/// Takes a point-in-time snapshot of all registered diagnostic metrics.
pub fn snapshot_all() -> Vec<GaugeSnapshot> {
    // Ensure all built-in gauges are initialized & registered
    ACTIVE_SUBAGENTS.register();
    ACTIVE_BACKGROUND_PROCESSES.register();
    ACTIVE_MCP_SERVERS.register();
    TOTAL_PROMPT_TOKENS.register();
    TOTAL_COMPLETION_TOKENS.register();
    TOOL_CALLS_EXECUTED.register();
    STORAGE_READS.register();
    STORAGE_WRITES.register();

    let lock = REGISTERED_GAUGES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    lock.iter()
        .map(|g| GaugeSnapshot {
            name: g.name,
            value: g.get(),
        })
        .collect()
}

/// Renders a diagnostic telemetry report suitable for debug CLI commands and TUI status displays.
pub fn render_diagnostics_report() -> String {
    let snaps = snapshot_all();
    let mut out = String::from("## M31A Diagnostic Gauges & Telemetry\n\n");
    for s in snaps {
        out.push_str(&format!("- **{}**: {}\n", s.name, s.value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_GAUGE: Gauge = Gauge::new("test_counter");

    #[test]
    fn test_gauge_increment_decrement_and_guard() {
        TEST_GAUGE.set(0);
        assert_eq!(TEST_GAUGE.get(), 0);

        TEST_GAUGE.increment();
        assert_eq!(TEST_GAUGE.get(), 1);

        {
            let _guard = TEST_GAUGE.track();
            assert_eq!(TEST_GAUGE.get(), 2);
        }
        assert_eq!(TEST_GAUGE.get(), 1);

        TEST_GAUGE.decrement();
        assert_eq!(TEST_GAUGE.get(), 0);

        // Saturating underflow check
        TEST_GAUGE.decrement();
        assert_eq!(TEST_GAUGE.get(), 0);
    }

    #[test]
    fn test_diagnostics_snapshot_and_report() {
        ACTIVE_SUBAGENTS.set(3);
        TOOL_CALLS_EXECUTED.set(42);

        let snaps = snapshot_all();
        assert!(
            snaps
                .iter()
                .any(|s| s.name == "active_subagents" && s.value == 3)
        );
        assert!(
            snaps
                .iter()
                .any(|s| s.name == "tool_calls_executed" && s.value == 42)
        );

        let report = render_diagnostics_report();
        assert!(report.contains("**active_subagents**: 3"));
        assert!(report.contains("**tool_calls_executed**: 42"));
    }
}
