//! Authoritative canonical live-model test inventory for M31A.
//!
//! Single source of truth for all real-provider integration tests, failure-path
//! probes, and release rehearsals. Used by:
//! 1. `tests/architecture_live_test_partition.rs` to enforce exhaustive coverage and anti-bypass invariants.
//! 2. The canonical live runner (`make test-live-all` / `scripts/run_live_tests.sh`) to execute mandatory gates.
//! 3. Telemetry and release reporting to prove that positive live acceptance tests observed genuine model inference.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Classification of a test in the live-verification taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiveTestCategory {
    /// 1. Positive live inference acceptance: proves a real model invocation and successful response
    /// through the genuine provider/router pipeline.
    PositiveInference,
    /// 2. Live failure-path probe: tests honest provider failure classification (invalid authentication,
    /// unknown model, pre-cancelled token, unreachable endpoint). Deliberately uses bad parameters.
    FailureProbe,
    /// live provider configuration/discovery verification that does not perform inference.
    ProviderVerification,
    /// live cancellation/continuation acceptance: multiple requests are dispatched but the final
    /// request may be cancelled before completion; at least one real inference must complete.
    LiveCancellationProbe,
    /// 3. Offline deterministic test: must run without credentials and never depend on external WAN.
    OfflineDeterministic,
    /// 4. Environment-dependent release rehearsal: tests assembled build artifacts (requires M31A_RC_DIR).
    ReleaseRehearsal,
}

impl LiveTestCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PositiveInference => "positive_inference",
            Self::FailureProbe => "failure_probe",
            Self::ProviderVerification => "provider_verification",
            Self::LiveCancellationProbe => "live_cancellation_probe",
            Self::OfflineDeterministic => "offline_deterministic",
            Self::ReleaseRehearsal => "release_rehearsal",
        }
    }
}

/// Canonical entry representing a registered test in the live-test inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveTestEntry {
    /// Integration test binary target name (e.g., "phase_29_5_real_model").
    pub test_binary: &'static str,
    /// Test function name (e.g., "real_minimal_intent_to_plan").
    pub test_name: &'static str,
    /// Explicit test category.
    pub category: LiveTestCategory,
    /// Human-readable explanation of what this test proves.
    pub description: &'static str,
    /// Minimum expected outbound model requests for positive verification.
    pub min_requests: usize,
    /// Cargo feature required (if any, e.g. "live-model-tests").
    pub required_feature: Option<&'static str>,
}

/// Authoritative canonical live test inventory for M31A.
pub const CANONICAL_LIVE_TEST_INVENTORY: &[LiveTestEntry] = &[
    // --- Phase 07: Model Provider & Context Repository ---
    LiveTestEntry {
        test_binary: "phase_07_models_context_repo",
        test_name: "test_live_nvidia_provider",
        category: LiveTestCategory::PositiveInference,
        description: "Direct real-model call through NvidiaProvider asserting authoritative usage and response",
        min_requests: 1,
        required_feature: None,
    },
    // --- Dynamic Model Discovery ---
    LiveTestEntry {
        test_binary: "dynamic_model_discovery",
        test_name: "test_live_nvidia_discovery_and_inference",
        category: LiveTestCategory::PositiveInference,
        description: "Live NVIDIA NIM /models inventory discovery and dynamic inference with discovered model",
        min_requests: 1,
        required_feature: None,
    },
    // --- Provider Capability Alignment ---
    LiveTestEntry {
        test_binary: "provider_capability_alignment",
        test_name: "test_condition_n_live_provider_execution",
        category: LiveTestCategory::PositiveInference,
        description: "Live NIM probe, discovery, and turn execution through RoutedModelCaller",
        min_requests: 1,
        required_feature: None,
    },
    // --- Onboarding Wizard Remediation ---
    LiveTestEntry {
        test_binary: "onboarding_wizard_remediation",
        test_name: "test_live_nvidia_provider_verification_with_env_key",
        category: LiveTestCategory::PositiveInference,
        description: "Setup wizard Step 7 live provider verification gate with environment credentials",
        min_requests: 1,
        required_feature: None,
    },
    // --- Golden Real Model Workflow ---
    LiveTestEntry {
        test_binary: "golden_real_model_workflow",
        test_name: "test_golden_real_model_workflow",
        category: LiveTestCategory::PositiveInference,
        description: "Full end-to-end golden workflow driving real Git repository changes and verification across tool-feedback turns",
        min_requests: 2,
        required_feature: None,
    },
    // --- Golden Interactive Session ---
    LiveTestEntry {
        test_binary: "golden_interactive_session",
        test_name: "test_golden_interactive_session",
        category: LiveTestCategory::PositiveInference,
        description: "End-to-end interactive session with multiple operator turns backed by genuine NVIDIA inference",
        min_requests: 2,
        required_feature: None,
    },
    // --- Phase 29.5: Real Model Suite ---
    LiveTestEntry {
        test_binary: "phase_29_5_real_model",
        test_name: "real_model_gate_and_configuration",
        category: LiveTestCategory::ProviderVerification,
        description: "Canonical provider configuration, authenticated endpoint probe, and live model discovery (no inference)",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_29_5_real_model",
        test_name: "real_minimal_intent_to_plan",
        category: LiveTestCategory::PositiveInference,
        description: "Intent-to-plan acceptance requiring separate real inference for intent discovery and plan generation",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_29_5_real_model",
        test_name: "real_consequential_decision_surfaces",
        category: LiveTestCategory::PositiveInference,
        description: "Consequential decision proof across live discovery and planning inference turns",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_29_5_real_model",
        test_name: "real_target_architecture_differentiation",
        category: LiveTestCategory::PositiveInference,
        description: "Domain-specific planning differentiation across distinct software domains",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_29_5_real_model",
        test_name: "real_tool_call_proposal_and_continuation",
        category: LiveTestCategory::PositiveInference,
        description: "Multi-turn tool-calling loop: model proposes tool, runtime executes, model receives feedback",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_29_5_real_model",
        test_name: "real_model_failure_classification",
        category: LiveTestCategory::FailureProbe,
        description: "Live failure-path probes: invalid key (AuthenticationFailed) and bad model ID",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_29_5_real_model",
        test_name: "real_research_execution_with_provenance",
        category: LiveTestCategory::PositiveInference,
        description: "Genesis controller autonomous research loop requiring at least three real provider calls",
        min_requests: 3,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_29_5_real_model",
        test_name: "real_end_to_end_dashboard_repair",
        category: LiveTestCategory::PositiveInference,
        description: "Full autonomous engineering repair requiring real model/tool feedback turns",
        min_requests: 2,
        required_feature: None,
    },
    // --- Phase 30: Agentic Core ---
    LiveTestEntry {
        test_binary: "phase_30_agentic_core",
        test_name: "test_real_model_nemotron_agentic_loop_if_configured",
        category: LiveTestCategory::PositiveInference,
        description: "Continuous agent loop step with real model, recording durable conversation turns",
        min_requests: 1,
        required_feature: None,
    },
    // --- Phase 32: Intent Understanding ---
    LiveTestEntry {
        test_binary: "phase_32_intent_understanding",
        test_name: "test_real_model_nemotron_intent_enrichment_if_configured",
        category: LiveTestCategory::PositiveInference,
        description: "Intent enrichment turn with real model through continuous agent loop",
        min_requests: 1,
        required_feature: None,
    },
    // --- Phase 33: Adaptive Execution ---
    LiveTestEntry {
        test_binary: "phase_33_adaptive_execution",
        test_name: "test_live_nemotron_adaptive_execution",
        category: LiveTestCategory::PositiveInference,
        description: "Live adaptive execution strategy step with real model reasoning",
        min_requests: 1,
        required_feature: None,
    },
    // --- Phase 40: Governed Execution ---
    LiveTestEntry {
        test_binary: "phase_40_governed_execution",
        test_name: "live_governed_planning_chain",
        category: LiveTestCategory::PositiveInference,
        description: "Governed planning spine requiring multiple real model requests for intent, plan, and task generation",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_40_governed_execution",
        test_name: "live_governed_planning_turn",
        category: LiveTestCategory::PositiveInference,
        description: "Single routed planning turn verifying production routing and tracer evidence",
        min_requests: 1,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_40_governed_execution",
        test_name: "live_model_tool_continuation_loop",
        category: LiveTestCategory::LiveCancellationProbe,
        description: "Real tool-feedback continuation requiring two dispatched requests, one completed inference, and mid-flight cancellation",
        min_requests: 2,
        required_feature: None,
    },
    // --- Phase 41: Integrity ---
    LiveTestEntry {
        test_binary: "phase_41_integrity",
        test_name: "live_phase41_model_tool_write_to_evidence",
        category: LiveTestCategory::PositiveInference,
        description: "Real model generates tool call -> executed -> verified -> satisfies evidence gate",
        min_requests: 1,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_41_integrity",
        test_name: "live_phase41_full_mission_repair",
        category: LiveTestCategory::PositiveInference,
        description: "Real routed call with authoritative usage settlement through Phase 41 path",
        min_requests: 1,
        required_feature: None,
    },
    // --- Phase 42: Production Proof ---
    LiveTestEntry {
        test_binary: "phase_42_production_proof",
        test_name: "live_phase42_journey_a_full_autonomous_mission",
        category: LiveTestCategory::PositiveInference,
        description: "Journey A: Full autonomous mission repair loop with real model and tool execution",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_42_production_proof",
        test_name: "live_phase42_journey_b_live_cancellation",
        category: LiveTestCategory::FailureProbe,
        description: "Journey B: pre-cancelled real-provider call must stop before any outbound request",
        min_requests: 0,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_42_production_proof",
        test_name: "live_phase42_journey_c_live_failure_honesty",
        category: LiveTestCategory::FailureProbe,
        description: "Journey C: Honest live failure classification without fabrication",
        min_requests: 1,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_42_production_proof",
        test_name: "live_phase42_journey_d_live_resume",
        category: LiveTestCategory::PositiveInference,
        description: "Journey D: Checkpoint resume continuity across real live turns",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_42_production_proof",
        test_name: "live_phase42_journey_e_accounting_fidelity",
        category: LiveTestCategory::PositiveInference,
        description: "Journey E: Token accounting fidelity with real NVIDIA provider",
        min_requests: 1,
        required_feature: None,
    },
    // --- Phase 43: Reliability ---
    LiveTestEntry {
        test_binary: "phase_43_reliability",
        test_name: "live_p43_reliability_multi_turn_evidence",
        category: LiveTestCategory::PositiveInference,
        description: "Two-turn real inference probe: the follow-up request carries forward prior-turn output",
        min_requests: 2,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_43_reliability",
        test_name: "live_p43_reliability_cancel_is_honest",
        category: LiveTestCategory::FailureProbe,
        description: "Live cancellation honesty probe: pre-cancelled token terminates honestly without a provider request",
        min_requests: 0,
        required_feature: None,
    },
    // --- Phase 44: Security Closure ---
    LiveTestEntry {
        test_binary: "phase_44_security_closure",
        test_name: "live_p44_security_probe_model_tool_accounting",
        category: LiveTestCategory::PositiveInference,
        description: "Real model with authoritative usage inside hardened execution boundary",
        min_requests: 1,
        required_feature: None,
    },
    // --- Phase 45: Release Validation ---
    LiveTestEntry {
        test_binary: "phase_45_release_validation",
        test_name: "live_p45_release_accounting_probe",
        category: LiveTestCategory::PositiveInference,
        description: "Release candidate accounting probe with real model",
        min_requests: 1,
        required_feature: None,
    },
    LiveTestEntry {
        test_binary: "phase_45_release_validation",
        test_name: "rehearsal_rc_manifest_verifies",
        category: LiveTestCategory::ReleaseRehearsal,
        description: "Clean-room rehearsal release output manifest and checksum verification (requires M31A_RC_DIR)",
        min_requests: 0,
        required_feature: None,
    },
    // --- Live Model E2E Suite ---
    LiveTestEntry {
        test_binary: "live_model_e2e",
        test_name: "test_live_model_full_production_lifecycle",
        category: LiveTestCategory::PositiveInference,
        description: "Full production lifecycle with multiple real model/tool turns (Phase 36.2)",
        min_requests: 2,
        required_feature: Some("live-model-tests"),
    },
    LiveTestEntry {
        test_binary: "live_model_e2e",
        test_name: "test_live_model_cannot_execute_without_authorization",
        category: LiveTestCategory::PositiveInference,
        description: "Real intent, plan, and task generation followed by an authorization denial",
        min_requests: 2,
        required_feature: Some("live-model-tests"),
    },
    LiveTestEntry {
        test_binary: "live_model_e2e",
        test_name: "test_live_model_stale_authorization_cannot_execute",
        category: LiveTestCategory::PositiveInference,
        description: "Multiple real planning turns prove stale authorization is invalidated on plan change",
        min_requests: 2,
        required_feature: Some("live-model-tests"),
    },
    LiveTestEntry {
        test_binary: "live_model_e2e",
        test_name: "test_live_model_failure_is_not_fabricated_as_success",
        category: LiveTestCategory::FailureProbe,
        description: "Negative failure honesty: provider errors are never converted to success",
        min_requests: 0,
        required_feature: Some("live-model-tests"),
    },
    LiveTestEntry {
        test_binary: "live_model_e2e",
        test_name: "test_live_invocation_guard_reports_credential_status",
        category: LiveTestCategory::OfflineDeterministic,
        description: "Diagnostic credential reporter confirming credentials and sanitized gate status",
        min_requests: 0,
        required_feature: Some("live-model-tests"),
    },
];

/// Retrieve the canonical live test inventory.
pub fn canonical_live_test_inventory() -> &'static [LiveTestEntry] {
    CANONICAL_LIVE_TEST_INVENTORY
}

/// Find a specific live test entry by binary and test name.
pub fn find_entry(test_binary: &str, test_name: &str) -> Option<&'static LiveTestEntry> {
    CANONICAL_LIVE_TEST_INVENTORY
        .iter()
        .find(|e| e.test_binary == test_binary && e.test_name == test_name)
}

/// Check if a given test is registered in the canonical live inventory.
pub fn is_registered_live_test(test_binary: &str, test_name: &str) -> bool {
    find_entry(test_binary, test_name).is_some()
}

/// Return all tests categorized as PositiveInference.
pub fn positive_acceptance_tests() -> Vec<&'static LiveTestEntry> {
    CANONICAL_LIVE_TEST_INVENTORY
        .iter()
        .filter(|e| e.category == LiveTestCategory::PositiveInference)
        .collect()
}

/// Return all tests categorized as FailureProbe.
pub fn failure_probe_tests() -> Vec<&'static LiveTestEntry> {
    CANONICAL_LIVE_TEST_INVENTORY
        .iter()
        .filter(|e| e.category == LiveTestCategory::FailureProbe)
        .collect()
}
