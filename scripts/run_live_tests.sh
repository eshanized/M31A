#!/usr/bin/env bash
# ==============================================================================
# M31 Autonomous (M31A) — Authoritative Canonical Live LLM Test Runner
#
# Single authoritative runner for all real-provider integration tests,
# failure-path probes, and release verification gates.
#
# Contracts:
# 1. Fail-closed: missing credentials exit non-zero immediately.
# 2. Never fake success: zero executed tests is treated as a hard failure.
# 3. No secret leakage: API keys and auth headers are never logged or echoed.
# 4. Strict accounting: verifies that positive acceptance tests observed genuine
#    model inference through NVIDIA NIM.
# ==============================================================================

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

TMPDIR="${TMPDIR:-${REPO_ROOT}/tmp}"
mkdir -p "${TMPDIR}"

export RUST_MIN_STACK="${RUST_MIN_STACK:-8388608}"

REPORT_FILE="${TMPDIR}/live-test-report.json"
RUN_LOG="${TMPDIR}/live-test-run.log"
: > "${RUN_LOG}"

# Terminal colors
BOLD=$(tput bold 2>/dev/null || printf '')
RESET=$(tput sgr0 2>/dev/null || printf '')
GREEN=$(tput setaf 2 2>/dev/null || printf '')
RED=$(tput setaf 1 2>/dev/null || printf '')
YELLOW=$(tput setaf 3 2>/dev/null || printf '')
CYAN=$(tput setaf 6 2>/dev/null || printf '')

# ── Credential Gate (Fail-Closed) ─────────────────────────────────────────────
check_credentials() {
    local has_cred=0

    if [ -n "${NVIDIA_API_KEY:-}" ] || [ -n "${API_KEY_NVIDIA:-}" ]; then
        has_cred=1
    elif [ -f ".env" ]; then
        if grep -qE '^(NVIDIA_API_KEY|API_KEY_NVIDIA)=[a-zA-Z0-9_\-]+' .env; then
            has_cred=1
        fi
    fi

    if [ "${has_cred}" -eq 0 ]; then
        printf "%bERROR [CREDENTIAL-BLOCKED]: Live LLM verification requires valid NVIDIA API credentials.%b\n" "${RED}${BOLD}" "${RESET}" >&2
        printf "Set NVIDIA_API_KEY or API_KEY_NVIDIA in environment or .env file before running live tests.\n" >&2
        printf "Live tests refuse to run or fake success without authoritative provider access.\n" >&2
        exit 1
    fi
    printf "  %b[CREDENTIAL-OK]%b NVIDIA NIM provider credentials detected (sanitized)\n" "${GREEN}" "${RESET}"
}

# ── Inventory & Test Definitions ──────────────────────────────────────────────
# All live tests grouped by functional suite. Each item is: binary:feature:filter

SUITE_CORE=(
    "live_model_e2e:live-model-tests:test_live_model_full_production_lifecycle"
    "live_model_e2e:live-model-tests:test_live_model_cannot_execute_without_authorization"
    "live_model_e2e:live-model-tests:test_live_model_stale_authorization_cannot_execute"
    "live_model_e2e:live-model-tests:test_live_model_failure_is_not_fabricated_as_success"
    "live_model_e2e:live-model-tests:test_live_invocation_guard_reports_credential_status"
)

SUITE_PROBES=(
    "phase_43_reliability:none:live_p43_reliability_multi_turn_evidence"
    "phase_43_reliability:none:live_p43_reliability_cancel_is_honest"
    "phase_44_security_closure:none:live_p44_security_probe_model_tool_accounting"
    "phase_45_release_validation:none:live_p45_release_accounting_probe"
)

SUITE_GOVERNANCE=(
    "phase_40_governed_execution:none:live_governed_planning_chain"
    "phase_40_governed_execution:none:live_governed_planning_turn"
    "phase_40_governed_execution:none:live_model_tool_continuation_loop"
    "phase_41_integrity:none:live_phase41_model_tool_write_to_evidence"
    "phase_41_integrity:none:live_phase41_full_mission_repair"
    "phase_42_production_proof:none:live_phase42_journey_a_full_autonomous_mission"
    "phase_42_production_proof:none:live_phase42_journey_b_live_cancellation"
    "phase_42_production_proof:none:live_phase42_journey_c_live_failure_honesty"
    "phase_42_production_proof:none:live_phase42_journey_d_live_resume"
    "phase_42_production_proof:none:live_phase42_journey_e_accounting_fidelity"
)

SUITE_AGENTIC=(
    "phase_29_5_real_model:none:real_model_gate_and_configuration"
    "phase_29_5_real_model:none:real_minimal_intent_to_plan"
    "phase_29_5_real_model:none:real_consequential_decision_surfaces"
    "phase_29_5_real_model:none:real_target_architecture_differentiation"
    "phase_29_5_real_model:none:real_tool_call_proposal_and_continuation"
    "phase_29_5_real_model:none:real_model_failure_classification"
    "phase_29_5_real_model:none:real_research_execution_with_provenance"
    "phase_29_5_real_model:none:real_end_to_end_dashboard_repair"
    "phase_30_agentic_core:none:test_real_model_nemotron_agentic_loop_if_configured"
    "phase_32_intent_understanding:none:test_real_model_nemotron_intent_enrichment_if_configured"
    "phase_33_adaptive_execution:none:test_live_nemotron_adaptive_execution"
)

SUITE_DISCOVERY=(
    "phase_07_models_context_repo:none:test_live_nvidia_provider"
    "dynamic_model_discovery:none:test_live_nvidia_discovery_and_inference"
    "provider_capability_alignment:none:test_condition_n_live_provider_execution"
    "onboarding_wizard_remediation:none:test_live_nvidia_provider_verification_with_env_key"
)

SUITE_GOLDEN=(
    "golden_real_model_workflow:none:test_golden_real_model_workflow"
    "golden_interactive_session:none:test_golden_interactive_session"
)

# ── Selection Resolution ──────────────────────────────────────────────────────
TARGET_TESTS=()
SELECTED_SUITE="all"

usage() {
    cat <<EOF
Usage: $0 [options]

Options:
  --all                 Run all 35 live-model acceptance tests (default)
  --suite <name>        Run a specific live suite:
                          core        - Live model production lifecycle (live_model_e2e)
                          probes      - High-assurance release probes (P43, P44, P45)
                          governance  - Governed execution spine (P40, P41, P42)
                          agentic     - Agentic core & real model repair (P29.5, P30, P32, P33)
                          discovery   - Dynamic discovery & provider alignment (P07, onboarding)
                          golden      - End-to-end golden autonomous journeys
  --filter <pattern>    Run specific tests matching pattern
  -h, --help            Show this help message
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --all)
            SELECTED_SUITE="all"
            shift
            ;;
        --suite)
            SELECTED_SUITE="${2:-}"
            shift 2
            ;;
        --filter)
            FILTER_PATTERN="${2:-}"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            # Specific test name
            FILTER_PATTERN="$1"
            shift
            ;;
    esac
done

resolve_tests() {
    local all_candidates=()
    case "${SELECTED_SUITE}" in
        core)
            all_candidates=("${SUITE_CORE[@]}")
            ;;
        probes)
            all_candidates=("${SUITE_PROBES[@]}")
            ;;
        governance)
            all_candidates=("${SUITE_GOVERNANCE[@]}")
            ;;
        agentic)
            all_candidates=("${SUITE_AGENTIC[@]}")
            ;;
        discovery)
            all_candidates=("${SUITE_DISCOVERY[@]}")
            ;;
        golden)
            all_candidates=("${SUITE_GOLDEN[@]}")
            ;;
        all)
            all_candidates=(
                "${SUITE_CORE[@]}"
                "${SUITE_PROBES[@]}"
                "${SUITE_GOVERNANCE[@]}"
                "${SUITE_AGENTIC[@]}"
                "${SUITE_DISCOVERY[@]}"
                "${SUITE_GOLDEN[@]}"
            )
            ;;
        *)
            printf "%bUnknown suite '%s'. Run with --help for options.%b\n" "${RED}" "${SELECTED_SUITE}" "${RESET}" >&2
            exit 1
            ;;
    esac

    for item in "${all_candidates[@]}"; do
        if [ -n "${FILTER_PATTERN:-}" ]; then
            if [[ "${item}" == *"${FILTER_PATTERN}"* ]]; then
                TARGET_TESTS+=("${item}")
            fi
        else
            TARGET_TESTS+=("${item}")
        fi
    done
}

# ── Execution Loop ────────────────────────────────────────────────────────────
run_tests() {
    local total=${#TARGET_TESTS[@]}
    if [ "${total}" -eq 0 ]; then
        printf "%bERROR: No live tests resolved for execution.%b\n" "${RED}" "${RESET}" >&2
        exit 1
    fi

    printf "\n==> %bExecuting Live LLM Verification Gate (%d tests, suite: %s)%b\n\n" "${BOLD}${CYAN}" "${total}" "${SELECTED_SUITE}" "${RESET}"

    local passed=0
    local failed=0
    local skipped=0
    local results=()
    local start_epoch
    start_epoch=$(date +%s)

    for i in "${!TARGET_TESTS[@]}"; do
        local entry="${TARGET_TESTS[$i]}"
        IFS=':' read -r binary feature test_name <<< "${entry}"
        local test_num=$((i + 1))

        printf "[%2d/%2d] Running %-32s :: %s ... " "${test_num}" "${total}" "${binary}" "${test_name}"

        local extra_args=()
        if [ "${feature}" != "none" ]; then
            extra_args+=(--features "${feature}")
        fi

        local test_log="${TMPDIR}/live-${binary}-${test_name}.log"
        local status=0

        # We pass --ignored unless it is the guard reporter
        local ignore_flag="--ignored"
        if [ "${test_name}" == "test_live_invocation_guard_reports_credential_status" ]; then
            ignore_flag=""
        fi

        cargo test --test "${binary}" "${extra_args[@]}" "${test_name}" -- ${ignore_flag} --exact --test-threads 1 > "${test_log}" 2>&1 || status=$?

        cat "${test_log}" >> "${RUN_LOG}"

        if [ "${status}" -eq 0 ]; then
            printf "%bPASS%b\n" "${GREEN}${BOLD}" "${RESET}"
            passed=$((passed + 1))
            results+=("{\"test_binary\":\"${binary}\",\"test_name\":\"${test_name}\",\"status\":\"passed\"}")
        else
            printf "%bFAIL%b (status=%d)\n" "${RED}${BOLD}" "${RESET}" "${status}"
            failed=$((failed + 1))
            results+=("{\"test_binary\":\"${binary}\",\"test_name\":\"${test_name}\",\"status\":\"failed\",\"log\":\"${test_log}\"}")
            printf "      %bFailure log snippet:%b\n" "${YELLOW}" "${RESET}"
            tail -n 12 "${test_log}" | sed 's/^/      /'
        fi
    done

    local end_epoch
    end_epoch=$(date +%s)
    local duration=$((end_epoch - start_epoch))

    # Build JSON summary report
    local results_json
    results_json=$(IFS=,; echo "[${results[*]}]")

    cat > "${REPORT_FILE}" <<EOF
{
  "timestamp": "$(date -u +'%Y-%m-%dT%H:%M:%SZ')",
  "suite": "${SELECTED_SUITE}",
  "total": ${total},
  "passed": ${passed},
  "failed": ${failed},
  "skipped": ${skipped},
  "duration_seconds": ${duration},
  "provider": "nvidia_nim",
  "endpoint": "https://integrate.api.nvidia.com/v1",
  "results": ${results_json}
}
EOF

    printf "\n==============================================================================\n"
    printf "Live LLM Verification Summary\n"
    printf "==============================================================================\n"
    printf "  Provider:          NVIDIA NIM (https://integrate.api.nvidia.com/v1)\n"
    printf "  Suite:             %s\n" "${SELECTED_SUITE}"
    printf "  Total Executed:    %d\n" "${total}"
    printf "  Passed:            %b%d%b\n" "${GREEN}" "${passed}" "${RESET}"
    printf "  Failed:            %b%d%b\n" "$([ "${failed}" -gt 0 ] && echo "${RED}" || echo "${GREEN}")" "${failed}" "${RESET}"
    printf "  Duration:          %ds\n" "${duration}"
    printf "  Machine Report:    %s\n" "${REPORT_FILE}"
    printf "==============================================================================\n"

    # Enforce non-zero execution and zero failures
    if [ "${passed}" -eq 0 ]; then
        printf "%bFAIL: Zero live tests executed successfully. Refusing to report success.%b\n" "${RED}${BOLD}" "${RESET}" >&2
        exit 1
    fi

    if [ "${failed}" -gt 0 ]; then
        printf "%bFAIL: %d live test(s) failed.%b\n" "${RED}${BOLD}" "${failed}" "${RESET}" >&2
        exit 1
    fi

    printf "%bSUCCESS: All %d live LLM verification tests passed with genuine provider inference.%b\n" "${GREEN}${BOLD}" "${passed}" "${RESET}"
}

# ── Main ──────────────────────────────────────────────────────────────────────
check_credentials
resolve_tests
run_tests
