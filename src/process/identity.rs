//! Platform-neutral process identity and instance verification (TL-02, JOB-02).
//!
//! Provides durable representation for verifying that a process identifier (PID)
//! continues to refer to the exact same process instance across restarts and checkpoints,
//! protecting against OS identifier recycling hazards without embedding Linux-specific
//! kernel structures in generic runtime data models.

use serde::{Deserialize, Serialize};

/// Platform-neutral durable process identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessIdentity {
    /// Operating system process identifier.
    pub pid: u32,
    /// Stable process creation/start timestamp (ticks, microseconds, or 100ns units).
    #[serde(alias = "linux_starttime")]
    pub start_time: Option<u64>,
    /// Platform family name on which this identity was captured.
    #[serde(default = "default_platform_name")]
    pub platform: String,
}

fn default_platform_name() -> String {
    std::env::consts::OS.to_string()
}

impl ProcessIdentity {
    /// Capture process identity for a live process identifier.
    pub fn capture(pid: u32) -> Self {
        let start_time = crate::platform::process::read_process_starttime(pid);
        Self {
            pid,
            start_time,
            platform: std::env::consts::OS.to_string(),
        }
    }

    /// Check if the recorded identity matches the current live identity for the PID.
    pub fn matches(&self, current: &Self) -> bool {
        if self.pid != current.pid {
            return false;
        }
        match (self.start_time, current.start_time) {
            (Some(a), Some(b)) => a == b,
            // If neither host interface provided start_time, identity cannot be proven
            _ => false,
        }
    }

    /// Whether this process identity represents a verified live instance.
    pub fn is_live(&self) -> bool {
        self.start_time.is_some()
    }

    /// Parse from stored recovery JSON metadata, supporting legacy `linux_starttime`.
    pub fn parse_metadata(json_str: &str) -> Option<Self> {
        serde_json::from_str::<Self>(json_str).ok().or_else(|| {
            let v = serde_json::from_str::<serde_json::Value>(json_str).ok()?;
            let pid = v.get("pid").and_then(|p| p.as_u64())? as u32;
            let start_time = v
                .get("start_time")
                .or_else(|| v.get("linux_starttime"))
                .and_then(|s| s.as_u64());
            let platform = v
                .get("platform")
                .and_then(|p| p.as_str())
                .unwrap_or(std::env::consts::OS)
                .to_string();
            Some(Self {
                pid,
                start_time,
                platform,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_identity_roundtrip() {
        let ident = ProcessIdentity {
            pid: 1234,
            start_time: Some(9999),
            platform: "linux".to_string(),
        };
        let s = serde_json::to_string(&ident).unwrap();
        let parsed: ProcessIdentity = serde_json::from_str(&s).unwrap();
        assert_eq!(ident, parsed);
        assert!(ident.matches(&parsed));
    }

    #[test]
    fn test_process_identity_backward_compatibility() {
        let legacy_json = r#"{"pid":4321,"linux_starttime":5555}"#;
        let parsed = ProcessIdentity::parse_metadata(legacy_json).unwrap();
        assert_eq!(parsed.pid, 4321);
        assert_eq!(parsed.start_time, Some(5555));
    }
}
