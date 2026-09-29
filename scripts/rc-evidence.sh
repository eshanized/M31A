#!/usr/bin/env bash
# scripts/rc-evidence.sh — assemble the canonical release candidate evidence bundle.
#
# Usage:
#   ./scripts/rc-evidence.sh [TARGET]
#
# Runs the release gate matrix, captures REAL command output (no manual
# counts), and writes:
#   dist/rc-manifest.json ... machine-readable RC manifest
#   dist/RC-EVIDENCE.md ..... human-readable evidence record
#
# Fails closed: any gate failure aborts the bundle (no partial evidence
# presented as complete). Live probes run only when API_KEY_NVIDIA is set;
# otherwise they are recorded as CREDENTIAL-BLOCKED, never faked.
#
# Evidence classification per gate: DETERMINISTIC / LIVE / CREDENTIAL-BLOCKED.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

TARGET="${1:-x86_64-unknown-linux-gnu}"
export TMPDIR="${REPO_ROOT}/tmp"
mkdir -p "${TMPDIR}" dist

VERSION=$(cargo metadata --no-deps --format-version=1 \
  | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d["packages"][0]["version"])')
GIT_COMMIT=$(git rev-parse HEAD)
GIT_DIRTY="false"
[ -n "$(git status --porcelain)" ] && GIT_DIRTY="true"
RUSTC_VERSION=$(rustc --version)
CARGO_VERSION=$(cargo --version)
STAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)

OUT="dist/RC-EVIDENCE.md"
{
  echo "# M31A RC Evidence — v${VERSION}"
  echo ""
  echo "- source revision: ${GIT_COMMIT} (dirty=${GIT_DIRTY})"
  echo "- target: ${TARGET}"
  echo "- toolchain: ${RUSTC_VERSION} / ${CARGO_VERSION}"
  echo "- assembled: ${STAMP}"
  echo ""
} > "${OUT}"

record() { echo "- $1" >> "${OUT}"; echo "  $1"; }

# ── Deterministic gates ───────────────────────────────────────────────────────
cargo fmt --check >>"${OUT}" 2>&1 && record "DETERMINISTIC fmt: PASS" || { record "DETERMINISTIC fmt: FAIL"; exit 1; }
cargo check --all-targets >>"${OUT}" 2>&1 && record "DETERMINISTIC check: PASS" || { record "DETERMINISTIC check: FAIL"; exit 1; }
cargo clippy --all-targets --all-features -- -D warnings >>"${OUT}" 2>&1 && record "DETERMINISTIC clippy: PASS" || { record "DETERMINISTIC clippy: FAIL"; exit 1; }

TEST_OUT="$(mktemp)"
# Deterministic gates must run WITHOUT provider credentials: several tests
# assert fail-closed behavior when keys are absent (e.g.
# test_condition_a_missing_credentials). Credentials are scoped to the live
# section only.
env -u API_KEY_NVIDIA -u NVIDIA_API_KEY \
cargo test --all-targets -j 2 -- --test-threads 2 >"${TEST_OUT}" 2>&1 || { tail -30 "${TEST_OUT}"; record "DETERMINISTIC tests: FAIL"; exit 1; }
SUMMARY=$(grep -E "test result:" "${TEST_OUT}" | python3 -c "
import sys,re
p=f=i=0
for l in sys.stdin:
    m=re.search(r'(\d+) passed; (\d+) failed; (\d+) ignored',l)
    if m: p+=int(m.group(1)); f+=int(m.group(2)); i+=int(m.group(3))
print(f'{p} passed; {f} failed; {i} ignored')")
record "DETERMINISTIC cargo test --all-targets: ${SUMMARY}"
echo "" >> "${OUT}"
echo "<details><summary>full test output tail</summary>" >> "${OUT}"
echo "" >> "${OUT}"
tail -5 "${TEST_OUT}" >> "${OUT}"
echo "</details>" >> "${OUT}"

# ── Migration ledger ──────────────────────────────────────────────────────────
MIGRATIONS=$(ls migrations/*.sql | wc -l | tr -d ' ')
record "migrations present: ${MIGRATIONS} files"

# ── Live probes (credential-gated) ────────────────────────────────────────────
if [ -n "${API_KEY_NVIDIA:-}" ]; then
  for t in phase_42_production_proof phase_43_reliability phase_44_security_closure phase_45_release_validation; do
    if cargo test --test "$t" -- --ignored --test-threads=1 >>"${OUT}" 2>&1; then
      record "LIVE $t: PASS"
    else
      record "LIVE $t: FAIL"; exit 1
    fi
  done
else
  record "LIVE probes: CREDENTIAL-BLOCKED (API_KEY_NVIDIA unset; deterministic coverage retained)"
fi

# ── Release artifacts ─────────────────────────────────────────────────────────
for f in "m31a-${VERSION}-${TARGET}.tar.gz" SHA256SUMS release.json sbom.json; do
  if [ -f "dist/$f" ]; then record "artifact present: dist/$f"; else record "artifact MISSING: dist/$f"; exit 1; fi
done

# ── rc-manifest.json ──────────────────────────────────────────────────────────
python3 - "$VERSION" "$GIT_COMMIT" "$TARGET" "$SUMMARY" <<'PYEOF'
import sys, json, hashlib, os
version, commit, target, summary = sys.argv[1:5]
dist = "dist"
def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()
names = sorted(["m31a-%s-%s.tar.gz" % (version, target), "SHA256SUMS", "release.json", "sbom.json"])
artifacts = []
for n in names:
    p = os.path.join(dist, n)
    artifacts.append({"name": n, "sha256": sha(p), "size_bytes": os.path.getsize(p), "kind": "release"})
manifest = {
    "schema_version": 1,
    "project": "m31a",
    "version": version,
    "source_revision": commit,
    "build_target": target,
    "toolchain_rustc": os.popen("rustc --version").read().strip(),
    "toolchain_cargo": os.popen("cargo --version").read().strip(),
    "build_timestamp": os.popen("date -u +%Y-%m-%dT%H:%M:%SZ").read().strip(),
    "reproducible_build": False,
    "artifacts": artifacts,
    "sbom_file": "sbom.json",
    "sbom_sha256": sha(os.path.join(dist, "sbom.json")),
    "provenance_file": "release.json",
    "migration_version": 24,
    "release_status": "candidate",
    "test_summary": summary,
}
with open(os.path.join(dist, "rc-manifest.json"), "w") as f:
    json.dump(manifest, f, indent=2)
    f.write("\n")
print("rc-manifest.json written with %d artifacts" % len(artifacts))
PYEOF

record "rc-manifest.json assembled"
echo ""
echo "==> RC evidence bundle complete in dist/"
