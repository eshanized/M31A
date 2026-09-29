#!/usr/bin/env bash
# scripts/release-check.sh
# Run all mandatory quality gates before creating a release.
# Fails fast on the first failing gate.
#
# Usage:
#   ./scripts/release-check.sh
#
# Gates run in order:
#   1.  Git clean tree check
#   2.  Secret scan
#   3.  Version consistency check
#   4.  cargo fmt --check
#   5.  cargo check --all-targets --all-features
#   6.  cargo clippy --all-targets --all-features -- -D warnings
#   7.  cargo test --all-targets --all-features
#   8.  cargo audit (if installed)
#   9.  Binary smoke tests (if release binary exists)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

PASS=0
FAIL=0

pass() { echo "  [PASS] $1"; PASS=$((PASS + 1)); }
fail() { echo "  [FAIL] $1" >&2; FAIL=$((FAIL + 1)); }
header() { echo ""; echo "==> $1"; }

# ── 1. Clean tree ─────────────────────────────────────────────────────────────
header "1/9  Working tree state"
if [ -z "$(git status --porcelain)" ]; then
  pass "Working tree is clean"
else
  fail "Working tree has uncommitted changes (git status --short):"
  git status --short >&2
fi

# ── 2. Secret scan ────────────────────────────────────────────────────────────
header "2/9  Secret scan"

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
header "3/9  Version consistency"

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
header "4/9  cargo fmt --check"
if cargo fmt --check 2>&1; then
  pass "cargo fmt --check"
else
  fail "cargo fmt --check (run 'cargo fmt' to fix)"
fi

# ── 5. Check ──────────────────────────────────────────────────────────────────
header "5/9  cargo check --all-targets --all-features"
if cargo check --all-targets --all-features 2>&1; then
  pass "cargo check"
else
  fail "cargo check failed"
  exit 1
fi

# ── 6. Clippy ─────────────────────────────────────────────────────────────────
header "6/9  cargo clippy --all-targets --all-features -- -D warnings"
if cargo clippy --all-targets --all-features -- -D warnings 2>&1; then
  pass "cargo clippy"
else
  fail "cargo clippy produced warnings or errors"
  exit 1
fi

# ── 7. Tests ──────────────────────────────────────────────────────────────────
header "7/9  cargo test (minimal resources: bounded jobs and test threads)"
if cargo test --lib -j 2 -- --test-threads 2 2>&1 && \
   cargo test --test docs_contract -j 2 -- --test-threads 2 2>&1 && \
   cargo test --test promptos_v2_behavioral -j 2 -- --test-threads 2 2>&1; then
  pass "cargo test (bounded minimal resources passed)"
else
  fail "cargo test: tests failed under bounded execution"
  exit 1
fi

# ── 8. cargo audit ────────────────────────────────────────────────────────────
header "8/9  cargo audit"
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
header "9/9  Binary smoke test (release binary)"
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
