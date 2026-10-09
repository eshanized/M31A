//! Autonomous Evaluation Runner Engine (D-15, TST-01..03, TST-06).
//!
//! Orchestrates the execution of canonical acceptance scenarios A through H with:
//! - Strict isolation per run in temporary Git fixtures.
//! - Configurable per-scenario timeouts (default 120s).
//! - Deterministic aggregation into `EvalScorecard`.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::eval::scenarios::{
    EvalScenario, ScenarioA, ScenarioB, ScenarioC, ScenarioD, ScenarioE, ScenarioF, ScenarioG,
    ScenarioH, ScenarioResult, ScenarioStatus,
};
use crate::eval::scorecard::EvalScorecard;

/// Autonomous evaluation runner executing scenarios and aggregating scorecards.
pub struct EvalRunner {
    scenarios: Vec<Arc<dyn EvalScenario>>,
    scenario_timeout: Duration,
}

impl Default for EvalRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl EvalRunner {
    /// Create a new evaluation runner pre-populated with Scenarios A through H.
    pub fn new() -> Self {
        Self {
            scenarios: vec![
                Arc::new(ScenarioA),
                Arc::new(ScenarioB),
                Arc::new(ScenarioC),
                Arc::new(ScenarioD),
                Arc::new(ScenarioE),
                Arc::new(ScenarioF),
                Arc::new(ScenarioG),
                Arc::new(ScenarioH),
            ],
            scenario_timeout: Duration::from_secs(120),
        }
    }

    /// Override the per-scenario execution timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.scenario_timeout = timeout;
        self
    }

    /// Access the registered scenarios.
    pub fn scenarios(&self) -> &[Arc<dyn EvalScenario>] {
        &self.scenarios
    }

    /// Execute a single scenario by ID or name substring.
    pub async fn run_scenario(&self, id_or_name: &str) -> Result<ScenarioResult, String> {
        let id_clean = id_or_name.trim().to_lowercase();
        // 1. Check exact ID match first
        if let Some(s) = self
            .scenarios
            .iter()
            .find(|s| s.id().eq_ignore_ascii_case(&id_clean))
        {
            return self.execute_scenario(s.as_ref()).await;
        }

        // 2. Fallback to name substring match
        let scenario = self
            .scenarios
            .iter()
            .find(|s| s.name().to_lowercase().contains(&id_clean))
            .ok_or_else(|| format!("Unknown scenario: {}", id_or_name))?;

        self.execute_scenario(scenario.as_ref()).await
    }

    /// Execute all registered acceptance scenarios and aggregate into an `EvalScorecard`.
    pub async fn run_all(&self) -> EvalScorecard {
        let mut results = Vec::new();
        for scenario in &self.scenarios {
            let res = match self.execute_scenario(scenario.as_ref()).await {
                Ok(r) => r,
                Err(err) => {
                    let status = if err.contains("timed out") {
                        ScenarioStatus::TimedOut
                    } else {
                        ScenarioStatus::HarnessError
                    };
                    ScenarioResult {
                        scenario_id: scenario.id().to_string(),
                        name: scenario.name().to_string(),
                        status,
                        duration_ms: if status == ScenarioStatus::TimedOut {
                            self.scenario_timeout.as_millis() as u64
                        } else {
                            0
                        },
                        tokens_used: 0,
                        cost_usd: None,
                        cost_provenance: crate::model::types::CostProvenance::Unknown,
                        usage_source: crate::model::types::UsageSource::Estimated,
                        verification_passed: false,
                        replans_count: 0,
                        retries_count: 0,
                        files_modified: 0,
                        details: format!("Harness error: {}", err),
                    }
                }
            };
            results.push(res);
        }

        EvalScorecard::from_results(results)
    }

    async fn execute_scenario(
        &self,
        scenario: &dyn EvalScenario,
    ) -> Result<ScenarioResult, String> {
        match timeout(self.scenario_timeout, scenario.run()).await {
            Ok(Ok(res)) => Ok(res),
            Ok(Err(err)) => Err(format!("Scenario {} error: {}", scenario.id(), err)),
            Err(_) => Err(format!(
                "Scenario {} timed out after {:?}",
                scenario.id(),
                self.scenario_timeout
            )),
        }
    }
}
