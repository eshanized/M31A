//! Sovereign structured guardian audit trail, telemetry, and forensic reporting.
//!
//! Maintains an in-memory bounded ring buffer of all Guardian decisions, risk evaluations,
//! prompt injection flags, exfiltration blocks, and circuit breaker trips. Supports
//! querying by session, counting denial rates, generating JSONL streams, and summaries.

use crate::assessment::GuardianRiskLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::RwLock;
use uuid::Uuid;

/// Evaluator component that produced the Guardian decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionSource {
    /// Fast static zero-latency rule heuristic.
    LocalHeuristic,
    /// Multi-turn threat scoring and risk momentum engine.
    AsyncScorer,
    /// Prompt injection and token pattern detector.
    PromptInjectionDetector,
    /// Outbound credential and secret exfiltration guard.
    DataExfiltrationGuard,
    /// Downstream LLM reviewer (e.g. Llama-3.3-70B or sovereign reviewer).
    LlmReviewer,
    /// Consecutive denial or cyber model circuit breaker.
    CircuitBreaker,
    /// Direct human interactive approval or override.
    ManualUserOverride,
}

/// Final outcome of a guardian review evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianDecisionOutcome {
    Allowed,
    Denied,
    Sanitized,
    TrippedCircuitBreaker,
    PendingUserApproval,
}

/// A structured, auditable record for every guardian safety evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GuardianAuditRecord {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub turn_index: usize,
    pub source: DecisionSource,
    pub outcome: GuardianDecisionOutcome,
    pub risk_level: GuardianRiskLevel,
    pub action_tool: String,
    pub action_summary: String,
    pub rationale: String,
    pub latency_us: u64,
}

impl GuardianAuditRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: impl Into<String>,
        turn_index: usize,
        source: DecisionSource,
        outcome: GuardianDecisionOutcome,
        risk_level: GuardianRiskLevel,
        action_tool: impl Into<String>,
        action_summary: impl Into<String>,
        rationale: impl Into<String>,
        latency_us: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            session_id: session_id.into(),
            turn_index,
            source,
            outcome,
            risk_level,
            action_tool: action_tool.into(),
            action_summary: action_summary.into(),
            rationale: rationale.into(),
            latency_us,
        }
    }
}

/// Aggregate statistical summary of guardian audit events.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditTrailSummary {
    pub total_decisions: usize,
    pub allowed_count: usize,
    pub denied_count: usize,
    pub sanitized_count: usize,
    pub tripped_breaker_count: usize,
    pub critical_risk_count: usize,
}

/// Thread-safe in-memory ring buffer for Guardian decision records.
pub struct GuardianAuditTrail {
    capacity: usize,
    buffer: RwLock<VecDeque<GuardianAuditRecord>>,
}

impl Default for GuardianAuditTrail {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl GuardianAuditTrail {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            buffer: RwLock::new(VecDeque::with_capacity(capacity)),
        }
    }

    /// Record a decision event into the audit trail.
    pub fn record(&self, record: GuardianAuditRecord) {
        let mut buf = self.buffer.write().unwrap();
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(record);
    }

    /// Retrieve the most recent N records.
    pub fn recent_records(&self, limit: usize) -> Vec<GuardianAuditRecord> {
        let buf = self.buffer.read().unwrap();
        buf.iter().rev().take(limit).cloned().collect()
    }

    /// Filter records belonging to a specific session ID.
    pub fn filter_by_session(&self, session_id: &str) -> Vec<GuardianAuditRecord> {
        let buf = self.buffer.read().unwrap();
        buf.iter()
            .filter(|r| r.session_id == session_id)
            .cloned()
            .collect()
    }

    /// Count denied actions for a specific session.
    pub fn count_denials(&self, session_id: &str) -> usize {
        let buf = self.buffer.read().unwrap();
        buf.iter()
            .filter(|r| r.session_id == session_id && r.outcome == GuardianDecisionOutcome::Denied)
            .count()
    }

    /// Compute summary statistics across all buffered audit records.
    pub fn summary(&self) -> AuditTrailSummary {
        let buf = self.buffer.read().unwrap();
        let mut summary = AuditTrailSummary {
            total_decisions: buf.len(),
            ..Default::default()
        };

        for r in buf.iter() {
            match r.outcome {
                GuardianDecisionOutcome::Allowed => summary.allowed_count += 1,
                GuardianDecisionOutcome::Denied => summary.denied_count += 1,
                GuardianDecisionOutcome::Sanitized => summary.sanitized_count += 1,
                GuardianDecisionOutcome::TrippedCircuitBreaker => {
                    summary.tripped_breaker_count += 1
                }
                GuardianDecisionOutcome::PendingUserApproval => {}
            }
            if r.risk_level == GuardianRiskLevel::Critical {
                summary.critical_risk_count += 1;
            }
        }

        summary
    }

    /// Export buffered records as a newline-delimited JSON (JSONL) string.
    pub fn export_jsonl(&self) -> Result<String, serde_json::Error> {
        let buf = self.buffer.read().unwrap();
        let mut out = String::new();
        for r in buf.iter() {
            let line = serde_json::to_string(r)?;
            out.push_str(&line);
            out.push('\n');
        }
        Ok(out)
    }

    /// Clear all recorded entries.
    pub fn clear(&self) {
        let mut buf = self.buffer.write().unwrap();
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_query_recent() {
        let trail = GuardianAuditTrail::new(10);
        let rec = GuardianAuditRecord::new(
            "sess_1",
            1,
            DecisionSource::LocalHeuristic,
            GuardianDecisionOutcome::Allowed,
            GuardianRiskLevel::Low,
            "read_file",
            "read_file: Cargo.toml",
            "Safe read",
            120,
        );

        trail.record(rec.clone());
        let recent = trail.recent_records(5);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].session_id, "sess_1");
        assert_eq!(recent[0].outcome, GuardianDecisionOutcome::Allowed);
    }

    #[test]
    fn test_capacity_ring_eviction() {
        let trail = GuardianAuditTrail::new(3);
        for i in 1..=5 {
            let rec = GuardianAuditRecord::new(
                "sess_overflow",
                i,
                DecisionSource::PromptInjectionDetector,
                GuardianDecisionOutcome::Denied,
                GuardianRiskLevel::High,
                "command",
                format!("turn {i}"),
                "reason",
                50,
            );
            trail.record(rec);
        }

        let recent = trail.recent_records(10);
        assert_eq!(recent.len(), 3);
        // Most recent should be turn 5, then 4, then 3
        assert_eq!(recent[0].turn_index, 5);
        assert_eq!(recent[1].turn_index, 4);
        assert_eq!(recent[2].turn_index, 3);
    }

    #[test]
    fn test_filter_and_count_denials() {
        let trail = GuardianAuditTrail::new(20);

        trail.record(GuardianAuditRecord::new(
            "sess_a",
            1,
            DecisionSource::LocalHeuristic,
            GuardianDecisionOutcome::Allowed,
            GuardianRiskLevel::Low,
            "read",
            "read",
            "ok",
            10,
        ));
        trail.record(GuardianAuditRecord::new(
            "sess_a",
            2,
            DecisionSource::DataExfiltrationGuard,
            GuardianDecisionOutcome::Denied,
            GuardianRiskLevel::Critical,
            "curl",
            "exfil",
            "blocked",
            15,
        ));
        trail.record(GuardianAuditRecord::new(
            "sess_b",
            1,
            DecisionSource::DataExfiltrationGuard,
            GuardianDecisionOutcome::Denied,
            GuardianRiskLevel::Critical,
            "curl",
            "exfil",
            "blocked",
            15,
        ));

        assert_eq!(trail.count_denials("sess_a"), 1);
        assert_eq!(trail.count_denials("sess_b"), 1);
        assert_eq!(trail.count_denials("sess_c"), 0);

        let filtered = trail.filter_by_session("sess_a");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_summary_and_jsonl() {
        let trail = GuardianAuditTrail::new(10);

        trail.record(GuardianAuditRecord::new(
            "s1",
            1,
            DecisionSource::LocalHeuristic,
            GuardianDecisionOutcome::Allowed,
            GuardianRiskLevel::Low,
            "tool",
            "action",
            "ok",
            10,
        ));
        trail.record(GuardianAuditRecord::new(
            "s1",
            2,
            DecisionSource::CircuitBreaker,
            GuardianDecisionOutcome::TrippedCircuitBreaker,
            GuardianRiskLevel::Critical,
            "tool",
            "action",
            "tripped",
            10,
        ));

        let summary = trail.summary();
        assert_eq!(summary.total_decisions, 2);
        assert_eq!(summary.allowed_count, 1);
        assert_eq!(summary.tripped_breaker_count, 1);
        assert_eq!(summary.critical_risk_count, 1);

        let jsonl = trail.export_jsonl().expect("jsonl serialized");
        assert!(jsonl.contains("\"session_id\":\"s1\""));
        assert_eq!(jsonl.lines().count(), 2);
    }
}
