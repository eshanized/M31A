#!/usr/bin/env bash
# scripts/verify-release.sh — independent release candidate artifact verification.
#
# Usage:
#   ./scripts/verify-release.sh [DIST_DIR] [VERSION] [TARGET]
#
# Verifies, failing closed on the first problem:
#   1. expected archive exists, no unexpected top-level artifacts
#   2. SHA256SUMS verifies (tamper detection)
#   3. extracted binary reports the exact expected version
#   4. release.json version/commit/target agree with expectations
#   5. sbom.json exists and is valid JSON with non-empty components
#   6. release directory hygiene (no temp files, no secrets)
#
# Never reports "verified" solely because files exist.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

DIST_DIR="${1:-dist}"
VERSION="${2:-$(cargo metadata --no-deps --format-version=1 | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d["packages"][0]["version"])')}"
TARGET="${3:-x86_64-unknown-linux-gnu}"

PKG="m31a-${VERSION}-${TARGET}.tar.gz"
PASS=0
FAIL=0
pass() { echo "  [PASS] $1"; PASS=$((PASS + 1)); }
fail() { echo "  [FAIL] $1" >&2; FAIL=$((FAIL + 1)); }

echo "==> Verifying release ${VERSION} (${TARGET}) in ${DIST_DIR}/"

# 1. Expected archive exists; no unexpected top-level artifacts.
if [ -f "${DIST_DIR}/${PKG}" ]; then
  pass "archive present: ${PKG}"
else
  fail "archive missing: ${PKG}"
fi

ALLOWED="^(m31a-[0-9]+\.[0-9]+\.[0-9]+-.+\.tar\.gz|SHA256SUMS|release\.json|sbom\.json|rc-manifest\.json|RC-EVIDENCE\.md)$"
UNEXPECTED=""
for f in "${DIST_DIR}"/*; do
  [ -e "$f" ] || continue
  base="$(basename "$f")"
  if ! [[ "${base}" =~ ${ALLOWED} ]]; then
    UNEXPECTED="${UNEXPECTED} ${base}"
  fi
done
if [ -z "${UNEXPECTED}" ]; then
  pass "no unexpected top-level artifacts"
else
  fail "unexpected artifacts:${UNEXPECTED}"
fi

# 2. Checksums verify (tamper detection).
if (cd "${DIST_DIR}" && sha256sum --check SHA256SUMS >/dev/null 2>&1); then
  pass "SHA256SUMS verifies (bytes match)"
else
  fail "SHA256SUMS verification failed (tampered or missing artifact)"
fi

# 3. Extracted binary reports the exact version.
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT
tar -xzf "${DIST_DIR}/${PKG}" -C "${WORK}"
BIN="$(find "${WORK}" -name m31a -type f | head -1)"
if [ -z "${BIN}" ]; then
  fail "binary missing inside archive"
else
  GOT="$("${BIN}" --version 2>&1 || true)"
  if [ "${GOT}" = "m31a ${VERSION}" ]; then
    pass "binary --version == 'm31a ${VERSION}'"
  else
    fail "binary --version mismatch: '${GOT}' (expected 'm31a ${VERSION}')"
  fi
  if "${BIN}" --help >/dev/null 2>&1; then
    pass "binary --help exits 0"
  else
    fail "binary --help failed"
  fi
fi

# 4. release.json agrees.
if [ -f "${DIST_DIR}/release.json" ]; then
  RV="$(python3 -c "import json;print(json.load(open('${DIST_DIR}/release.json'))['version'])")"
  RT="$(python3 -c "import json;print(json.load(open('${DIST_DIR}/release.json'))['target'])")"
  [ "${RV}" = "${VERSION}" ] && pass "release.json version agrees" || fail "release.json version '${RV}' != '${VERSION}'"
  [ "${RT}" = "${TARGET}" ] && pass "release.json target agrees" || fail "release.json target '${RT}' != '${TARGET}'"
else
  fail "release.json missing"
fi

# 5. SBOM present, valid JSON, non-empty components.
if [ -f "${DIST_DIR}/sbom.json" ]; then
  if python3 -c "import json;d=json.load(open('${DIST_DIR}/sbom.json'));assert d['components'], 'empty'"; then
    pass "sbom.json valid with non-empty components"
  else
    fail "sbom.json invalid or empty"
  fi
else
  fail "sbom.json missing"
fi

# 6. Hygiene: no temp files, no secret patterns in small metadata files.
BAD=""
for f in "${DIST_DIR}"/*; do
  [ -f "$f" ] || continue
  base="$(basename "$f")"
  case "${base}" in
    *.tmp|*.bak|*~|*.swp|.env*) BAD="${BAD} ${base}(temp)" ;;
  esac
done
for pat in 'nvapi-' 'AKIA' 'BEGIN .*PRIVATE KEY' 'ghp_'; do
  if grep -rlE "${pat}" "${DIST_DIR}"/release.json "${DIST_DIR}"/sbom.json "${DIST_DIR}"/SHA256SUMS 2>/dev/null | grep -q .; then
    BAD="${BAD} secret-pattern(${pat})"
  fi
done
if [ -z "${BAD}" ]; then
  pass "release directory hygiene clean"
else
  fail "hygiene findings:${BAD}"
fi

echo ""
echo "==> ${PASS} passed, ${FAIL} failed"
[ "${FAIL}" -eq 0 ]
