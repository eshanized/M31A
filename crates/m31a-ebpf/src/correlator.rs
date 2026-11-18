//! Execution Context Correlation & Process Lineage Tracker.
//!
//! Maps operating system PIDs and PPIDs to M31A sandbox sessions and turn IDs,
//! allowing kernel events to be accurately attributed to specific agent actions.

use std::collections::HashMap;
use std::sync::RwLock;

/// Active execution identity for correlation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionProvenance {
    pub turn_id: String,
    pub sandbox_id: String,
    pub root_pid: u32,
    pub command_summary: String,
}

pub struct ExecutionCorrelator {
    /// PID -> ExecutionProvenance mapping
    active_pids: RwLock<HashMap<u32, ExecutionProvenance>>,
}

impl Default for ExecutionCorrelator {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionCorrelator {
    pub fn new() -> Self {
        Self {
            active_pids: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a newly launched sandboxed root process.
    pub fn register_execution(
        &self,
        pid: u32,
        turn_id: impl Into<String>,
        sandbox_id: impl Into<String>,
        command_summary: impl Into<String>,
    ) {
        let provenance = ExecutionProvenance {
            turn_id: turn_id.into(),
            sandbox_id: sandbox_id.into(),
            root_pid: pid,
            command_summary: command_summary.into(),
        };

        if let Ok(mut map) = self.active_pids.write() {
            map.insert(pid, provenance);
        }
    }

    /// Propagates provenance to child processes when `execve` or `fork` occurs.
    pub fn inherit_or_lookup(&self, pid: u32, ppid: u32) -> Option<ExecutionProvenance> {
        let read_guard = self.active_pids.read().ok()?;
        if let Some(prov) = read_guard.get(&pid) {
            return Some(prov.clone());
        }

        // Check if parent process is registered in our provenance tree
        if let Some(parent_prov) = read_guard.get(&ppid) {
            let child_prov = parent_prov.clone();
            drop(read_guard);
            if let Ok(mut write_guard) = self.active_pids.write() {
                write_guard.insert(pid, child_prov.clone());
            }
            return Some(child_prov);
        }

        None
    }

    /// Cleans up PID mapping when a process exits.
    pub fn deregister_pid(&self, pid: u32) -> Option<ExecutionProvenance> {
        if let Ok(mut map) = self.active_pids.write() {
            map.remove(&pid)
        } else {
            None
        }
    }

    pub fn active_count(&self) -> usize {
        self.active_pids.read().map(|m| m.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correlator_lineage_inheritance() {
        let correlator = ExecutionCorrelator::new();
        correlator.register_execution(1001, "turn-42", "sb-linux-1", "npm test");

        // Lookup root
        let prov = correlator.inherit_or_lookup(1001, 1).unwrap();
        assert_eq!(prov.turn_id, "turn-42");
        assert_eq!(prov.root_pid, 1001);

        // Child process 1002 spawned by 1001 inherits provenance
        let child_prov = correlator.inherit_or_lookup(1002, 1001).unwrap();
        assert_eq!(child_prov.turn_id, "turn-42");
        assert_eq!(child_prov.root_pid, 1001);

        // Grandchild process 1003 spawned by 1002
        let grand_child = correlator.inherit_or_lookup(1003, 1002).unwrap();
        assert_eq!(grand_child.turn_id, "turn-42");

        // Clean up
        assert!(correlator.deregister_pid(1001).is_some());
        assert!(correlator.deregister_pid(9999).is_none());
    }
}
