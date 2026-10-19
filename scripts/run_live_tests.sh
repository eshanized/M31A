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
export M31A_MODEL="${M31A_MODEL:-nvidia/nemotron-3-ultra-550b-a55b}"
export NVIDIA_MODEL="${NVIDIA_MODEL:-nvidia/nemotron-3-ultra-550b-a55b}"

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
  --all                 Run all runnable tests registered in the canonical inventory (default)
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
            if [[ $# -lt 2 || -z "${2:-}" ]]; then
                printf "%bERROR: --suite requires a suite name.%b\\n" "${RED}" "${RESET}" >&2
                exit 1
            fi
            SELECTED_SUITE="$2"
            shift 2
            ;;
        --filter)
            if [[ $# -lt 2 || -z "${2:-}" ]]; then
                printf "%bERROR: --filter requires a test-name pattern.%b\\n" "${RED}" "${RESET}" >&2
                exit 1
            fi
            FILTER_PATTERN="$2"
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

# ── Inventory Metadata ───────────────────────────────────────────────────────
# Resolve metadata from the same canonical Rust inventory used by architecture
# guards. Suite lists select tests; this lookup supplies the category, request
# floor, and required feature for every invocation.
lookup_inventory_entry() {
    python3 - "${REPO_ROOT}/tests/support/live_inventory.rs" "$1" "$2" <<'PY'
import re
import sys
from pathlib import Path

source = Path(sys.argv[1]).read_text(encoding="utf-8")
wanted_binary, wanted_test = sys.argv[2], sys.argv[3]

for match in re.finditer(r"LiveTestEntry\s*\{([^}]*)\}", source, re.S):
    entry = match.group(1)
    binary = re.search(r'test_binary:\s*"([^"]+)"', entry)
    name = re.search(r'test_name:\s*"([^"]+)"', entry)
    category = re.search(r'category:\s*LiveTestCategory::([A-Za-z]+)', entry)
    minimum = re.search(r'min_requests:\s*(\d+)', entry)
    feature = re.search(r'required_feature:\s*Some\("([^"]+)"\)|required_feature:\s*None', entry)
    if not all((binary, name, category, minimum, feature)):
        continue
    if binary.group(1) == wanted_binary and name.group(1) == wanted_test:
        required_feature = feature.group(1) or "-"
        print(f"{category.group(1)}\t{minimum.group(1)}\t{required_feature}")
        sys.exit(0)

sys.exit(2)
PY
}

validate_inventory_selection() {
    local entry binary feature test_name metadata category min_requests inventory_feature expected_feature
    for entry in "${TARGET_TESTS[@]}"; do
        IFS=':' read -r binary feature test_name <<< "${entry}"
        if ! metadata="$(lookup_inventory_entry "${binary}" "${test_name}")"; then
            printf "%bERROR: Runner target is absent from canonical live inventory: %s::%s%b\n" "${RED}" "${binary}" "${test_name}" "${RESET}" >&2
            exit 1
        fi
        IFS=$'\t' read -r category min_requests inventory_feature <<< "${metadata}"
        expected_feature="${inventory_feature}"
        if [ "${expected_feature}" = "-" ]; then
            expected_feature="none"
        fi
        if [ "${feature}" != "${expected_feature}" ]; then
            printf "%bERROR: Feature drift for %s::%s (runner=%s inventory=%s).%b\n" "${RED}" "${binary}" "${test_name}" "${feature}" "${expected_feature}" "${RESET}" >&2
            exit 1
        fi
        case "${category}" in
            PositiveInference|FailureProbe|ProviderVerification|LiveCancellationProbe|OfflineDeterministic)
                ;;
            *)
                printf "%bERROR: Unsupported live-test category '%s' for %s::%s.%b\n" "${RED}" "${category}" "${binary}" "${test_name}" "${RESET}" >&2
                exit 1
                ;;
        esac
        if [ "${category}" = "PositiveInference" ] && [ "${min_requests}" -lt 1 ]; then
            printf "%bERROR: Positive inference test has no minimum request requirement: %s::%s%b\n" "${RED}" "${binary}" "${test_name}" "${RESET}" >&2
            exit 1
        fi
        if [ "${category}" = "ProviderVerification" ] && [ "${min_requests}" -lt 1 ]; then
            printf "%bERROR: Provider verification has no minimum request requirement: %s::%s%b\n" "${RED}" "${binary}" "${test_name}" "${RESET}" >&2
            exit 1
        fi
        if [ "${category}" = "LiveCancellationProbe" ] && [ "${min_requests}" -lt 2 ]; then
            printf "%bERROR: Live cancellation probes must require at least two real inference attempts: %s::%s%b\n" "${RED}" "${binary}" "${test_name}" "${RESET}" >&2
            exit 1
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

    validate_inventory_selection
    printf "\n==> %bExecuting Live LLM Verification Gate (%d tests, suite: %s)%b\n\n" "${BOLD}${CYAN}" "${total}" "${SELECTED_SUITE}" "${RESET}"

    local passed=0
    local failed=0
    local executed=0
    local results=()
    local start_epoch
    start_epoch=$(date +%s)

    for i in "${!TARGET_TESTS[@]}"; do
        local entry="${TARGET_TESTS[$i]}"
        IFS=':' read -r binary feature test_name <<< "${entry}"
        local test_num=$((i + 1))

        local metadata category min_requests inventory_feature
        metadata="$(lookup_inventory_entry "${binary}" "${test_name}")"
        IFS=$'\t' read -r category min_requests inventory_feature <<< "${metadata}"

        printf "[%2d/%2d] Running %-32s :: %s ... " "${test_num}" "${total}" "${binary}" "${test_name}"

        local extra_args=()
        if [ "${feature}" != "none" ]; then
            extra_args+=(--features "${feature}")
        fi

        local test_log="${TMPDIR}/live-${binary}-${test_name}.log"
        local evidence_file="${TMPDIR}/live-evidence-${binary}-${test_name}.events"
        : > "${evidence_file}"
        local status=0
        local ignore_args=(--ignored)
        if [ "${category}" = "OfflineDeterministic" ]; then
            ignore_args=()
        fi

        # Each cargo invocation is exact and isolated, and the provider writes
        # only sanitized event names into this per-test evidence file.
        CARGO_TERM_COLOR=never M31A_LIVE_EVIDENCE_FILE="${evidence_file}" cargo test --test "${binary}" "${extra_args[@]}" "${test_name}" -- "${ignore_args[@]}" --exact --test-threads 1 > "${test_log}" 2>&1 || status=$?

        cat "${test_log}" >> "${RUN_LOG}"

        local test_run_count=0
        local passing_summary_count=0
        local request_attempt_count=0
        local http_response_count=0
        local response_count=0
        local inference_request_count=0
        local inference_response_count=0
        test_run_count=$(grep -Fxc "test ${test_name} ... ok" "${test_log}" || true)
        if [ "${test_run_count}" -eq 0 ] && grep -Fq "test ${test_name} ... " "${test_log}"; then
            if grep -Eq "^ok$" "${test_log}"; then
                test_run_count=1
            fi
        fi
        passing_summary_count=$(grep -Fc 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;' "${test_log}" || true)
        request_attempt_count=$(grep -cx 'request_attempt' "${evidence_file}" || true)
        http_response_count=$(grep -cx 'request' "${evidence_file}" || true)
        response_count=$(grep -cx 'response' "${evidence_file}" || true)
        inference_request_count=$(grep -cx 'inference_request' "${evidence_file}" || true)
        inference_response_count=$(grep -cx 'inference_response' "${evidence_file}" || true)
        executed=$((executed + test_run_count))

        local failure_reason=""
        if [ "${status}" -ne 0 ]; then
            failure_reason="cargo test exited with status ${status}"
        elif [ "${test_run_count}" -ne 1 ] || [ "${passing_summary_count}" -ne 1 ]; then
            failure_reason="cargo reported success without exactly one executed-and-passing test (matched test lines=${test_run_count}, passing summaries=${passing_summary_count})"
        elif [ "${category}" = "PositiveInference" ] && { [ "${inference_request_count}" -lt "${min_requests}" ] || [ "${inference_response_count}" -lt "${min_requests}" ]; }; then
            failure_reason="live inference evidence below inventory minimum ${min_requests} (requests=${inference_request_count}, successful responses=${inference_response_count})"
        elif [ "${category}" = "ProviderVerification" ] && { [ "${request_attempt_count}" -lt "${min_requests}" ] || [ "${response_count}" -lt "${min_requests}" ]; }; then
            failure_reason="provider verification evidence below inventory minimum ${min_requests} (request attempts=${request_attempt_count}, successful responses=${response_count})"
        elif [ "${category}" = "LiveCancellationProbe" ] && { [ "${inference_request_count}" -lt "${min_requests}" ] || [ "${inference_response_count}" -lt 1 ]; }; then
            failure_reason="live cancellation evidence requires at least ${min_requests} inference request attempts and one completed inference (attempts=${inference_request_count}, completed=${inference_response_count})"
        elif [ "${category}" = "FailureProbe" ] && [ "${request_attempt_count}" -lt "${min_requests}" ]; then
            failure_reason="failure-probe outbound request attempts below inventory minimum ${min_requests} (attempts=${request_attempt_count})"
        fi

        if [ -z "${failure_reason}" ]; then
            printf "%bPASS%b (attempts=%d http_responses=%d successful_responses=%d inference_responses=%d)\n" "${GREEN}${BOLD}" "${RESET}" "${request_attempt_count}" "${http_response_count}" "${response_count}" "${inference_response_count}"
            passed=$((passed + 1))
            results+=("{\"test_binary\":\"${binary}\",\"test_name\":\"${test_name}\",\"category\":\"${category}\",\"min_requests\":${min_requests},\"observed_requests\":${request_attempt_count},\"http_responses\":${http_response_count},\"successful_responses\":${response_count},\"inference_requests\":${inference_request_count},\"inference_responses\":${inference_response_count},\"status\":\"passed\"}")
        else
            printf "%bFAIL%b — %s\n" "${RED}${BOLD}" "${RESET}" "${failure_reason}"
            failed=$((failed + 1))
            results+=("{\"test_binary\":\"${binary}\",\"test_name\":\"${test_name}\",\"category\":\"${category}\",\"min_requests\":${min_requests},\"observed_requests\":${request_attempt_count},\"http_responses\":${http_response_count},\"successful_responses\":${response_count},\"inference_requests\":${inference_request_count},\"inference_responses\":${inference_response_count},\"status\":\"failed\"}")
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
  "executed": ${executed},
  "passed": ${passed},
  "failed": ${failed},
  "duration_seconds": ${duration},
  "provider": "nvidia_nim",
  "endpoint": "https://integrate.api.nvidia.com/v1",
  "evidence_source": "sanitized provider dispatch attempts, HTTP responses, and completed inference events",
  "results": ${results_json}
}
EOF

    printf "\n==============================================================================\n"
    printf "Live LLM Verification Summary\n"
    printf "==============================================================================\n"
    printf "  Provider:          NVIDIA NIM (https://integrate.api.nvidia.com/v1)\n"
    printf "  Suite:             %s\n" "${SELECTED_SUITE}"
    printf "  Inventory Targets: %d\n" "${total}"
    printf "  Executed:          %d\n" "${executed}"
    printf "  Passed:            %b%d%b\n" "${GREEN}" "${passed}" "${RESET}"
    printf "  Failed:            %b%d%b\n" "$([ "${failed}" -gt 0 ] && echo "${RED}" || echo "${GREEN}")" "${failed}" "${RESET}"
    printf "  Duration:          %ds\n" "${duration}"
    printf "  Machine Report:    %s\n" "${REPORT_FILE}"
    printf "==============================================================================\n"

    if [ "${executed}" -ne "${total}" ]; then
        printf "%bFAIL: Expected %d tests to execute but observed %d.%b\n" "${RED}${BOLD}" "${total}" "${executed}" "${RESET}" >&2
        exit 1
    fi
    if [ "${passed}" -ne "${total}" ] || [ "${failed}" -gt 0 ]; then
        printf "%bFAIL: %d live verification test(s) failed strict result/evidence checks.%b\n" "${RED}${BOLD}" "${failed}" "${RESET}" >&2
        exit 1
    fi

    printf "%bSUCCESS: All %d canonical live tests executed and passed strict provider-evidence checks.%b\n" "${GREEN}${BOLD}" "${passed}" "${RESET}"
}

# ── Main ──────────────────────────────────────────────────────────────────────
check_credentials
resolve_tests
run_tests
