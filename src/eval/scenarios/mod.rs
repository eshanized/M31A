//! Acceptance Scenarios A through H implementation (CONTEXT_M31A.md §99, TST-01..03, D-15).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod scenario_a;
pub mod scenario_b;
pub mod scenario_c;
pub mod scenario_d;
pub mod scenario_e;
pub mod scenario_f;
pub mod scenario_g;
pub mod scenario_h;

pub use scenario_a::ScenarioA;
pub use scenario_b::ScenarioB;
pub use scenario_c::ScenarioC;
pub use scenario_d::ScenarioD;
pub use scenario_e::ScenarioE;
pub use scenario_f::ScenarioF;
pub use scenario_g::ScenarioG;
pub use scenario_h::ScenarioH;

/// Execution outcome status of an acceptance scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioStatus {
    Passed,
    Failed,
    PolicyBlocked,
    HarnessError,
    TimedOut,
}

impl ScenarioStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::PolicyBlocked => "policy_blocked",
            Self::HarnessError => "harness_error",
            Self::TimedOut => "timed_out",
        }
    }
}

/// Comprehensive outcome metrics for a single scenario run (TST-06).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioResult {
    pub scenario_id: String,
    pub name: String,
    pub status: ScenarioStatus,
    pub duration_ms: u64,
    pub tokens_used: u64,
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub cost_provenance: crate::model::types::CostProvenance,
    #[serde(default)]
    pub usage_source: crate::model::types::UsageSource,
    pub verification_passed: bool,
    pub replans_count: usize,
    pub retries_count: usize,
    pub files_modified: usize,
    pub details: String,
}

/// Canonical trait implemented by all acceptance evaluation scenarios (D-15).
#[async_trait]
pub trait EvalScenario: Send + Sync {
    /// Scenario identifier (e.g. "a", "b", "c").
    fn id(&self) -> &'static str;

    /// Human-readable scenario name.
    fn name(&self) -> &'static str;

    /// Scenario objective and description.
    fn description(&self) -> &'static str;

    /// Execute the complete scenario in an isolated temporary environment.
    async fn run(&self) -> Result<ScenarioResult, String>;
}
