//! Turn execution telemetry, model routing facts, and extension metadata.
//!
//! Captures comprehensive per-turn metrics including latency intervals (inference,
//! tool execution, guardian review), prompt cache hit states, dynamic model routing
//! decisions, and arbitrary custom extensions attached by plugins or agents.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// High-resolution latency intervals for an execution turn in milliseconds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnExecutionTiming {
    pub inference_duration_ms: u64,
    pub tool_execution_duration_ms: u64,
    pub guardian_review_duration_ms: u64,
    pub user_wait_duration_ms: u64,
    pub total_turn_duration_ms: u64,
}

/// Prompt cache status reported by the model provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TurnCacheStatus {
    Hit {
        cached_tokens: u64,
    },
    Miss,
    #[default]
    NotSupported,
}

/// Details of how the model provider was selected for this turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRoutingDecision {
    pub requested_model: String,
    pub active_model: String,
    pub fallback_triggered: bool,
    pub reason: String,
}

impl Default for ModelRoutingDecision {
    fn default() -> Self {
        Self {
            requested_model: "default".to_string(),
            active_model: "default".to_string(),
            fallback_triggered: false,
            reason: "Standard configuration".to_string(),
        }
    }
}

/// Comprehensive turn metadata and diagnostic telemetry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TurnMetadata {
    pub turn_id: String,
    pub turn_index: u32,
    pub timestamp_millis: i64,
    pub routing: ModelRoutingDecision,
    pub cache: TurnCacheStatus,
    pub timing: TurnExecutionTiming,
    pub guardian_risk_score: Option<f64>,
    pub extensions: HashMap<String, serde_json::Value>,
}

impl TurnMetadata {
    pub fn new(turn_id: impl Into<String>, turn_index: u32) -> Self {
        Self {
            turn_id: turn_id.into(),
            turn_index,
            timestamp_millis: Utc::now().timestamp_millis(),
            routing: ModelRoutingDecision::default(),
            cache: TurnCacheStatus::default(),
            timing: TurnExecutionTiming::default(),
            guardian_risk_score: None,
            extensions: HashMap::new(),
        }
    }

    pub fn set_extension(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.extensions.insert(key.into(), value);
    }

    pub fn get_extension<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.extensions
            .get(key)
            .and_then(|val| serde_json::from_value(val.clone()).ok())
    }

    pub fn format_telemetry_line(&self) -> String {
        let cache_str = match &self.cache {
            TurnCacheStatus::Hit { cached_tokens } => format!("cache_hit({} tok)", cached_tokens),
            TurnCacheStatus::Miss => "cache_miss".to_string(),
            TurnCacheStatus::NotSupported => "no_cache".to_string(),
        };

        format!(
            "Turn #{} [{}] model={} timing={}ms (inf: {}ms, tools: {}ms, guard: {}ms) [{}]",
            self.turn_index,
            self.turn_id,
            self.routing.active_model,
            self.timing.total_turn_duration_ms,
            self.timing.inference_duration_ms,
            self.timing.tool_execution_duration_ms,
            self.timing.guardian_review_duration_ms,
            cache_str
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_metadata_formatting_and_extensions() {
        let mut meta = TurnMetadata::new("turn-xyz-101", 3);
        meta.routing = ModelRoutingDecision {
            requested_model: "deepseek/deepseek-r1".to_string(),
            active_model: "meta/llama-3.3-70b-instruct".to_string(),
            fallback_triggered: true,
            reason: "Primary provider rate limited".to_string(),
        };
        meta.cache = TurnCacheStatus::Hit {
            cached_tokens: 4096,
        };
        meta.timing = TurnExecutionTiming {
            inference_duration_ms: 1200,
            tool_execution_duration_ms: 300,
            guardian_review_duration_ms: 50,
            user_wait_duration_ms: 0,
            total_turn_duration_ms: 1550,
        };
        meta.set_extension("git_commit_sha", serde_json::json!("abc1234"));

        let line = meta.format_telemetry_line();
        assert!(line.contains("Turn #3 [turn-xyz-101]"));
        assert!(line.contains("model=meta/llama-3.3-70b-instruct"));
        assert!(line.contains("timing=1550ms"));
        assert!(line.contains("cache_hit(4096 tok)"));

        let sha: Option<String> = meta.get_extension("git_commit_sha");
        assert_eq!(sha.as_deref(), Some("abc1234"));
    }
}
