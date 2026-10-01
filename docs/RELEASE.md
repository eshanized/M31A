# M31A Release Procedure

This document is the canonical release procedure for M31A. Follow it exactly for every public release.

---

## Prerequisites

| Requirement | Minimum Version | Verify |
|:---|:---|:---|
| Rust (stable) | 1.85+ | `rustc --version` |
| Cargo | bundled with Rust | `cargo --version` |
| Git | 2.30+ | `git --version` |
| `sha256sum` | any | `sha256sum --version` |
| `tar` | any | `tar --version` |

Optional (recommended):
- `cargo audit`: `cargo install cargo-audit`
- `cargo deny`: `cargo install cargo-deny`

---

## Step 1: Prepare the Release Commit

### 1.1 Verify working tree is clean

```bash
git status --porcelain
```

Expected: empty output. No modified, staged, or untracked files.

If there are uncommitted changes, review and either commit or stash them before proceeding.

### 1.2 Verify no secrets in working tree

```bash
# Manual scan for common secret patterns
grep -r "nvapi-[A-Za-z0-9_-]\{30\}" --include="*.rs" --include="*.toml" --include="*.json" src/ tests/
grep -r "sk-[A-Za-z0-9_-]\{20,\}" --include="*.rs" --include="*.toml" src/ tests/
```

Expected: no matches (test fixtures with short fake keys are acceptable).

Confirm `.env` and `.m31a/` are in `.gitignore` and not tracked:

```bash
git check-ignore -v .env .m31a/credentials.json
```

### 1.3 Update version in Cargo.toml

Edit `Cargo.toml` and set the `version` field to the release version:

```toml
[package]
version = "X.Y.Z"
```

### 1.4 Update CHANGELOG.md

Add a release section at the top of the `[Unreleased]` block:

```markdown
## [X.Y.Z] — YYYY-MM-DD

### Added
...

### Changed
...

### Fixed
...

### Security
...
```

Move all unreleased entries into the new release section.

Update the comparison links at the bottom:

```markdown
[Unreleased]: https://github.com/eshanized/M31A/compare/vX.Y.Z...HEAD
[X.Y.Z]: https://github.com/eshanized/M31A/compare/vX.Y.Z-1...vX.Y.Z
```

### 1.5 Commit the release

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: release v$(cargo metadata --no-deps --format-version=1 | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["packages"][0]["version"])')"
```

---

## Step 2: Run All Quality Gates

Run in this exact order. Stop and fix any failure before proceeding.

```bash
# 1. Format check
cargo fmt --check

# 2. Compilation check (all targets, all features)
cargo check --all-targets --all-features

# 3. Lint check (must be warning-free)
cargo clippy --all-targets --all-features -- -D warnings

# 4. Full test suite
cargo test --all-targets --all-features

# 5. Security audit (if cargo-audit is installed)
cargo audit
```

All five gates must pass before proceeding.

---

## Step 3: Release Build

```bash
cargo build --release
```

### Verify the release binary

```bash
# Must be an ELF executable on Linux
file target/release/m31a

# Record binary size
ls -lh target/release/m31a

# Check library dependencies (must only link libc, libgcc, libm)
ldd target/release/m31a
```

The binary must NOT link:
- Node.js
- Python
- CUDA / libcudart
- Any development-only service

### Run binary smoke tests

```bash
TARGET_BIN=./target/release/m31a

# Must print: m31a X.Y.Z
$TARGET_BIN --version

# Must print help text and exit 0
$TARGET_BIN --help

# Must print: m31a X.Y.Z
$TARGET_BIN version

# Must run doctor (exit 0 even with partial failures)
$TARGET_BIN doctor

# Must exit 0 with valid configuration report
$TARGET_BIN config validate

# Must exit 0 with JSON doctor output
$TARGET_BIN --output json doctor

# Must list sessions (may be empty)
$TARGET_BIN session list

# Must list missions (may be empty)
$TARGET_BIN mission list
```

---

## Step 4: Package Release Artifacts

Determine the release version:

```bash
VERSION=$(cargo metadata --no-deps --format-version=1 | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["packages"][0]["version"])')
TARGET=x86_64-unknown-linux-gnu
echo "Packaging m31a-${VERSION}-${TARGET}"
```

Create the release archive:

```bash
mkdir -p dist
PKG_DIR="m31a-${VERSION}-${TARGET}"
mkdir -p "dist/${PKG_DIR}"

cp target/release/m31a     "dist/${PKG_DIR}/m31a"
cp README.md               "dist/${PKG_DIR}/README.md"
cp LICENSE-MIT             "dist/${PKG_DIR}/LICENSE-MIT"
cp LICENSE-APACHE          "dist/${PKG_DIR}/LICENSE-APACHE"
cp CHANGELOG.md            "dist/${PKG_DIR}/CHANGELOG.md"

tar -czf "dist/${PKG_DIR}.tar.gz" -C dist "${PKG_DIR}"
rm -rf   "dist/${PKG_DIR}"
```

---

## Step 5: Generate Checksums

```bash
cd dist
sha256sum *.tar.gz > SHA256SUMS
cat SHA256SUMS
cd ..
```

Verify the checksum file:

```bash
cd dist
sha256sum --check SHA256SUMS
cd ..
```

Expected: `<file>.tar.gz: OK` for each artifact.

---

## Step 6: Generate Release Metadata

```bash
VERSION=$(cargo metadata --no-deps --format-version=1 | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["packages"][0]["version"])')
GIT_COMMIT=$(git rev-parse HEAD)
RUSTC_VERSION=$(rustc --version)
CARGO_VERSION=$(cargo --version)
BUILD_TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
BINARY_SHA256=$(sha256sum target/release/m31a | awk '{print $1}')

cat > dist/release.json <<EOF
{
  "version": "${VERSION}",
  "git_commit": "${GIT_COMMIT}",
  "target": "x86_64-unknown-linux-gnu",
  "binary_sha256": "${BINARY_SHA256}",
  "build_timestamp": "${BUILD_TIMESTAMP}",
  "rustc_version": "${RUSTC_VERSION}",
  "cargo_version": "${CARGO_VERSION}"
}
EOF

cat dist/release.json
```

---

## Step 7: Install Smoke Test from Artifact

Test the packaged artifact (not just `target/release/m31a`):

```bash
VERSION=$(cargo metadata --no-deps --format-version=1 | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["packages"][0]["version"])')
TARGET=x86_64-unknown-linux-gnu

TMPDIR=$(mktemp -d)
tar -xzf "dist/m31a-${VERSION}-${TARGET}.tar.gz" -C "${TMPDIR}"

INSTALLED_BIN="${TMPDIR}/m31a-${VERSION}-${TARGET}/m31a"

# Smoke tests on extracted binary
$INSTALLED_BIN --version
$INSTALLED_BIN --help
$INSTALLED_BIN version
$INSTALLED_BIN doctor
$INSTALLED_BIN config validate
$INSTALLED_BIN session list
$INSTALLED_BIN mission list

rm -rf "${TMPDIR}"
echo "Artifact smoke test passed."
```

---

## Step 8: Git Tag and GitHub Release

### 8.1 Verify version consistency

Before tagging, confirm all four sources agree:

```bash
CARGO_VER=$(cargo metadata --no-deps --format-version=1 | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["packages"][0]["version"])')
BINARY_VER=$(./target/release/m31a --version | awk '{print $2}')

echo "Cargo.toml version:  ${CARGO_VER}"
echo "Binary --version:    ${BINARY_VER}"
echo "CHANGELOG entry:     (verify manually)"
echo "Planned Git tag:     v${CARGO_VER}"

if [ "${CARGO_VER}" != "${BINARY_VER}" ]; then
  echo "ERROR: version mismatch between Cargo.toml and binary!"
  exit 1
fi
echo "Version consistency check: PASS"
```

### 8.2 Create Git tag

```bash
VERSION=$(cargo metadata --no-deps --format-version=1 | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["packages"][0]["version"])')
git tag -a "v${VERSION}" -m "Release v${VERSION}"
```

### 8.3 Push tag

```bash
git push origin "v${VERSION}"
```

### 8.4 Create GitHub release

Upload to GitHub Releases:
- Tag: `vX.Y.Z`
- Title: `M31A vX.Y.Z`
- Body: contents of the `[X.Y.Z]` section from `CHANGELOG.md`
- Assets:
  - `m31a-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
  - `SHA256SUMS`
  - `release.json`

**Do NOT publish until all previous gates have passed.**

---

## Step 9: Post-Release Verification

After the GitHub release is published:

```bash
# Download and verify from the published release URL
RELEASE_URL="https://github.com/eshanized/M31A/releases/download/vX.Y.Z/m31a-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz"
wget "$RELEASE_URL" -O /tmp/m31a-release.tar.gz

# Verify checksum matches
EXPECTED_SHA=$(grep "m31a-X.Y.Z-x86_64" dist/SHA256SUMS | awk '{print $1}')
ACTUAL_SHA=$(sha256sum /tmp/m31a-release.tar.gz | awk '{print $1}')

if [ "$EXPECTED_SHA" != "$ACTUAL_SHA" ]; then
  echo "ERROR: checksum mismatch in published artifact!"
  exit 1
fi
echo "Post-release checksum: PASS"
```

---

## Rollback Procedure

If a critical issue is discovered post-release:

1. **Do not delete the Git tag** (it creates confusion and breaks `cargo install --tag` users)
2. Prepare a hotfix branch from the release tag:
   ```bash
   git checkout -b hotfix/vX.Y.Z+1 vX.Y.Z
   ```
3. Apply the minimal fix
4. Bump version to `X.Y.Z+1` (patch increment)
5. Follow this full procedure for the hotfix release

---

## Phase 45: Release Candidate Procedure (RC)

This section extends the procedure above for Release Candidates. It is
normative for every RC; the Phase 45 evidence bundle proves each step.

### RC.1 Assemble (`scripts/build-release.sh`)

```bash
./scripts/build-release.sh [TARGET]   # default: x86_64-unknown-linux-gnu
```

Produces in `dist/`: versioned tarball (deterministic assembly: sorted
entries, mtime clamped to the source commit timestamp, `gzip -n`),
`SHA256SUMS` (tarball + SBOM), `release.json`, `sbom.json` (resolved from
`Cargo.lock`). The build records `source_dirty=true` in provenance when the
tree is uncommitted — RCs should be assembled from clean trees.

### RC.2 Verify (`scripts/verify-release.sh`)

```bash
./scripts/verify-release.sh [DIST_DIR] [VERSION] [TARGET]
```

Fails closed on: missing/unexpected artifacts, checksum mismatch (tamper),
binary `--version` mismatch, `release.json` disagreement, invalid/empty SBOM,
hygiene findings (temp files, secret patterns). Exit non-zero blocks promotion.

### RC.3 Evidence (`scripts/rc-evidence.sh`)

```bash
./scripts/rc-evidence.sh [TARGET]     # requires API_KEY_NVIDIA for LIVE probes
```

Runs the full gate matrix with real command output captured into
`dist/RC-EVIDENCE.md` and assembles `dist/rc-manifest.json` (schema v1,
validated by `ReleaseManifest` in `src/release/`). Without
`API_KEY_NVIDIA`, live probes are recorded as `CREDENTIAL-BLOCKED` (formally
adopted classification, non-blocking when deterministic coverage exists).

### RC.4 Layout contract

Expected `dist/` top-level members: the versioned tarball, `SHA256SUMS`,
`release.json`, `sbom.json`, `rc-manifest.json`, `RC-EVIDENCE.md`. Anything
else fails verification as an unexpected artifact.

### RC.5 Supported upgrade

Supported: re-running a newer binary against an existing `.m31a/m31a.db`.
Migrations apply pending versions only, preserve all rows, and record
`_sqlx_migrations`. Verified by `p45_migration_upgrade_applies_pending_024`
and `p45_upgrade_preserves_durable_state` (missions, authorizations, budget,
fingerprints, telemetry intact; authorization revalidation passes).

### RC.6 Unsupported upgrade / corrupt installation

- **Downgrade is unsupported**: no `*.down.sql` migrations exist by design.
  Restoring an older binary over a newer schema fails explicitly at migration
  time (never best-effort).
- **Corrupt database**: bootstrap fails closed (`p45_migration_invalid_state_fails_closed`).
- **Supported recovery**: file-level backup restoration — checkpoint the WAL
  (`PRAGMA wal_checkpoint(TRUNCATE)`), copy `m31a.db`, restore by copy-back
  (`p45_backup_restore_round_trip`). Restart revalidates authorization before
  resuming (Phase 44 invariant, intact).

### RC.7 Blocker taxonomy (single, closed)

`RESOLVED` · `FORMALLY CONTAINED` · `ACCEPTABLE FOR RELEASE` ·
`POST-RELEASE` · `SECURITY-BLOCKED` · `ENVIRONMENT-BLOCKED` ·
`CREDENTIAL-BLOCKED` (formally adopted per §12 Option A: credential/cost-gated
live coverage, non-blocking only with deterministic coverage + explicit
record). Regression-anchored by `taxonomy_is_closed_and_documented` in
`src/release/status.rs`. Historical Phase 43/44 reports are left untouched.

### RC.8 Supported platforms

Executed and verified: `x86_64-unknown-linux-gnu` (this environment).
Any other target is unexecuted until proven — never called "verified".
Cross targets may be *built* but ship only with explicit per-target execution
evidence.

### RC.9 Known limitations (explicit)

- No cryptographic signing infrastructure exists (`SIGNING_MECHANISM: none`);
  provenance is factual metadata, verified for consistency, not signatures.
- `Cargo.lock` carries no license texts; the SBOM states this and claims none.
- Byte-identical compilation is NOT claimed (rustc path embedding, LTO);
  reproducibility covers packaging determinism + metadata equivalence.
- Full live-mission restart mid-flight remains credential-gated by cost
  policy; deterministic fault injection covers the invariants.
- Host `/tmp` tmpfs pressure: use `TMPDIR=<repo>/tmp` (≥1G writable scratch).

---

## Reference

| File | Purpose |
|:---|:---|
| `Cargo.toml` | Authoritative version source |
| `CHANGELOG.md` | Human-readable release notes |
| `dist/SHA256SUMS` | Artifact integrity verification |
| `dist/release.json` | Machine-readable release metadata |
| `dist/sbom.json` | Dependency SBOM from `Cargo.lock` |
| `dist/rc-manifest.json` | Machine-readable RC manifest (schema v1) |
| `dist/RC-EVIDENCE.md` | Human-readable RC evidence record |
| `docs/RELEASE.md` | Authoritative release procedure and quality gates |
