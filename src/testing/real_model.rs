//! Real-model integration harness.
//!
//! Establishes the clean separation between deterministic runtime/invariant
//! tests and real-model behavioral tests:
//!
//! ```text
//! DETERMINISTIC TESTS  →  runtime correctness (mocks allowed for failures)
//! REAL NVIDIA TESTS    →  intelligence correctness (no mocks, ever)
//! REAL E2E             →  autonomous engineering correctness
//! ```
//!
//! # Gating contract
//!
//! - Every real-model test resolves the canonical configuration
//!   (`provider = nvidia`, `model = nvidia/nemotron-3-ultra-550b-a55b`,
//!   endpoint `https://integrate.api.nvidia.com/v1`).
//! - Credentials come exclusively from the existing `.env`/configuration
//!   pipeline (`NVIDIA_API_KEY` / `API_KEY_NVIDIA`). They are never printed,
//!   logged, or embedded.
//! - When credentials are unavailable the test SKIPS cleanly. It MUST NOT
//!   substitute a mock/fake model and report success:
//!
//! ```text
//! NO API KEY  →  SKIP REAL-MODEL TEST   (correct)
//! NO API KEY  →  USE MOCK → PASS        (forbidden: fakes success)
//! ```
//!
//! # Production path
//!
//! The harness builds the same production objects as the real runtime:
//! [`NvidiaProvider`] → [`RoutedModelCaller`] (real router, real tool
//! schemas) → [`AppRuntime`] via [`AppRuntime::with_model_provider`], with the
//! real [`PromptCatalog`](crate::prompt::PromptCatalog) and real registries.
//! No test-only shortcuts bypass production configuration.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::agent::model_policy::RoutedModelCaller;
use crate::config::{SafeEnvironmentStatus, load_dotenv};
use crate::model::catalog::{
    CANONICAL_REAL_MODEL_BASE_URL, CANONICAL_REAL_MODEL_ID, CANONICAL_REAL_MODEL_PROVIDER,
};
use crate::model::provider::nvidia::NvidiaProvider;
use crate::model::router::resolver::ModelTier;
use crate::runtime::AppRuntime;

/// Marker env var that opts the dispatcher out of mock-forcing test detection.
///
/// `ProductionWorkerDispatcher` treats test binaries as mock-only environments
/// unless this variable is set; real-model tests must set it so the genuine
/// provider path is constructed.
pub const REAL_MODEL_TEST_ENV: &str = "M31A_REAL_MODEL_TEST";

/// Signal that a real-model test must skip: no valid credentials / network.
///
/// Carries a human-readable reason for the test log. This is a skip, never a
/// failure, and never a license to fall back to a mock.
#[derive(Debug, Clone)]
pub struct RealModelSkipped {
    pub reason: String,
}

impl std::fmt::Display for RealModelSkipped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "real-model test SKIPPED: {}", self.reason)
    }
}

/// One outbound model request observed by the harness tracer.
#[derive(Debug, Clone)]
pub struct TracedModelRequest {
    /// Model identifier the runtime selected for this request.
    pub model_name: String,
    /// Sanitized request payload (credentials are never captured: the tracer
    /// only observes the JSON payload, never headers).
    pub payload: serde_json::Value,
    /// Wall-clock time the request was issued.
    pub issued_at: Instant,
}

/// Shareable live view of traced model requests (for progress pollers).
#[derive(Debug, Clone)]
pub struct TracedHandle {
    traced: Arc<Mutex<Vec<TracedModelRequest>>>,
    started: Instant,
}

impl TracedHandle {
    /// Number of outbound model requests observed so far.
    pub fn call_count(&self) -> usize {
        self.traced.lock().expect("tracer mutex poisoned").len()
    }

    /// Elapsed time since harness construction.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}
///
/// Running measurements for a real-model test.
///
/// Counts model invocations and tool-feedback turns observed through the
/// production path, plus wall-clock latency. Token usage is recorded where
/// the provider surface exposes it (direct provider calls); mission-level
/// tests record calls + latency.
#[derive(Debug, Default)]
pub struct RealModelMetrics {
    pub model_calls: usize,
    pub tool_feedback_turns: usize,
    pub assistant_tool_call_turns: usize,
    pub elapsed: Duration,
}

impl RealModelMetrics {
    /// Summarize measurements in one log line (no credentials involved).
    pub fn summary_line(&self) -> String {
        format!(
            "model_calls={} tool_call_turns={} tool_feedback_turns={} elapsed_ms={}",
            self.model_calls,
            self.assistant_tool_call_turns,
            self.tool_feedback_turns,
            self.elapsed.as_millis()
        )
    }
}

/// Canonical real-model harness: credentials, provider, model, and tracer.
///
/// Construct via [`RealModelHarness::ensure`], which enforces the gating
/// contract (skip without credentials, never mock).
pub struct RealModelHarness {
    provider: Arc<NvidiaProvider>,
    model_id: String,
    traced: Arc<Mutex<Vec<TracedModelRequest>>>,
    started: Instant,
}

impl RealModelHarness {
    /// Enforce the real-model gate and construct the harness.
    ///
    /// 1. Loads `.env` through the existing configuration pipeline.
    /// 2. Forces the canonical model selection (`M31A_MODEL`) and the
    ///    real-model dispatcher path (`M31A_REAL_MODEL_TEST=1`) for this
    ///    process. Both values are constant across the suite, so parallel
    ///    tests within one binary cannot race on distinct values.
    /// 3. Constructs the real [`NvidiaProvider`] from environment credentials.
    ///    Missing/invalid credentials → `Err(RealModelSkipped)`.
    ///
    /// Never constructs a mock provider.
    pub fn ensure() -> Result<Self, RealModelSkipped> {
        load_dotenv();

        // SAFETY: Test-harness startup only; values are suite-wide constants.
        unsafe {
            std::env::set_var(REAL_MODEL_TEST_ENV, "1");
            std::env::set_var("M31A_MODEL", CANONICAL_REAL_MODEL_ID);
        }

        let status = SafeEnvironmentStatus::probe();
        if !status.api_key_configured {
            return Err(RealModelSkipped {
                reason: "no NVIDIA credentials: set NVIDIA_API_KEY (or API_KEY_NVIDIA) in .env"
                    .to_string(),
            });
        }

        let traced: Arc<Mutex<Vec<TracedModelRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = traced.clone();
        let provider = NvidiaProvider::new(None, None)
            .map_err(|e| RealModelSkipped {
                reason: format!(
                    "NVIDIA provider initialization failed ({e}); check credentials/network"
                ),
            })?
            .with_request_tracer(Arc::new(move |model_name, payload| {
                let mut guard = sink.lock().expect("tracer mutex poisoned");
                guard.push(TracedModelRequest {
                    model_name: model_name.to_string(),
                    payload: payload.clone(),
                    issued_at: Instant::now(),
                });
            }));

        Ok(Self {
            provider: Arc::new(provider),
            model_id: CANONICAL_REAL_MODEL_ID.to_string(),
            traced,
            started: Instant::now(),
        })
    }

    /// Canonical provider identifier (`nvidia`).
    pub fn provider_name(&self) -> &'static str {
        CANONICAL_REAL_MODEL_PROVIDER
    }

    /// Canonical model identifier under test.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Canonical endpoint base URL.
    pub fn base_url(&self) -> &'static str {
        CANONICAL_REAL_MODEL_BASE_URL
    }

    /// The real provider (production path, tracer attached).
    pub fn provider(&self) -> Arc<NvidiaProvider> {
        self.provider.clone()
    }

    /// All outbound model requests observed so far (sanitized payloads).
    pub fn traced_requests(&self) -> Vec<TracedModelRequest> {
        self.traced.lock().expect("tracer mutex poisoned").clone()
    }

    /// Shareable live view of the tracer for progress pollers.
    pub fn tracer_handle(&self) -> TracedHandle {
        TracedHandle {
            traced: self.traced.clone(),
            started: self.started,
        }
    }

    /// Assert every observed request targeted the canonical model.
    ///
    /// This is the authoritative proof that production model resolution
    /// selected `nvidia/nemotron-3-ultra-550b-a55b`.
    pub fn assert_canonical_model_routing(&self) {
        let traced = self.traced_requests();
        assert!(
            !traced.is_empty(),
            "real model must have been invoked at least once (no requests traced)"
        );
        for req in &traced {
            assert_eq!(
                req.model_name, self.model_id,
                "production routing must select the canonical model"
            );
        }
    }

    /// Build the real routed caller: genuine provider + real router +
    /// deterministic selection of the canonical model + real tool schemas.
    pub fn routed_caller(&self, tools: Vec<serde_json::Value>) -> RoutedModelCaller {
        RoutedModelCaller::new(
            Some(self.provider.clone() as Arc<dyn crate::model::provider::ModelProvider>),
            ModelTier::Standard,
            tools,
        )
        .with_model(self.model_id.clone())
        .with_provider_status(
            "nvidia_nim".to_string(),
            crate::model::types::ProviderCapabilityStatus::Available,
        )
    }

    /// Build a production [`AppRuntime`] for `workspace_root` wired to the
    /// real NVIDIA provider through the standard
    /// [`AppRuntime::with_model_provider`] path (same provider, model
    /// resolver, PromptCatalog, policy, and context compiler as production).
    pub async fn runtime_for(
        &self,
        workspace_root: &Path,
    ) -> Result<AppRuntime, crate::error::M31AError> {
        let runtime = AppRuntime::new(workspace_root).await?;
        Ok(runtime.with_model_provider(self.provider.clone()))
    }

    /// Compute aggregate metrics from traced requests (§15).
    pub fn metrics(&self) -> RealModelMetrics {
        let traced = self.traced_requests();
        let mut m = RealModelMetrics {
            model_calls: traced.len(),
            elapsed: self.started.elapsed(),
            ..Default::default()
        };
        for req in &traced {
            if let Some(messages) = req.payload.get("messages").and_then(|v| v.as_array()) {
                let mut saw_tool_feedback = false;
                let mut saw_assistant_tool_calls = false;
                for msg in messages {
                    match msg.get("role").and_then(|r| r.as_str()) {
                        Some("tool") => saw_tool_feedback = true,
                        Some("assistant")
                            if msg
                                .get("tool_calls")
                                .and_then(|t| t.as_array())
                                .is_some_and(|a| !a.is_empty()) =>
                        {
                            saw_assistant_tool_calls = true;
                        }
                        _ => {}
                    }
                }
                if saw_tool_feedback {
                    m.tool_feedback_turns += 1;
                }
                if saw_assistant_tool_calls {
                    m.assistant_tool_call_turns += 1;
                }
            }
        }
        m
    }

    /// Assert the traced payloads carried production context fields (§12).
    ///
    /// `needles` are substrings expected to appear in at least one outbound
    /// payload (e.g. intent text, requirement keys, decision titles). This
    /// proves the model actually received them through the production path —
    /// a unit test on the context compiler alone cannot prove that.
    pub fn assert_context_propagated(&self, needles: &[&str]) {
        let traced = self.traced_requests();
        assert!(
            !traced.is_empty(),
            "cannot prove context propagation without model requests"
        );
        let corpus: String = traced
            .iter()
            .map(|r| r.payload.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        for needle in needles {
            assert!(
                corpus.contains(needle),
                "production context must reach the model; missing: {needle}"
            );
        }
    }

    /// Log a metrics line for telemetry and audit (stdout only, no secrets).
    pub fn log_metrics(&self, label: &str) {
        let m = self.metrics();
        println!(
            "[REAL-MODEL:{label}] provider={} model={} {}",
            self.provider_name(),
            self.model_id(),
            m.summary_line()
        );
    }
}
