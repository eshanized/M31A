#!/usr/bin/env bash
# Deterministic regression fixtures for the canonical live-test runner.
# A fake cargo executable proves that exit-zero alone cannot produce green.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUNNER="${REPO_ROOT}/scripts/run_live_tests.sh"
TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "${TMP_ROOT}"' EXIT

mkdir -p "${TMP_ROOT}/bin"
cat > "${TMP_ROOT}/bin/cargo" <<'FAKE_CARGO'
#!/usr/bin/env bash
set -euo pipefail

case "${FAKE_CARGO_MODE:-success}" in
    zero-tests)
        # model a cargo invocation that exits zero but ran no test
        printf 'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s\n'
        ;;
    no-evidence)
        printf 'test test_live_nvidia_provider ... ok\n'
        printf 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 36 filtered out; finished in 0.00s\n'
        ;;
    success)
        printf 'test test_live_nvidia_provider ... ok\n'
        printf 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 36 filtered out; finished in 0.00s\n'
        printf 'request_attempt\nrequest\nresponse\ninference_request\ninference_response\n' >> "${M31A_LIVE_EVIDENCE_FILE}"
        ;;
    cancellation-success)
        printf 'test live_model_tool_continuation_loop ... ok\n'
        printf 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 36 filtered out; finished in 0.00s\n'
        # two real inference attempts; the second turn is cancelled in flight
        printf 'request_attempt\nrequest\nresponse\ninference_request\ninference_response\nrequest_attempt\ninference_request\n' >> "${M31A_LIVE_EVIDENCE_FILE}"
        ;;
    failing-test)
        printf 'running 1 test\n'
        printf 'thread '\''test_live_nvidia_provider'\'' panicked at tests/test.rs:10:5:\nassertion failed\n'
        printf 'failures:\n    test_live_nvidia_provider\n'
        printf 'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
        exit 101
        ;;
    *)
        printf 'unknown FAKE_CARGO_MODE\n' >&2
        exit 2
        ;;
esac
FAKE_CARGO
chmod +x "${TMP_ROOT}/bin/cargo"

run_fixture() {
    local mode="$1"
    local test_filter="${2:-model_context_and_repository_intelligence:none:test_live_nvidia_provider}"
    local case_dir="${TMP_ROOT}/${mode}"
    mkdir -p "${case_dir}"
    PATH="${TMP_ROOT}/bin:${PATH}" \
    API_KEY_NVIDIA="fixture-only-key" \
    TMPDIR="${case_dir}" \
    FAKE_CARGO_MODE="${mode}" \
        bash "${RUNNER}" --filter "${test_filter}" > "${case_dir}/runner.log" 2>&1
}

expect_rejection() {
    local mode="$1"
    if run_fixture "${mode}"; then
        printf 'ERROR: live runner incorrectly accepted fixture mode: %s\n' "${mode}" >&2
        cat "${TMP_ROOT}/${mode}/runner.log" >&2
        exit 1
    fi
}

expect_rejection zero-tests
expect_rejection no-evidence

if ! run_fixture success; then
    printf 'ERROR: live runner rejected a valid evidence fixture\n' >&2
    cat "${TMP_ROOT}/success/runner.log" >&2
    exit 1
fi

python3 - "${TMP_ROOT}/success/live-test-report.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert report["total"] == 1, report
assert report["executed"] == 1, report
assert report["passed"] == 1, report
assert report["failed"] == 0, report
test = report["results"][0]
assert test["observed_requests"] == 1, test
assert test["http_responses"] == 1, test
assert test["successful_responses"] == 1, test
assert test["inference_requests"] == 1, test
assert test["inference_responses"] == 1, test
assert test["status"] == "passed", test
PY

if ! run_fixture cancellation-success live_model_tool_continuation_loop; then
    printf 'ERROR: live runner rejected a valid in-flight-cancellation evidence fixture\n' >&2
    cat "${TMP_ROOT}/cancellation-success/runner.log" >&2
    exit 1
fi

python3 - "${TMP_ROOT}/cancellation-success/live-test-report.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert report["total"] == 1, report
assert report["executed"] == 1, report
assert report["passed"] == 1, report
test = report["results"][0]
assert test["category"] == "LiveCancellationProbe", test
assert test["observed_requests"] == 2, test
assert test["inference_requests"] == 2, test
assert test["inference_responses"] == 1, test
assert test["status"] == "passed", test
PY

# Verify failing test accounting
expect_rejection failing-test
python3 - "${TMP_ROOT}/failing-test/live-test-report.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert report["total"] == 1, report
assert report["planned"] == 1, report
assert report["started"] == 1, report
assert report["executed"] == 1, report
assert report["passed"] == 0, report
assert report["failed"] == 1, report
assert report["overall_result"] == "FAIL", report
test = report["results"][0]
assert test["status"] == "failed", test
assert "cargo test exited with status" in test["failure_reason"] or "failed" in test["failure_reason"]
PY

# Verify credential-blocked report generation
run_credential_blocked_fixture() {
    local case_dir="${TMP_ROOT}/credential-blocked"
    mkdir -p "${case_dir}"
    PATH="${TMP_ROOT}/bin:${PATH}" \
    NVIDIA_API_KEY="" \
    API_KEY_NVIDIA="" \
    M31A_NO_DOTENV="1" \
    TMPDIR="${case_dir}" \
        bash "${RUNNER}" --suite core > "${case_dir}/runner.log" 2>&1 || true
}
run_credential_blocked_fixture
python3 - "${TMP_ROOT}/credential-blocked/live-test-report.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert report["total"] == 5, report
assert report["planned"] == 5, report
assert report["started"] == 0, report
assert report["executed"] == 0, report
assert report["passed"] == 0, report
assert report["failed"] == 0, report
assert report["blocked"] == 5, report
assert report["not_run"] == 5, report
assert report["overall_result"] == "CREDENTIAL-BLOCKED", report
PY

printf 'live runner false-green regression fixtures: PASS\n'
