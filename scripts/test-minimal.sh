#!/usr/bin/env bash
# scripts/test-minimal.sh
# Minimal Resource Test Runner for M31A
#
# Prevents CPU exhaustion and OOM thrashing on resource-constrained systems
# by strictly bounding compiler jobs, test execution threads, and linker memory.
#
# Usage:
#   ./scripts/test-minimal.sh              # Runs library unit tests (minimal RAM/CPU)
#   ./scripts/test-minimal.sh --test <NAME> # Runs a specific integration test target
#   ./scripts/test-minimal.sh --all        # Runs unit tests + key integration test suites
#   ./scripts/test-minimal.sh --check      # Runs cargo check + minimal tests
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

# Bounded Resource Enforcements
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-2}"

# Optional CPU niceness to prevent UI/terminal freezing
NICECMD=""
if command -v nice >/dev/null 2>&1; then
  NICECMD="nice -n 19"
fi

MODE="lib"
TARGET_TEST=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --all)
      MODE="all"
      shift
      ;;
    --test)
      MODE="single"
      TARGET_TEST="$2"
      shift 2
      ;;
    --check)
      MODE="check"
      shift
      ;;
    --lib)
      MODE="lib"
      shift
      ;;
    -h|--help)
      echo "M31A Minimal Resource Test Runner"
      echo ""
      echo "Usage:"
      echo "  $0                  Run all unit tests in src/ with bounded resources"
      echo "  $0 --all            Run unit tests + critical integration test suites"
      echo "  $0 --test <NAME>    Run a single integration test target (e.g. docs_contract)"
      echo "  $0 --check          Run cargo check followed by unit tests"
      echo ""
      echo "Configured bounds:"
      echo "  CARGO_BUILD_JOBS  = ${CARGO_BUILD_JOBS}"
      echo "  RUST_TEST_THREADS = ${RUST_TEST_THREADS}"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo "Run '$0 --help' for usage."
      exit 1
      ;;
  esac
done

START_TIME=$(date +%s)
echo "=========================================================="
echo " M31A Minimal Resource Test Execution Engine"
echo " Build Jobs: ${CARGO_BUILD_JOBS} | Test Threads: ${RUST_TEST_THREADS}"
echo " Mode: ${MODE}"
echo "=========================================================="

run_cmd() {
  echo ""
  echo "==> Running: $*"
  if [ -n "${NICECMD}" ]; then
    ${NICECMD} "$@"
  else
    "$@"
  fi
}

case "${MODE}" in
  lib)
    run_cmd cargo test --lib -j "${CARGO_BUILD_JOBS}" -- --test-threads "${RUST_TEST_THREADS}"
    ;;

  single)
    if [ -z "${TARGET_TEST}" ]; then
      echo "Error: --test requires a test target name." >&2
      exit 1
    fi
    run_cmd cargo test --test "${TARGET_TEST}" -j "${CARGO_BUILD_JOBS}" -- --test-threads "${RUST_TEST_THREADS}"
    ;;

  check)
    run_cmd cargo check -j "${CARGO_BUILD_JOBS}"
    run_cmd cargo test --lib -j "${CARGO_BUILD_JOBS}" -- --test-threads "${RUST_TEST_THREADS}"
    ;;

  all)
    echo "Phase 1: Unit & Component Tests (src/)"
    run_cmd cargo test --lib -j "${CARGO_BUILD_JOBS}" -- --test-threads "${RUST_TEST_THREADS}"

    echo "Phase 2: Architectural Contract & Documentation Tests"
    run_cmd cargo test --test docs_contract -j "${CARGO_BUILD_JOBS}" -- --test-threads "${RUST_TEST_THREADS}"

    echo "Phase 3: PromptOS V2 Behavioral Verification"
    run_cmd cargo test --test promptos_v2_behavioral -j "${CARGO_BUILD_JOBS}" -- --test-threads "${RUST_TEST_THREADS}"

    echo "Phase 4: Workflow Prompts Contract Verification"
    run_cmd cargo test --test workflow_prompts -j "${CARGO_BUILD_JOBS}" -- --test-threads "${RUST_TEST_THREADS}"
    ;;
esac

END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

echo ""
echo "=========================================================="
echo " Test Execution Succeeded in ${ELAPSED}s"
echo " System Resources Maintained Within Safe Operating Bounds"
echo "=========================================================="
