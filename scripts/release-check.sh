#!/usr/bin/env bash
# scripts/release-check.sh
# AUTHORITATIVE local production release gate definition (P1-06).
#
# This script is the single source of truth for the mandatory pre-release
# gate SET. CI (.github/workflows/release.yml `gates` job and
# release-gates.yml) invokes this script and then adds only the slow,
# machine-specific enforcement (full test suite, per-target identity).
# Do not define a second drifting gate list anywhere: update this file.
#
# Usage:
#   ./scripts/release-check.sh
#
# Gates run in order:
#   1.  Git clean tree check (BLOCKING)
#   2.  Secret scan — canonical pattern, see gate body (BLOCKING)
#   3.  Version consistency: Cargo.toml (sole authority) vs CHANGELOG (BLOCKING)
#   4.  cargo fmt --check (BLOCKING)
#   5a. cargo check --all-targets (production channel, BLOCKING)
#   5b. cargo check --all-targets --features development (BLOCKING)
#   5c. Invalid channel combination MUST fail (mutual exclusion, BLOCKING)
#   6a. cargo clippy --all-targets -- -D warnings (production, BLOCKING)
#   6b. cargo clippy --all-targets --features development -- -D warnings (BLOCKING)
#   7.  cargo test (bounded minimal subset; CI enforces the FULL suite) (BLOCKING)
#   8.  cargo audit — OPTIONAL advisory (SKIP when not installed; never blocking)
#   9.  Binary smoke tests (SKIP when release binary absent; BLOCKING when present)
#
# NOTE on `--all-features`: the `development` and `production` cargo features
# are mutually exclusive (compile error). Gates therefore exercise each
# channel separately and MUST NOT use `--all-features`.
#
# Taxonomy labels used below: BLOCKING (must pass), SKIP (tool/artifact
# absent, explicitly listed), OPTIONAL (advisory only).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

PASS=0
FAIL=0

pass() { echo "  [PASS] $1"; PASS=$((PASS + 1)); }
fail() { echo "  [FAIL] $1" >&2; FAIL=$((FAIL + 1)); }
header() { echo ""; echo "==> $1"; }

# ── 1. Clean tree ─────────────────────────────────────────────────────────────
header "1/10  Working tree state"
if [ -z "$(git status --porcelain)" ]; then
  pass "Working tree is clean"
else
  fail "Working tree has uncommitted changes (git status --short):"
  git status --short >&2
fi

# ── 2. Secret scan ────────────────────────────────────────────────────────────
# Canonical secret scan (mirrored by .github/workflows/development.yml and
# documented in docs/RELEASE.md — do not redefine elsewhere).
header "2/10  Secret scan"

SECRET_HITS=$(grep -rn "nvapi-[A-Za-z0-9_-]\{30\}" --include="*.rs" --include="*.toml" src/ tests/ 2>/dev/null | grep -v "test" | grep -v "dummy" || true)
if [ -z "${SECRET_HITS}" ]; then
  pass "No nvapi-* real keys in src/ or tests/"
else
  fail "Possible real nvapi key found:"
  echo "${SECRET_HITS}" >&2
fi

ENV_TRACKED=$(git ls-files .env 2>/dev/null || true)
if [ -z "${ENV_TRACKED}" ]; then
  pass ".env is not tracked by git"
else
  fail ".env is tracked by git — remove it from tracking immediately"
fi

CREDS_TRACKED=$(git ls-files .m31a/ 2>/dev/null || true)
if [ -z "${CREDS_TRACKED}" ]; then
  pass ".m31a/ is not tracked by git"
else
  fail ".m31a/ is tracked by git: ${CREDS_TRACKED}"
fi

# ── 3. Version consistency ────────────────────────────────────────────────────
header "3/10  Version consistency"

VERSION=$(cargo metadata --no-deps --format-version=1 \
  | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d["packages"][0]["version"])')
echo "  Cargo.toml version: ${VERSION}"

CHANGELOG_ENTRY=$(grep -m1 "^\## \[${VERSION}\]" CHANGELOG.md 2>/dev/null || true)
if [ -n "${CHANGELOG_ENTRY}" ]; then
  pass "CHANGELOG.md has entry for [${VERSION}]"
else
  fail "CHANGELOG.md missing entry for [${VERSION}]"
fi

# ── 4. Format ─────────────────────────────────────────────────────────────────
header "4/10  cargo fmt --check"
if cargo fmt --check 2>&1; then
  pass "cargo fmt --check"
else
  fail "cargo fmt --check (run 'cargo fmt' to fix)"
fi

# ── 5. Check (per channel; never --all-features) ─────────────────────────
header "5a/10  cargo check --all-targets (production channel)"
if cargo check --all-targets 2>&1; then
  pass "cargo check (production)"
else
  fail "cargo check (production) failed"
  exit 1
fi

header "5b/10  cargo check --all-targets --features development"
if cargo check --all-targets --features development 2>&1; then
  pass "cargo check (development)"
else
  fail "cargo check (development) failed"
  exit 1
fi

header "5c/10  Invalid channel combination must fail (mutual exclusion)"
if cargo check --lib --features development,production 2>&1 | grep -qi "mutually exclusive"; then
  pass "development+production correctly rejected"
else
  fail "development+production was NOT rejected — mutual exclusion broken"
  exit 1
fi

# ── 6. Clippy (per channel) ───────────────────────────────────────────────
header "6a/10  cargo clippy --all-targets -- -D warnings (production)"
if cargo clippy --all-targets -- -D warnings 2>&1; then
  pass "cargo clippy (production)"
else
  fail "cargo clippy (production) produced warnings or errors"
  exit 1
fi

header "6b/10  cargo clippy --all-targets --features development -- -D warnings"
if cargo clippy --all-targets --features development -- -D warnings 2>&1; then
  pass "cargo clippy (development)"
else
  fail "cargo clippy (development) produced warnings or errors"
  exit 1
fi

# ── 7. Tests ──────────────────────────────────────────────────────────────
header "7/10  cargo test (minimal resources: bounded jobs and test threads)"
if cargo test --lib -j 2 -- --test-threads 2 2>&1 && \
   cargo test --test docs_contract -j 2 -- --test-threads 2 2>&1 && \
   cargo test --test promptos_v2_behavioral -j 2 -- --test-threads 2 2>&1; then
  pass "cargo test (bounded minimal resources passed)"
else
  fail "cargo test: tests failed under bounded execution"
  exit 1
fi

# ── 8. cargo audit (OPTIONAL advisory — never blocking) ─────────────────────
header "8/10  cargo audit (OPTIONAL advisory)"
if command -v cargo-audit >/dev/null 2>&1 || cargo audit --version >/dev/null 2>&1; then
  if cargo audit 2>&1; then
    pass "cargo audit: no known vulnerabilities"
  else
    fail "cargo audit: advisories found — review before release"
  fi
else
  echo "  [SKIP] cargo-audit not installed (cargo install cargo-audit)"
fi

# ── 9. Binary smoke test ──────────────────────────────────────────────────────
header "9/10  Binary smoke test (release binary)"
BINARY="target/release/m31a"
if [ -f "${BINARY}" ]; then
  BINARY_VERSION=$("${BINARY}" --version 2>&1 || true)
  if [[ "${BINARY_VERSION}" == "m31a ${VERSION}" ]]; then
    pass "Binary --version: ${BINARY_VERSION}"
  else
    fail "Binary version '${BINARY_VERSION}' does not match Cargo.toml '${VERSION}'"
  fi

  if "${BINARY}" --help >/dev/null 2>&1; then
    pass "Binary --help: exit 0"
  else
    fail "Binary --help: non-zero exit"
  fi
else
  echo "  [SKIP] Release binary not found at ${BINARY} (run 'cargo build --release' first)"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "========================================"
echo "Release Check Results: ${PASS} passed, ${FAIL} failed"
echo "========================================"

if [ "${FAIL}" -gt 0 ]; then
  echo "RELEASE BLOCKED: ${FAIL} gate(s) failed." >&2
  exit 1
fi

echo "All gates passed. Ready to tag and publish."
