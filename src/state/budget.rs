//! Resource budgets for bounding autonomous execution across 10 dimensions (BST-01, D-05).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Resource budget bounding mission execution across 10 canonical dimensions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudget {
    /// 1. Maximum wall-clock execution time in seconds.
    pub max_wall_clock_seconds: Option<u64>,
    /// 2. Maximum number of concurrently executing agents.
    pub max_concurrent_agents: Option<usize>,
    /// 3. Maximum execution steps per agent.
    pub max_agent_steps: Option<usize>,
    /// 4. Maximum total model calls across the mission.
    pub max_model_calls: Option<usize>,
    /// 5. Maximum total tokens consumed.
    pub max_tokens: Option<u64>,
    /// 6. Maximum estimated financial cost in USD.
    pub max_cost_usd: Option<f64>,
    /// 7. Maximum CPU seconds consumed by child processes.
    pub max_cpu_seconds: Option<u64>,
    /// 8. Maximum memory bytes allocated across child processes.
    pub max_memory_bytes: Option<u64>,
    /// 9. Maximum cumulative artifact storage bytes across the mission.
    pub max_artifact_bytes: Option<u64>,
    /// 10. Maximum retry attempts for failed tasks.
    pub max_retries: Option<usize>,
}

impl ResourceBudget {
    /// Create an empty/unbounded budget with all limits optional.
    pub fn unbounded() -> Self {
        Self {
            max_wall_clock_seconds: None,
            max_concurrent_agents: None,
            max_agent_steps: None,
            max_model_calls: None,
            max_tokens: None,
            max_cost_usd: None,
            max_cpu_seconds: None,
            max_memory_bytes: None,
            max_artifact_bytes: None,
            max_retries: None,
        }
    }
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self::unbounded()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_budget_defaults() {
        let budget = ResourceBudget::default();
        assert!(budget.max_wall_clock_seconds.is_none());
        assert!(budget.max_cost_usd.is_none());
        assert!(budget.max_cpu_seconds.is_none());
        assert!(budget.max_memory_bytes.is_none());
        assert!(budget.max_artifact_bytes.is_none());
    }

    #[test]
    fn test_resource_budget_serde_roundtrip() {
        let budget = ResourceBudget {
            max_wall_clock_seconds: Some(3600),
            max_concurrent_agents: Some(4),
            max_agent_steps: Some(50),
            max_model_calls: Some(100),
            max_tokens: Some(1_000_000),
            max_cost_usd: Some(25.50),
            max_cpu_seconds: Some(600),
            max_memory_bytes: Some(4 * 1024 * 1024 * 1024),
            max_artifact_bytes: Some(500 * 1024 * 1024),
            max_retries: Some(3),
        };
        let json = serde_json::to_string(&budget).unwrap();
        let parsed: ResourceBudget = serde_json::from_str(&json).unwrap();
        assert_eq!(budget, parsed);

        let toml_str = toml::to_string(&budget).unwrap();
        let toml_parsed: ResourceBudget = toml::from_str(&toml_str).unwrap();
        assert_eq!(budget, toml_parsed);
    }
}
