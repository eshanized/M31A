//! Adaptive Problem-Solving & Tool Oscillation / Stuck-Loop Detector (`m31a-agent-engine::oscillation`).
//!
//! Tracks tool call execution history and diagnostics to detect:
//! 1. Identical Repeating Calls: Agent repeatedly invoking the exact same command or edit.
//! 2. Ping-Pong Tool Oscillation: Agent alternating back and forth between two states (e.g. A -> B -> A -> B).
//! 3. Unrevised Repeated Failures: Repeating failed actions without addressing the error or revising the approach.
//!
//! Synthesizes steering interventions to break cycles and redirect the model toward adaptive problem-solving.

use m31a_core::types::ToolCall;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

/// Category of detected execution cycle or stuck loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OscillationKind {
    /// Exact same tool and arguments called repeatedly.
    IdenticalCallLoop { tool_name: String, count: usize },
    /// Alternating oscillation between two repeating tool calls.
    PingPongOscillation { tool_a: String, tool_b: String },
    /// Repeated failure with identical error output.
    RepeatedFailureWithoutRevision {
        tool_name: String,
        error_snippet: String,
    },
}

/// Alert raised when execution enters a non-progressing loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OscillationAlert {
    pub kind: OscillationKind,
    pub description: String,
    pub adaptive_steering_prompt: String,
}

#[derive(Debug, Clone)]
struct ToolExecutionRecord {
    pub tool_name: String,
    pub args_hash: u64,
    pub is_error: bool,
    pub error_hash: Option<u64>,
}

/// Ring buffer tracker for adaptive loop detection.
#[derive(Debug, Clone)]
pub struct OscillationDetector {
    history: VecDeque<ToolExecutionRecord>,
    max_window: usize,
    stuck_threshold: usize,
}

impl Default for OscillationDetector {
    fn default() -> Self {
        Self::new(10, 3)
    }
}

impl OscillationDetector {
    pub fn new(max_window: usize, stuck_threshold: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_window),
            max_window,
            stuck_threshold,
        }
    }

    /// Records a tool execution and checks whether an oscillation or stuck loop has occurred.
    pub fn record_and_evaluate(
        &mut self,
        call: &ToolCall,
        is_error: bool,
        output_snippet: Option<&str>,
    ) -> Option<OscillationAlert> {
        let args_hash = hash_str(&call.function.arguments);
        let error_hash = output_snippet.filter(|_| is_error).map(hash_str);

        let record = ToolExecutionRecord {
            tool_name: call.function.name.clone(),
            args_hash,
            is_error,
            error_hash,
        };

        if self.history.len() >= self.max_window {
            self.history.pop_front();
        }
        self.history.push_back(record);

        self.detect_cycle()
    }

    /// Evaluates current history for cycles, ping-pongs, or repeated failures.
    pub fn detect_cycle(&self) -> Option<OscillationAlert> {
        if self.history.len() < 2 {
            return None;
        }

        // 1. Detect Identical Consecutive Calls (e.g. same tool and arguments 3+ times)
        let last = self.history.back().unwrap();
        let consecutive_same = self
            .history
            .iter()
            .rev()
            .take_while(|r| r.tool_name == last.tool_name && r.args_hash == last.args_hash)
            .count();

        if consecutive_same >= self.stuck_threshold {
            return Some(OscillationAlert {
                kind: OscillationKind::IdenticalCallLoop {
                    tool_name: last.tool_name.clone(),
                    count: consecutive_same,
                },
                description: format!(
                    "Tool '{}' called {} consecutive times with identical parameters without progressing.",
                    last.tool_name, consecutive_same
                ),
                adaptive_steering_prompt: format!(
                    "[SYSTEM ADAPTIVE STEERING]: You have executed `{}` {} times in a row with identical arguments. Stop repeating this action. Re-evaluate assumptions, inspect error logs, or switch to an alternate approach.",
                    last.tool_name, consecutive_same
                ),
            });
        }

        // 2. Detect Ping-Pong Oscillation (e.g., A -> B -> A -> B)
        if self.history.len() >= 4 {
            let n = self.history.len();
            let r0 = &self.history[n - 1];
            let r1 = &self.history[n - 2];
            let r2 = &self.history[n - 3];
            let r3 = &self.history[n - 4];

            if r0.tool_name == r2.tool_name
                && r0.args_hash == r2.args_hash
                && r1.tool_name == r3.tool_name
                && r1.args_hash == r3.args_hash
                && (r0.tool_name != r1.tool_name || r0.args_hash != r1.args_hash)
            {
                return Some(OscillationAlert {
                    kind: OscillationKind::PingPongOscillation {
                        tool_a: r0.tool_name.clone(),
                        tool_b: r1.tool_name.clone(),
                    },
                    description: format!(
                        "Oscillating ping-pong pattern detected between '{}' and '{}'.",
                        r0.tool_name, r1.tool_name
                    ),
                    adaptive_steering_prompt: format!(
                        "[SYSTEM ADAPTIVE STEERING]: Cycle detected! You are oscillating between `{}` and `{}` without reaching a stable state. Step back, formulate a revised plan, and do not repeat these actions.",
                        r0.tool_name, r1.tool_name
                    ),
                });
            }
        }

        // 3. Detect Repeated Failures with Identical Error Output
        if self.history.len() >= 2 {
            let n = self.history.len();
            let curr = &self.history[n - 1];
            let prev = &self.history[n - 2];

            if curr.is_error
                && prev.is_error
                && curr.tool_name == prev.tool_name
                && curr.error_hash.is_some()
                && curr.error_hash == prev.error_hash
            {
                return Some(OscillationAlert {
                    kind: OscillationKind::RepeatedFailureWithoutRevision {
                        tool_name: curr.tool_name.clone(),
                        error_snippet: "<repeated error>".to_string(),
                    },
                    description: format!(
                        "Tool '{}' failed twice with identical error output. Approach needs revision.",
                        curr.tool_name
                    ),
                    adaptive_steering_prompt: format!(
                        "[SYSTEM ADAPTIVE STEERING]: Tool `{}` returned the exact same error twice. Diagnose the underlying root cause instead of retrying the same flawed command.",
                        curr.tool_name
                    ),
                });
            }
        }

        None
    }

    /// Clears the history on major turn transition or explicit recovery.
    pub fn reset(&mut self) {
        self.history.clear();
    }
}

fn hash_str(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_core::types::FunctionCall;

    fn make_call(name: &str, args: &str) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: args.to_string(),
            },
        }
    }

    #[test]
    fn test_detect_identical_consecutive_calls() {
        let mut detector = OscillationDetector::new(10, 3);
        let call = make_call("exec_command", "{\"command\":\"cargo check\"}");

        assert!(detector.record_and_evaluate(&call, false, None).is_none());
        assert!(detector.record_and_evaluate(&call, false, None).is_none());

        let alert = detector.record_and_evaluate(&call, false, None);
        assert!(alert.is_some());
        assert!(matches!(
            alert.unwrap().kind,
            OscillationKind::IdenticalCallLoop { .. }
        ));
    }

    #[test]
    fn test_detect_ping_pong_oscillation() {
        let mut detector = OscillationDetector::new(10, 3);
        let call_a = make_call("edit", "{\"path\":\"foo.rs\"}");
        let call_b = make_call("verify", "{\"mode\":\"check\"}");

        detector.record_and_evaluate(&call_a, false, None);
        detector.record_and_evaluate(&call_b, false, None);
        detector.record_and_evaluate(&call_a, false, None);
        let alert = detector.record_and_evaluate(&call_b, false, None);

        assert!(alert.is_some());
        assert!(matches!(
            alert.unwrap().kind,
            OscillationKind::PingPongOscillation { .. }
        ));
    }

    #[test]
    fn test_detect_repeated_failure() {
        let mut detector = OscillationDetector::new(10, 3);
        let call = make_call("exec_command", "{\"command\":\"broken_script.sh\"}");

        detector.record_and_evaluate(&call, true, Some("error: file not found"));
        let alert = detector.record_and_evaluate(&call, true, Some("error: file not found"));

        assert!(alert.is_some());
        assert!(matches!(
            alert.unwrap().kind,
            OscillationKind::RepeatedFailureWithoutRevision { .. }
        ));
    }
}
