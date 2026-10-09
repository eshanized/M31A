//! Autonomous evaluation harness, fixture builder, and acceptance scenarios (D-15, TST-01..03, TST-06).

pub mod autonomous;
pub mod fixture;
pub mod runner;
pub mod scenarios;
pub mod scorecard;

pub use autonomous::AutonomousEvalRunner;
pub use fixture::{FixtureRepo, FixtureRepoBuilder};
pub use runner::EvalRunner;
pub use scenarios::{
    EvalScenario, ScenarioA, ScenarioB, ScenarioC, ScenarioD, ScenarioE, ScenarioF, ScenarioG,
    ScenarioH, ScenarioResult, ScenarioStatus,
};
pub use scorecard::{EvalScorecard, EvalSummary};
