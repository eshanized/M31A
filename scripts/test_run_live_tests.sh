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
        printf 'request\nresponse\ninference_request\ninference_response\n' >> "${M31A_LIVE_EVIDENCE_FILE}"
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
    local case_dir="${TMP_ROOT}/${mode}"
    mkdir -p "${case_dir}"
    PATH="${TMP_ROOT}/bin:${PATH}" \
    API_KEY_NVIDIA="fixture-only-key" \
    TMPDIR="${case_dir}" \
    FAKE_CARGO_MODE="${mode}" \
        bash "${RUNNER}" --filter test_live_nvidia_provider > "${case_dir}/runner.log" 2>&1
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
assert test["successful_responses"] == 1, test
assert test["inference_requests"] == 1, test
assert test["inference_responses"] == 1, test
assert test["status"] == "passed", test
PY

printf 'live runner false-green regression fixtures: PASS\n'
