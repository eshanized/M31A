//! autonomous evaluation harness driving real runtime, tool pipeline, and model callers.
//!
//! separates acceptance scenarios (a-h) from true autonomous evaluation where:
//! - a pristine fixture is created
//! - actual runtime authorities execute model-proposed actions
//! - changes traverse the 11-stage tool pipeline
//! - verification and evidence gates run
//! - metrics (tokens, cost, files modified, verification) reflect true runtime evidence.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::agent::engine::AgentEngineState;
use crate::agent::model_policy::{ModelCaller, ModelProposal, ModelToolCall, TestModelCaller};
use crate::eval::fixture::FixtureRepoBuilder;
use crate::eval::scenarios::{ScenarioResult, ScenarioStatus};
use crate::eval::scorecard::EvalScorecard;
use crate::model::types::{CostProvenance, TokenUsage, UsageSource};
use crate::runtime::AppRuntime;

/// true autonomous evaluation runner executing full-stack runtime workflows.
pub struct AutonomousEvalRunner {
    live_nvidia_opt_in: bool,
    scenario_timeout: Duration,
}

impl Default for AutonomousEvalRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl AutonomousEvalRunner {
    /// create a new autonomous evaluation runner.
    pub fn new() -> Self {
        let live_nvidia_opt_in = std::env::var("M31A_EVAL_LIVE_NVIDIA")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self {
            live_nvidia_opt_in,
            scenario_timeout: Duration::from_secs(120),
        }
    }

    /// override live nvidia opt-in flag.
    pub fn with_live_nvidia(mut self, opt_in: bool) -> Self {
        self.live_nvidia_opt_in = opt_in;
        self
    }

    /// check whether live nvidia provider opt-in is enabled.
    pub fn live_nvidia_opt_in(&self) -> bool {
        self.live_nvidia_opt_in
    }

    /// override per-scenario execution timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.scenario_timeout = timeout;
        self
    }

    /// execute the autonomous bug-fix evaluation scenario.
    pub async fn run_autonomous_bug_fix(&self) -> Result<ScenarioResult, String> {
        let start = Instant::now();

        // 1. set up pristine fixture repository with failing calculation
        let mut builder = FixtureRepoBuilder::new().map_err(|e| e.to_string())?;
        let cargo_toml = r#"[package]
name = "fixture_calc"
version = "0.1.0"
edition = "2021"

[lib]
path = "src/calc.rs"
"#;
        let buggy_code = "// calculator module\npub fn add(a: i32, b: i32) -> i32 {\n    a + b + 1 // bug\n}\n\n#[test]\nfn test_add() {\n    assert_eq!(add(2, 2), 4);\n}\n";
        let fixed_code = "// calculator module\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[test]\nfn test_add() {\n    assert_eq!(add(2, 2), 4);\n}\n";

        builder
            .with_file("Cargo.toml", cargo_toml)
            .with_file("src/calc.rs", buggy_code)
            .with_commit("feat: initial calculator implementation");

        let fixture = builder.build().map_err(|e| e.to_string())?;

        // 2. initialize actual canonical runtime on pristine fixture
        let runtime = Arc::new(
            AppRuntime::new(fixture.path())
                .await
                .map_err(|e| format!("runtime initialization failed: {e}"))?,
        );

        // 3. construct model caller: live nvidia if explicitly opted-in, or scripted test caller
        let caller: Arc<dyn ModelCaller> = if self.live_nvidia_opt_in {
            let creds = crate::runtime_authorities::resolve_runtime_credentials(
                runtime.workspace_root(),
                crate::deployment::DeploymentChannel::current(),
            );
            if creds.api_key.is_none() {
                return Err(
                    "M31A_EVAL_LIVE_NVIDIA is enabled but no valid NVIDIA credentials were found via canonical resolution (checked global file, channel file, NVIDIA_API_KEY, and API_KEY_NVIDIA)".to_string(),
                );
            }
            runtime
                .model_caller()
                .ok_or_else(|| "no model caller configured on runtime".to_string())?
        } else {
            let proposals = vec![
                // turn 1: model inspects the buggy file
                Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall {
                        id: "call_read_1".to_string(),
                        name: "read_file".to_string(),
                        arguments: serde_json::json!({ "path": "src/calc.rs" }),
                    }],
                }),
                // turn 2: model applies the fix via governed write_file tool
                Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall {
                        id: "call_write_1".to_string(),
                        name: "write_file".to_string(),
                        arguments: serde_json::json!({
                            "path": "src/calc.rs",
                            "content": fixed_code,
                        }),
                    }],
                }),
                // turn 3: model verifies the fix using run_tests (satisfying EvidenceCompletionGate)
                Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall {
                        id: "call_test_1".to_string(),
                        name: "run_tests".to_string(),
                        arguments: serde_json::json!({}),
                    }],
                }),
                // turn 4: model completes the task with verification evidence
                Ok(ModelProposal::Complete {
                    summary: "fixed calculation bug in src/calc.rs so tests pass".to_string(),
                    artifacts: Vec::new(),
                }),
            ];
            // deterministic test double with explicit estimated per-call token usage
            Arc::new(
                TestModelCaller::from_proposals(proposals)
                    .with_usage(TokenUsage::new(80, 45, 125, 0, UsageSource::Estimated))
                    .with_provider_and_model("nvidia", "meta/llama-3.1-8b-instruct"),
            )
        };

        // 4. create session and canonical agent engine with cooperative cancellation token
        let session = runtime
            .session_repo()
            .create_session(fixture.path())
            .await
            .map_err(|e| format!("failed to create session: {e}"))?;

        let cancel_token = CancellationToken::new();
        let mut engine = runtime
            .create_agent_engine(session.id)
            .with_model_caller(caller)
            .with_cancellation_token(cancel_token.clone());

        // 5. execute continuous autonomous loop bounded by scenario timeout
        let run_fut = engine.run_continuous_with_steering(
            Some("Fix the calculation bug in src/calc.rs so tests pass"),
            |_| Ok(()),
        );

        let outcome = match tokio::time::timeout(self.scenario_timeout, run_fut).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(e)) => return Err(format!("engine execution failed: {e}")),
            Err(_) => {
                cancel_token.cancel();
                let duration_ms = start.elapsed().as_millis() as u64;
                return Ok(ScenarioResult {
                    scenario_id: "auto-bug-fix".to_string(),
                    name: "Autonomous Scenario: Real Bug Fix Workflow".to_string(),
                    status: ScenarioStatus::TimedOut,
                    duration_ms,
                    tokens_used: 0,
                    cost_usd: None,
                    cost_provenance: CostProvenance::Unknown,
                    usage_source: UsageSource::Estimated,
                    verification_passed: false,
                    replans_count: 0,
                    retries_count: 0,
                    files_modified: 0,
                    details: format!("scenario timed out after {:?}", self.scenario_timeout),
                });
            }
        };

        // 6. observe durable results
        let completed = matches!(outcome, AgentEngineState::Completed { .. });

        // verify actual file mutation through the tool pipeline
        let updated_calc = fixture
            .read_file("src/calc.rs")
            .map_err(|e| format!("failed to read calc.rs: {e}"))?;
        let code_fixed = updated_calc.contains("a + b") && !updated_calc.contains("a + b + 1");

        // verify git diff shows exactly 1 file modified
        let diff_out = fixture
            .run_git(&["diff", "--name-only"])
            .map_err(|e| e.to_string())?;
        let diff_str = String::from_utf8_lossy(&diff_out.stdout);
        let files_modified = diff_str.lines().count();

        // verify tests in fixture actually pass
        let test_out = std::process::Command::new("cargo")
            .arg("test")
            .current_dir(fixture.path())
            .output();
        let cargo_test_passed = test_out.map(|o| o.status.success()).unwrap_or(false);

        let verification_passed =
            completed && code_fixed && files_modified == 1 && cargo_test_passed;
        let duration_ms = start.elapsed().as_millis() as u64;

        // 7. aggregate actual recorded invocations for this mission from persistent repository
        let mission_id = engine.active_mission_id().unwrap_or_default();
        let inv_repo = crate::model::persistence::invocation::SqliteModelInvocationRepository::new(
            runtime.pool().clone(),
        );
        let invocations = inv_repo
            .get_invocations_for_mission(&mission_id)
            .await
            .unwrap_or_default();

        let total_prompt_tokens: usize = invocations.iter().map(|i| i.prompt_tokens).sum();
        let total_completion_tokens: usize = invocations.iter().map(|i| i.completion_tokens).sum();
        let total_tokens: u64 = invocations.iter().map(|i| i.total_tokens as u64).sum();

        let (cost_usd, cost_provenance) = if invocations.is_empty()
            || invocations
                .iter()
                .any(|i| i.cost_usd.is_none() || i.cost_provenance == CostProvenance::Unknown)
        {
            (None, CostProvenance::Unknown)
        } else {
            let sum_cost: f64 = invocations.iter().map(|i| i.cost_usd.unwrap_or(0.0)).sum();
            let provenance = if invocations
                .iter()
                .any(|i| i.cost_provenance == CostProvenance::Estimated)
            {
                CostProvenance::Estimated
            } else {
                CostProvenance::Authoritative
            };
            (Some(sum_cost), provenance)
        };

        let usage_source = if invocations.is_empty()
            || invocations.iter().any(|i| i.usage_source == "estimated")
        {
            UsageSource::Estimated
        } else {
            UsageSource::AuthoritativeProvider
        };

        let provider_model_summary = if let Some(first) = invocations.first() {
            format!("{}/{}", first.provider, first.model_name)
        } else {
            "unrecorded".to_string()
        };

        Ok(ScenarioResult {
            scenario_id: "auto-bug-fix".to_string(),
            name: "Autonomous Scenario: Real Bug Fix Workflow".to_string(),
            status: if verification_passed {
                ScenarioStatus::Passed
            } else {
                ScenarioStatus::Failed
            },
            duration_ms,
            tokens_used: total_tokens,
            cost_usd,
            cost_provenance,
            usage_source,
            verification_passed,
            replans_count: 0,
            retries_count: 0,
            files_modified,
            details: format!(
                "outcome: {:?}, code_fixed: {}, files_modified: {}, cargo_tests: {}, invocations: {}, tokens: {} (prompt: {}, completion: {}), cost: {:?}, provenance: {:?}, model: {}",
                outcome,
                code_fixed,
                files_modified,
                cargo_test_passed,
                invocations.len(),
                total_tokens,
                total_prompt_tokens,
                total_completion_tokens,
                cost_usd,
                cost_provenance,
                provider_model_summary
            ),
        })
    }

    /// run all registered autonomous evaluation scenarios and aggregate into an EvalScorecard.
    pub async fn run_all(&self) -> EvalScorecard {
        let mut results = Vec::new();
        match self.run_autonomous_bug_fix().await {
            Ok(res) => results.push(res),
            Err(err) => {
                let status = if err.contains("timed out") {
                    ScenarioStatus::TimedOut
                } else {
                    ScenarioStatus::HarnessError
                };
                results.push(ScenarioResult {
                    scenario_id: "auto-bug-fix".to_string(),
                    name: "Autonomous Scenario: Real Bug Fix Workflow".to_string(),
                    status,
                    duration_ms: if status == ScenarioStatus::TimedOut {
                        self.scenario_timeout.as_millis() as u64
                    } else {
                        0
                    },
                    tokens_used: 0,
                    cost_usd: None,
                    cost_provenance: CostProvenance::Unknown,
                    usage_source: UsageSource::Estimated,
                    verification_passed: false,
                    replans_count: 0,
                    retries_count: 0,
                    files_modified: 0,
                    details: format!("harness error: {}", err),
                });
            }
        }
        EvalScorecard::from_results(results)
    }
}
