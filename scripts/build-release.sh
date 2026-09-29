#!/usr/bin/env bash
# scripts/build-release.sh
# Build a release binary and produce a distributable archive.
#
# Usage:
#   ./scripts/build-release.sh [TARGET]
#
# TARGET defaults to x86_64-unknown-linux-gnu.
# Artifacts are written to dist/.
#
# Requirements:
#   - Rust stable toolchain (rustc --version must show 1.85+)
#   - sha256sum, tar (standard Linux utilities)
#   - python3 (for cargo metadata parsing)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

TARGET="${1:-x86_64-unknown-linux-gnu}"

# ── Prerequisites ─────────────────────────────────────────────────────────────
echo "==> Verifying prerequisites..."

if ! command -v cargo >/dev/null 2>&1; then
  echo "ERROR: 'cargo' not found. Install Rust from https://rustup.rs" >&2
  exit 1
fi
if ! command -v sha256sum >/dev/null 2>&1; then
  echo "ERROR: 'sha256sum' not found." >&2
  exit 1
fi

# ── Determine version ─────────────────────────────────────────────────────────
VERSION=$(cargo metadata --no-deps --format-version=1 \
  | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d["packages"][0]["version"])')
echo "==> Building m31a v${VERSION} for ${TARGET}"

# ── Clean git tree check ───────────────────────────────────────────────────────
if [ -n "$(git status --porcelain)" ]; then
  echo "WARNING: Working tree is not clean. Release builds should use a clean tree." >&2
  git status --short >&2
fi

# ── Build ─────────────────────────────────────────────────────────────────────
echo "==> Running: cargo build --release"
cargo build --release

BINARY="target/release/m31a"

if [ ! -f "${BINARY}" ]; then
  echo "ERROR: Release binary not found at ${BINARY}" >&2
  exit 1
fi

echo "==> Binary: $(ls -lh ${BINARY})"

# ── Quick smoke test ──────────────────────────────────────────────────────────
echo "==> Smoke testing binary..."
BINARY_VERSION=$("${BINARY}" --version 2>&1 || true)
echo "    ${BINARY_VERSION}"
if [[ "${BINARY_VERSION}" != "m31a ${VERSION}" ]]; then
  echo "ERROR: Binary version '${BINARY_VERSION}' does not match Cargo.toml '${VERSION}'" >&2
  exit 1
fi
echo "    Version consistency: PASS"

# ── Package ───────────────────────────────────────────────────────────────────
echo "==> Packaging..."
mkdir -p dist
PKG_DIR="m31a-${VERSION}-${TARGET}"
PKG_TMP="${REPO_ROOT}/dist/${PKG_DIR}"
rm -rf "${PKG_TMP}"
mkdir -p "${PKG_TMP}"

cp "${BINARY}"              "${PKG_TMP}/m31a"
cp README.md                "${PKG_TMP}/README.md"
cp CHANGELOG.md             "${PKG_TMP}/CHANGELOG.md"
[ -f LICENSE-MIT    ] && cp LICENSE-MIT    "${PKG_TMP}/LICENSE-MIT"
[ -f LICENSE-APACHE ] && cp LICENSE-APACHE "${PKG_TMP}/LICENSE-APACHE"

# Deterministic archive assembly: Python tarfile with sorted
# entries, mtime clamped to the source commit timestamp (source-derived,
# stable per revision), uid/gid 0, and gzip mtime=0. The host `tar` on some
# systems lacks --sort-name despite its version string, so assembly does not
# depend on it. Rebuilding the same revision reproduces equivalent packaging
# bytes (compiler-level byte identity across environments is not claimed; see docs/RELEASE.md).
EPOCH="$(git log -1 --format=%ct)"
python3 - "${PKG_TMP}" "${PKG_DIR}" "${EPOCH}" <<'PYEOF'
import sys, tarfile, gzip, os
pkg_tmp, pkg_dir, epoch = sys.argv[1:4]
mtime = int(epoch)
tar_path = pkg_tmp + ".tar"
names = sorted(os.listdir(pkg_tmp))
with tarfile.open(tar_path, "w", format=tarfile.PAX_FORMAT) as tar:
    for name in names:
        full = os.path.join(pkg_tmp, name)
        arc = os.path.join(pkg_dir, name)
        info = tar.gettarinfo(full, arcname=arc)
        info.uid = 0
        info.gid = 0
        info.uname = ""
        info.gname = ""
        info.mtime = mtime
        if info.isfile():
            with open(full, "rb") as f:
                tar.addfile(info, f)
        else:
            tar.addfile(info)
with open(tar_path, "rb") as f_in, open(pkg_tmp + ".tar.gz", "wb") as f_out_raw:
    with gzip.GzipFile(filename="", mode="wb", compresslevel=9, mtime=0, fileobj=f_out_raw) as gz:
        while True:
            chunk = f_in.read(65536)
            if not chunk:
                break
            gz.write(chunk)
os.remove(tar_path)
print("    deterministic entries: %d (mtime=%s)" % (len(names), epoch))
PYEOF
# The archive is already at dist/${PKG_DIR}.tar.gz (PKG_TMP is under dist/).
rm -rf "${PKG_TMP}"

echo "==> Archive: dist/${PKG_DIR}.tar.gz"

# ── SBOM (from the exact resolved graph) ────────────────────────────────────
echo "==> Generating SBOM from Cargo.lock..."
python3 - <<'PYEOF'
import json, re

lock = open("Cargo.lock").read()
blocks = re.split(r"\n\[\[package\]\]\n", lock)
components = []
for block in blocks[1:]:
    name = re.search(r'^name = "([^"]+)"', block, re.M).group(1)
    version = re.search(r'^version = "([^"]+)"', block, re.M).group(1)
    source = re.search(r'^source = "([^"]+)"', block, re.M)
    checksum = re.search(r'^checksum = "([^"]+)"', block, re.M)
    components.append({
        "name": name,
        "version": version,
        "source": source.group(1) if source else None,
        "checksum": checksum.group(1) if checksum else None,
    })
components.sort(key=lambda c: (c["name"], c["version"], c["source"] or ""))
assert components, "Cargo.lock contains zero packages"
sbom = {
    "bomFormat": "CycloneDX",
    "specVersion": "1.6",
    "version": 1,
    "metadata_tool": "m31a-build-release.sh",
    "licenses_note": "Cargo.lock provides no license data; licenses are not claimed here. See individual crate registries.",
    "components": components,
}
with open("dist/sbom.json", "w") as f:
    json.dump(sbom, f, indent=2)
    f.write("\n")
print(f"    SBOM components: {len(components)}")
PYEOF
echo "    SBOM: dist/sbom.json"

# ── Checksums ─────────────────────────────────────────────────────────────────
echo "==> Generating checksums..."
cd dist
sha256sum "${PKG_DIR}.tar.gz" sbom.json > SHA256SUMS
cat SHA256SUMS
sha256sum --check SHA256SUMS >/dev/null
echo "    Checksum verification: PASS"
cd "${REPO_ROOT}"

# ── Release metadata ──────────────────────────────────────────────────────────
echo "==> Generating release.json..."
GIT_COMMIT=$(git rev-parse HEAD)
RUSTC_VERSION=$(rustc --version)
CARGO_VERSION=$(cargo --version)
BUILD_TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
BINARY_SHA256=$(sha256sum "${BINARY}" | awk '{print $1}')

cat > dist/release.json <<EOF
{
  "version": "${VERSION}",
  "git_commit": "${GIT_COMMIT}",
  "target": "${TARGET}",
  "binary_sha256": "${BINARY_SHA256}",
  "build_timestamp": "${BUILD_TIMESTAMP}",
  "rustc_version": "${RUSTC_VERSION}",
  "cargo_version": "${CARGO_VERSION}"
}
EOF

echo ""
echo "==> Release artifacts ready in dist/:"
ls -lh dist/

echo ""
echo "==> Release build complete: m31a v${VERSION}"
echo "    Next steps: run scripts/release-check.sh to run all quality gates."
