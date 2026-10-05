#!/usr/bin/env bash
# scripts/build-release.sh
# Build a channel-aware release binary and produce distributable packages.
#
# Usage:
#   ./scripts/build-release.sh [--channel development|production]
#                              [--format FORMAT[,...]|all] [TARGET]
#
# TARGET is a Rust target triple (default: host triple). When TARGET differs
# from the host, `cargo build --release --target TARGET` is used and the
# binary is taken from target/<TARGET>/release/.
#
# FORMAT selects distributable package formats (default: tar.gz on Unix,
# zip on Windows targets). Repeatable / comma-separated; `all` selects every
# format applicable to the HOST operating system:
#   tar.gz   - deterministic gzip tarball (all Unix hosts)
#   zip      - deterministic zip archive (any host; Windows layout)
#   deb      - Debian package via cargo-deb (Linux hosts)
#   rpm      - RPM package via cargo-generate-rpm (Linux hosts)
#   appimage - AppImage via appimagetool (Linux hosts)
#   dmg      - macOS disk image via hdiutil (macOS hosts only)
#   msi      - Windows installer via cargo-wix (Windows hosts only)
#
# CHANNEL defaults to production. The resolved channel is always echoed,
# recorded in metadata, and verified against the built binary — production is
# never silently inferred: a dirty tree fails a production build instead of
# reporting clean provenance. Artifacts are written to dist/.
#
# Asset naming (short platform names, shared with CI and installers):
#   m31a-<VERSION>-linux-x64.tar.gz   m31a-<VERSION>-linux-arm64.tar.gz
#   m31a-<VERSION>-darwin-x64.tar.gz  m31a-<VERSION>-darwin-arm64.tar.gz
#   m31a-<VERSION>-windows-x64.zip    m31a-<VERSION>-windows-arm64.zip
#   m31a_<VERSION>_amd64.deb          m31a-<VERSION>-<rel>.x86_64.rpm
#   m31a-<VERSION>-linux-x64.AppImage m31a-<VERSION>-darwin-arm64.dmg
#   m31a-<VERSION>-windows-x64.msi
#
# Requirements:
#   - Rust stable toolchain (rustc --version must show 1.85+)
#   - sha256sum (or shasum), python3, git
#   - per format: cargo-deb (deb), cargo-generate-rpm (rpm),
#     appimagetool (appimage), hdiutil (dmg, macOS only),
#     cargo-wix + WiX (msi, Windows only), 7z or python3 zipfile (zip)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

CHANNEL="production"
TARGET=""
# Space-separated format list (plain string, not an array: macOS ships Bash
# 3.2, where expanding an empty array under `set -u` is a fatal error).
FORMATS=""
# Append a format unless already present (dedup, order-preserving).
add_format() {
  case " $FORMATS " in
    *" $1 "*) ;;
    *) FORMATS="$FORMATS $1" ;;
  esac
}
# Split a comma-separated --format value into the list.
add_csv_formats() {
  _csv="$1,"
  while [ -n "$_csv" ]; do
    _one="${_csv%%,*}"
    _csv="${_csv#*,}"
    if [ -n "$_one" ]; then
      add_format "$_one"
    fi
  done
}
while [ $# -gt 0 ]; do
  case "$1" in
    --channel=*) CHANNEL="${1#--channel=}"; shift ;;
    --channel)
      shift
      if [ $# -eq 0 ]; then echo "ERROR: --channel requires development|production" >&2; exit 1; fi
      CHANNEL="$1"; shift ;;
    --format=*)
      add_csv_formats "${1#--format=}"; shift ;;
    --format)
      shift
      if [ $# -eq 0 ]; then echo "ERROR: --format requires a value" >&2; exit 1; fi
      add_csv_formats "$1"; shift ;;
    development|production) CHANNEL="$1"; shift ;;
    -h|--help) sed -n '2,42p' "$0"; exit 0 ;;
    *) TARGET="$1"; shift ;;
  esac
done

case "$CHANNEL" in
  development|production) ;;
  *) echo "ERROR: unknown channel '$CHANNEL' (expected development|production)" >&2; exit 1 ;;
esac

HOST_OS="$(uname -s)"
case "$HOST_OS" in
  Linux) HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')" ;;
  Darwin) HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')" ;;
  *) HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')" ;;
esac
if [ -z "$TARGET" ]; then
  TARGET="$HOST_TRIPLE"
fi

# Cross-architecture guard: a binary built for a different CPU architecture
# cannot be EXECUTED on this host (e.g. ARM64 Windows binaries on x64
# runners — there is no emulation layer). Presence, size, and package
# contents are still verified; only execution-based identity checks are
# skipped. Never silently skipped on matching arches.
TARGET_ARCH="${TARGET%%-*}"
HOST_ARCH="${HOST_TRIPLE%%-*}"
CAN_EXEC="1"
if [ "$TARGET_ARCH" != "$HOST_ARCH" ]; then
  CAN_EXEC="0"
  echo "    NOTE: cross-arch target (${TARGET}) on ${HOST_TRIPLE} host: execution-based validation will be skipped (presence/content checks still enforced)"
fi

# Short platform name shared with CI matrix and standalone installers.
short_platform() {
  case "$1" in
    x86_64-unknown-linux-gnu) echo "linux-x64" ;;
    aarch64-unknown-linux-gnu) echo "linux-arm64" ;;
    x86_64-apple-darwin) echo "darwin-x64" ;;
    aarch64-apple-darwin) echo "darwin-arm64" ;;
    x86_64-pc-windows-msvc) echo "windows-x64" ;;
    aarch64-pc-windows-msvc) echo "windows-arm64" ;;
    *) echo "$1" ;;
  esac
}
PLATFORM="$(short_platform "$TARGET")"
EXE_SUFFIX=""
case "$TARGET" in
  *-pc-windows-*) EXE_SUFFIX=".exe" ;;
esac

if [ "$CHANNEL" = "development" ]; then
  BIN_NAME="m31a-dev"
  FEATURES="--features development"
else
  BIN_NAME="m31a"
  FEATURES=""
fi

# Default format: tarball on Unix targets, zip on Windows targets.
if [ -z "$FORMATS" ]; then
  if [ -n "$EXE_SUFFIX" ]; then
    FORMATS="zip"
  else
    FORMATS="tar.gz"
  fi
fi
# Expand `all` to every format applicable to the HOST os.
EXPANDED=""
# shellcheck disable=SC2086
for _f in $FORMATS; do
  if [ "$_f" = "all" ]; then
    case "$HOST_OS" in
      Linux) EXPANDED="$EXPANDED tar.gz deb rpm appimage" ;;
      Darwin) EXPANDED="$EXPANDED tar.gz dmg" ;;
      *) EXPANDED="$EXPANDED zip msi" ;;
    esac
  else
    EXPANDED="$EXPANDED $_f"
  fi
done
# Deduplicate while preserving order.
FORMATS=""
# shellcheck disable=SC2086
for _f in $EXPANDED; do
  add_format "$_f"
done
# shellcheck disable=SC2086
for _f in $FORMATS; do
  case "$_f" in
    tar.gz|zip|deb|rpm|appimage|dmg|msi) ;;
    *) echo "ERROR: unknown --format '$_f' (expected tar.gz|zip|deb|rpm|appimage|dmg|msi|all)" >&2; exit 1 ;;
  esac
done
case "$HOST_OS" in
  Darwin) ;;
  # shellcheck disable=SC2086
  *) for _f in $FORMATS; do
       if [ "$_f" = "dmg" ]; then echo "ERROR: --format dmg requires a macOS host (hdiutil)" >&2; exit 1; fi
     done ;;
esac
case "$HOST_OS" in
  Linux) ;;
  # shellcheck disable=SC2086
  *) for _f in $FORMATS; do
       if [ "$_f" = "deb" ] || [ "$_f" = "rpm" ] || [ "$_f" = "appimage" ]; then
         echo "ERROR: --format $_f requires a Linux host" >&2; exit 1
       fi
     done ;;
esac
case "$HOST_OS" in
  *MINGW*|*MSYS*|*CYGWIN*|Windows_NT) ;;
  # shellcheck disable=SC2086
  *) for _f in $FORMATS; do
       if [ "$_f" = "msi" ]; then echo "ERROR: --format msi requires a Windows host (cargo-wix + WiX)" >&2; exit 1; fi
     done ;;
esac

# ── Prerequisites ─────────────────────────────────────────────────────────────
echo "==> Verifying prerequisites..."

if ! command -v cargo >/dev/null 2>&1; then
  echo "ERROR: 'cargo' not found. Install Rust from https://rustup.rs" >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: 'python3' not found." >&2
  exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
  SHA256SUM=sha256sum
elif command -v shasum >/dev/null 2>&1; then
  SHA256SUM="shasum -a 256"
else
  echo "ERROR: neither 'sha256sum' nor 'shasum' found." >&2
  exit 1
fi

# ── Determine version (canonical authority: Cargo.toml) ───────────────────────
VERSION=$(cargo metadata --no-deps --format-version=1 \
  | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d["packages"][0]["version"])')
echo "==> Building $BIN_NAME v${VERSION} channel=${CHANNEL} for ${TARGET} (platform=${PLATFORM})"
echo "    formats:$FORMATS"

# ── Build metadata ────────────────────────────────────────────────────────────
GIT_COMMIT=$(git rev-parse HEAD)
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
if [ -n "$(git status --porcelain)" ]; then
  DIRTY="true"
else
  DIRTY="false"
fi
echo "    commit=${GIT_COMMIT} branch=${GIT_BRANCH} dirty=${DIRTY}"

# ── Dirty production gate ─────────────────────────────────────────────────────
# Production artifacts MUST NOT silently report clean provenance from an
# uncommitted tree. Development builds are allowed and explicitly marked.
if [ "$CHANNEL" = "production" ] && [ "$DIRTY" = "true" ]; then
  echo "ERROR: refusing production release build from a dirty working tree." >&2
  git status --short >&2
  echo "Commit or stash changes, or build with '--channel development'." >&2
  exit 1
fi

# ── Build ─────────────────────────────────────────────────────────────────────
# shellcheck disable=SC2086
if [ "$TARGET" = "$HOST_TRIPLE" ]; then
  echo "==> Running: cargo build --release $FEATURES"
  # shellcheck disable=SC2086
  cargo build --release $FEATURES
  BINARY="target/release/m31a${EXE_SUFFIX}"
else
  echo "==> Running: cargo build --release --target ${TARGET} $FEATURES"
  # shellcheck disable=SC2086
  cargo build --release --target "${TARGET}" $FEATURES
  BINARY="target/${TARGET}/release/m31a${EXE_SUFFIX}"
fi

if [ ! -f "${BINARY}" ]; then
  echo "ERROR: Release binary not found at ${BINARY}" >&2
  exit 1
fi

echo "==> Binary: $(ls -lh "${BINARY}" | awk '{print $9, $5}')"

# ── Artifact identity validation ──────────────────────────────────────────────
echo "==> Validating artifact identity..."
if [ "$CAN_EXEC" = "1" ]; then
  BINARY_VERSION=$("${BINARY}" --version 2>&1 || true)
  echo "    ${BINARY_VERSION}"
  if [ "$CHANNEL" = "production" ]; then
    if [[ "${BINARY_VERSION}" != "m31a ${VERSION}" ]]; then
      echo "ERROR: Binary version '${BINARY_VERSION}' does not match Cargo.toml '${VERSION}'" >&2
      exit 1
    fi
  else
    if [[ "${BINARY_VERSION}" != "m31a-dev ${VERSION}-dev+"* ]]; then
      echo "ERROR: Development binary version '${BINARY_VERSION}' lacks expected 'm31a-dev ${VERSION}-dev+' identity" >&2
      exit 1
    fi
  fi
  echo "    Version consistency: PASS (channel=${CHANNEL})"
else
  # Cross-arch: the binary cannot execute here; enforce presence, size,
  # and executable bit instead of version output.
  if [ ! -s "${BINARY}" ]; then
    echo "ERROR: Release binary missing or empty at ${BINARY}" >&2
    exit 1
  fi
  echo "    Cross-arch binary present ($(wc -c < "${BINARY}" | tr -d ' ') bytes; execution check skipped)"
fi
echo "    Target metadata: ${TARGET}"
echo "    Dirty marker: dirty=${DIRTY}"

# ── Package staging ───────────────────────────────────────────────────────────
echo "==> Staging package files..."
mkdir -p dist
if [ "$CHANNEL" = "production" ]; then
  PKG_BASE="m31a-${VERSION}-${PLATFORM}"
else
  PKG_BASE="m31a-dev-${VERSION}-${PLATFORM}"
fi
PKG_TMP="${REPO_ROOT}/dist/.stage-${PKG_BASE}"
rm -rf "${PKG_TMP}"
mkdir -p "${PKG_TMP}"

cp "${BINARY}"              "${PKG_TMP}/${BIN_NAME}${EXE_SUFFIX}"
cp README.md                "${PKG_TMP}/README.md"
cp CHANGELOG.md             "${PKG_TMP}/CHANGELOG.md"
[ -f LICENSE ] && cp LICENSE "${PKG_TMP}/LICENSE"

EPOCH="$(git log -1 --format=%ct)"
# Collected distributable artifacts: filenames relative to dist/.
ARTIFACTS=""

# ── Format: deterministic tar.gz (Unix) ──────────────────────────────────────
package_tarball() {
  echo "==> Packaging tar.gz..."
  python3 - "${PKG_TMP}" "${PKG_BASE}" "${EPOCH}" <<'PYEOF'
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
  mv "${PKG_TMP}.tar.gz" "dist/${PKG_BASE}.tar.gz"
  ARTIFACTS="$ARTIFACTS ${PKG_BASE}.tar.gz"
  echo "    Archive: dist/${PKG_BASE}.tar.gz"
}

# ── Format: deterministic zip ────────────────────────────────────────────────
package_zip() {
  echo "==> Packaging zip..."
  python3 - "${PKG_TMP}" "${PKG_BASE}" "${EPOCH}" <<'PYEOF'
import sys, os, zipfile
pkg_tmp, pkg_dir, epoch = sys.argv[1:4]
mtime = int(epoch)
# Zip timestamps have 2s resolution and no timezone; clamp deterministically.
ztime = __import__("datetime").datetime.fromtimestamp(mtime, tz=__import__("datetime").timezone.utc).timetuple()[:6]
out_path = os.path.join("dist", pkg_dir + ".zip")
names = sorted(os.listdir(pkg_tmp))
with zipfile.ZipFile(out_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
    for name in names:
        full = os.path.join(pkg_tmp, name)
        arc = os.path.join(pkg_dir, name)
        zi = zipfile.ZipInfo(arc, date_time=ztime)
        zi.compress_type = zipfile.ZIP_DEFLATED
        zi.create_system = 3  # Unix attributes follow
        if os.path.isfile(full):
            with open(full, "rb") as f:
                data = f.read()
            zi.external_attr = (0o755 if os.access(full, os.X_OK) else 0o644) << 16
            zf.writestr(zi, data)
        else:
            zi.external_attr = 0o755 << 16
            zf.writestr(zi, b"")
print("    deterministic entries: %d (mtime=%s)" % (len(names), epoch))
PYEOF
  ARTIFACTS="$ARTIFACTS ${PKG_BASE}.zip"
  echo "    Archive: dist/${PKG_BASE}.zip"
}

# ── Format: Debian package (Linux, cargo-deb) ────────────────────────────────
package_deb() {
  echo "==> Packaging deb (cargo-deb)..."
  if ! command -v cargo-deb >/dev/null 2>&1 && ! cargo deb --version >/dev/null 2>&1; then
    echo "ERROR: 'cargo-deb' not found. Install with: cargo install cargo-deb" >&2
    exit 1
  fi
  # shellcheck disable=SC2086
  if [ "$TARGET" = "$HOST_TRIPLE" ]; then
    cargo deb --no-build
    DEB_SRC=(target/debian/*.deb)
  else
    cargo deb --target "$TARGET" --no-build
    DEB_SRC=(target/"$TARGET"/debian/*.deb)
  fi
  if [ ! -f "${DEB_SRC[0]}" ]; then
    echo "ERROR: cargo-deb produced no .deb artifact" >&2
    exit 1
  fi
  for _d in "${DEB_SRC[@]}"; do
    cp "$_d" dist/
    ARTIFACTS="$ARTIFACTS $(basename "$_d")"
    echo "    Debian package: dist/$(basename "$_d")"
  done
}

# ── Format: RPM package (Linux, cargo-generate-rpm) ──────────────────────────
package_rpm() {
  echo "==> Packaging rpm (cargo-generate-rpm)..."
  if ! command -v cargo-generate-rpm >/dev/null 2>&1; then
    echo "ERROR: 'cargo-generate-rpm' not found. Install with: cargo install cargo-generate-rpm" >&2
    exit 1
  fi
  # cargo-generate-rpm performs its own build; the cargo cache makes the
  # earlier `cargo build` reusable so this stays incremental. Cross-target
  # RPM builds are not supported by the tool: TARGET must equal the host.
  if [ "$TARGET" != "$HOST_TRIPLE" ]; then
    echo "ERROR: --format rpm only supports the host target (got ${TARGET}, host ${HOST_TRIPLE})" >&2
    exit 1
  fi
  cargo generate-rpm
  RPM_SRC=(target/generate-rpm/*.rpm)
  if [ ! -f "${RPM_SRC[0]}" ]; then
    echo "ERROR: cargo-generate-rpm produced no .rpm artifact" >&2
    exit 1
  fi
  for _r in "${RPM_SRC[@]}"; do
    cp "$_r" dist/
    ARTIFACTS="$ARTIFACTS $(basename "$_r")"
    echo "    RPM package: dist/$(basename "$_r")"
  done
}

# ── Format: AppImage (Linux, appimagetool) ───────────────────────────────────
package_appimage() {
  echo "==> Packaging AppImage (appimagetool)..."
  TOOLS_DIR="${REPO_ROOT}/dist/.tools"
  mkdir -p "$TOOLS_DIR"
  APPIMAGETOOL="${APPIMAGETOOL:-$TOOLS_DIR/appimagetool}"
  if [ ! -x "$APPIMAGETOOL" ]; then
    echo "    appimagetool not found; downloading..."
    case "$(uname -m)" in
      x86_64) _AT_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" ;;
      aarch64) _AT_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-aarch64.AppImage" ;;
      *) echo "ERROR: unsupported AppImage host arch '$(uname -m)'" >&2; exit 1 ;;
    esac
    curl -fsSL "$_AT_URL" -o "$APPIMAGETOOL"
    chmod +x "$APPIMAGETOOL"
  fi
  APPDIR="${REPO_ROOT}/dist/.AppDir-${PKG_BASE}"
  rm -rf "$APPDIR"
  mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/scalable/apps" "$APPDIR/usr/share/doc/m31a"
  cp "${BINARY}" "$APPDIR/usr/bin/${BIN_NAME}"
  cp README.md CHANGELOG.md LICENSE "$APPDIR/usr/share/doc/m31a/" 2>/dev/null || true
  cat > "$APPDIR/usr/share/applications/m31a.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=M31 Autonomous
Comment=M31 Autonomous — Rust-native autonomous software-engineering runtime
Exec=${BIN_NAME}
Icon=m31a
Categories=Development;Utility;
Terminal=true
EOF
  cat > "$APPDIR/usr/share/icons/hicolor/scalable/apps/m31a.svg" <<'EOF'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="12" fill="#0b1020"/><text x="32" y="42" font-family="monospace" font-size="28" fill="#7dd3fc" text-anchor="middle">M31</text></svg>
EOF
  ln -sf "usr/share/applications/m31a.desktop" "$APPDIR/m31a.desktop"
  ln -sf "usr/share/icons/hicolor/scalable/apps/m31a.svg" "$APPDIR/m31a.svg"
  ln -sf "usr/bin/${BIN_NAME}" "$APPDIR/AppRun"
  APPIMAGE_OUT="dist/${PKG_BASE}.AppImage"
  ARCH="$(uname -m)"
  # shellcheck disable=SC2086
  env APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$ARCH" "$APPIMAGETOOL" ${APPIMAGETOOL_ARGS:-} "$APPDIR" "$APPIMAGE_OUT" >/dev/null
  rm -rf "$APPDIR"
  ARTIFACTS="$ARTIFACTS $(basename "$APPIMAGE_OUT")"
  echo "    AppImage: $APPIMAGE_OUT"
}

# ── Format: macOS disk image (macOS, hdiutil) ────────────────────────────────
package_dmg() {
  echo "==> Packaging dmg (hdiutil)..."
  if ! command -v hdiutil >/dev/null 2>&1; then
    echo "ERROR: 'hdiutil' not found (macOS host required for --format dmg)" >&2
    exit 1
  fi
  DMG_STAGE="${REPO_ROOT}/dist/.dmg-${PKG_BASE}"
  rm -rf "$DMG_STAGE"
  mkdir -p "$DMG_STAGE"
  cp "${BINARY}" "$DMG_STAGE/${BIN_NAME}"
  cp README.md CHANGELOG.md LICENSE "$DMG_STAGE/" 2>/dev/null || true
  # Clean intermediate compiler build artifacts to free disk space on constrained macOS runners
  rm -rf "${REPO_ROOT}/target"/*/release/deps "${REPO_ROOT}/target"/*/release/build "${REPO_ROOT}/target/release/deps" "${REPO_ROOT}/target/release/build" "${REPO_ROOT}/target/debug" 2>/dev/null || true
  DMG_OUT="dist/${PKG_BASE}.dmg"
  rm -f "$DMG_OUT"
  hdiutil create -volname "m31a ${VERSION}" -srcfolder "$DMG_STAGE" -ov -format UDZO "$DMG_OUT" >/dev/null
  rm -rf "$DMG_STAGE"
  ARTIFACTS="$ARTIFACTS $(basename "$DMG_OUT")"
  echo "    Disk image: $DMG_OUT"
}

# ── Format: Windows installer (Windows, cargo-wix) ──────────────────────────
package_msi() {
  echo "==> Packaging msi (cargo-wix)..."
  if ! command -v cargo-wix >/dev/null 2>&1 && ! cargo wix --version >/dev/null 2>&1; then
    echo "ERROR: 'cargo-wix' not found. Install with: cargo install cargo-wix" >&2
    exit 1
  fi
  # cargo-wix requires generated WiX sources; create them from Cargo metadata
  # when absent (uses [package.metadata.wix] when present).
  if [ ! -f wix/main.wxs ]; then
    echo "    generating wix/ sources via 'cargo wix init'..."
    cargo wix init
  fi
  # cargo-wix performs its own release build; cargo cache keeps it
  # incremental. CARGO_BUILD_TARGET carries cross targets (e.g. ARM64 MSI
  # built on x64 runners — WiX itself is arch-neutral).
  if [ "$TARGET" = "$HOST_TRIPLE" ]; then
    cargo wix --nocapture
  else
    CARGO_BUILD_TARGET="$TARGET" cargo wix --nocapture
  fi
  MSI_SRC=(target/wix/*.msi)
  if [ ! -f "${MSI_SRC[0]}" ]; then
    echo "ERROR: cargo-wix produced no .msi artifact" >&2
    exit 1
  fi
  MSI_OUT="dist/${PKG_BASE}.msi"
  cp "${MSI_SRC[0]}" "$MSI_OUT"
  ARTIFACTS="$ARTIFACTS $(basename "$MSI_OUT")"
  echo "    Installer: $MSI_OUT"
}

for _f in $FORMATS; do
  case "$_f" in
    tar.gz) package_tarball ;;
    zip) package_zip ;;
    deb) package_deb ;;
    rpm) package_rpm ;;
    appimage) package_appimage ;;
    dmg) package_dmg ;;
    msi) package_msi ;;
  esac
done

# Staging is reusable across formats; remove only after all are built.
rm -rf "${PKG_TMP}"

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
# shellcheck disable=SC2086
$SHA256SUM $ARTIFACTS sbom.json > SHA256SUMS
cat SHA256SUMS
# shellcheck disable=SC2086
$SHA256SUM --check SHA256SUMS >/dev/null 2>&1 || shasum -a 256 -c SHA256SUMS >/dev/null
echo "    Checksum verification: PASS"
# Per-archive sidecars (CI release jobs glob dist/*.sha256).
for _a in $ARTIFACTS; do
  # shellcheck disable=SC2086
  $SHA256SUM "$_a" > "${_a}.sha256" 2>/dev/null || shasum -a 256 "$_a" > "${_a}.sha256"
done
echo "    Per-archive sidecars: $(echo "$ARTIFACTS" | wc -w)"
cd "${REPO_ROOT}"

# ── Release metadata ──────────────────────────────────────────────────────────
echo "==> Generating release.json..."
RUSTC_VERSION=$(rustc --version)
CARGO_VERSION=$(cargo --version)
BUILD_TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
BINARY_SHA256=$(python3 -c "import hashlib; print(hashlib.sha256(open('${BINARY}','rb').read()).hexdigest())")
# Deterministic build ID: sha256(version|channel|commit|target)[..16].
BUILD_ID=$(python3 -c "import hashlib; print(hashlib.sha256('|'.join(['${VERSION}','${CHANNEL}','${GIT_COMMIT}','${TARGET}']).encode()).hexdigest()[:16])")

python3 - "${VERSION}" "${CHANNEL}" "${BIN_NAME}" "${BUILD_ID}" "${GIT_COMMIT}" "${GIT_BRANCH}" "${DIRTY}" "${TARGET}" "${PLATFORM}" "${BINARY_SHA256}" "${BUILD_TIMESTAMP}" "${RUSTC_VERSION}" "${CARGO_VERSION}" "$ARTIFACTS" <<'PYEOF'
import sys, json, hashlib
(version, channel, bin_name, build_id, commit, branch, dirty, target,
 platform, bin_sha, timestamp, rustc, cargo, artifacts) = sys.argv[1:14]
entries = []
for name in artifacts.split():
    data = open("dist/" + name, "rb").read()
    entries.append({
        "filename": name,
        "sha256": hashlib.sha256(data).hexdigest(),
        "size": len(data),
        "target": target,
        "platform": platform,
        "build_id": build_id,
        "commit": commit,
    })
with open("dist/release.json", "w") as f:
    json.dump({
        "version": version,
        "channel": channel,
        "binary": bin_name,
        "build_id": build_id,
        "git_commit": commit,
        "git_branch": branch,
        "source_dirty": dirty == "true",
        "target": target,
        "platform": platform,
        "binary_sha256": bin_sha,
        "build_timestamp": timestamp,
        "rustc_version": rustc,
        "cargo_version": cargo,
        "artifacts": entries,
    }, f, indent=2)
    f.write("\n")
print("    release.json artifacts: %d" % len(entries))
PYEOF

# ── Deployment manifest (schema v1, channel-aware, all artifacts) ────────────
echo "==> Generating deployment-manifest.json..."
python3 - "${VERSION}" "${CHANNEL}" "${BUILD_ID}" "${GIT_COMMIT}" "${TARGET}" "${PLATFORM}" "$ARTIFACTS" <<'PYEOF'
import sys, json, hashlib
version, channel, build_id, commit, target, platform, artifacts = sys.argv[1:8]
entries = []
for name in artifacts.split():
    data = open("dist/" + name, "rb").read()
    entries.append({
        "target": target,
        "platform": platform,
        "filename": name,
        "sha256": hashlib.sha256(data).hexdigest(),
        "size": len(data),
        "build_id": build_id,
        "commit": commit,
    })
with open("dist/deployment-manifest.json", "w") as f:
    json.dump({
        "schema_version": 1,
        "version": version,
        "channel": channel,
        "build_id": build_id,
        "commit": commit,
        "artifacts": entries,
    }, f, indent=2)
    f.write("\n")
print("    deployment-manifest.json: channel=%s target=%s artifacts=%d" % (channel, target, len(entries)))
PYEOF

# ── Post-package validation ───────────────────────────────────────────────────
echo "==> Validating packaged artifacts..."
for _a in $ARTIFACTS; do
  case "$_a" in
    *.tar.gz)
      TMPD=$(mktemp -d)
      tar -xzf "dist/$_a" -C "$TMPD"
      STAGED_BIN="$TMPD/${PKG_BASE}/${BIN_NAME}${EXE_SUFFIX}"
      if [ ! -f "$STAGED_BIN" ]; then
        echo "ERROR: expected binary missing in archive: $_a" >&2
        rm -rf "$TMPD"
        exit 1
      fi
      if [ "$CAN_EXEC" = "1" ]; then
        STAGED_VER=$("$STAGED_BIN" --version 2>&1 || true)
        echo "    $_a: ${STAGED_VER}"
        if [ "$CHANNEL" = "production" ] && [[ "$STAGED_VER" != "m31a ${VERSION}" ]]; then
          echo "ERROR: staged production binary misreports version ($_a)" >&2
          rm -rf "$TMPD"
          exit 1
        fi
        if [ "$CHANNEL" = "development" ] && [[ "$STAGED_VER" != "m31a-dev ${VERSION}-dev+"* ]]; then
          echo "ERROR: staged development binary misreports channel ($_a)" >&2
          rm -rf "$TMPD"
          exit 1
        fi
      else
        echo "    $_a: binary present (cross-arch execution check skipped)"
      fi
      rm -rf "$TMPD"
      ;;
    *.zip)
      if ! python3 -c "import zipfile,sys; z=zipfile.ZipFile(sys.argv[1]); assert any(n.endswith('${BIN_NAME}${EXE_SUFFIX}') for n in z.namelist()), 'binary missing'" "dist/$_a"; then
        echo "ERROR: expected binary missing in archive: $_a" >&2
        exit 1
      fi
      echo "    $_a: binary present"
      ;;
    *.deb)
      if command -v dpkg-deb >/dev/null 2>&1; then
        dpkg-deb --info "dist/$_a" | head -8
        dpkg-deb --contents "dist/$_a" | grep -q "usr/bin/${BIN_NAME}$" \
          || { echo "ERROR: ${_a} missing usr/bin/${BIN_NAME}" >&2; exit 1; }
        echo "    $_a: contents PASS"
      else
        echo "    $_a: present (dpkg-deb unavailable for content check)"
      fi
      ;;
    *.rpm)
      if command -v rpm >/dev/null 2>&1; then
        rpm -qlp "dist/$_a" | grep -q "/usr/bin/${BIN_NAME}$" \
          || { echo "ERROR: ${_a} missing /usr/bin/${BIN_NAME}" >&2; exit 1; }
        echo "    $_a: contents PASS"
      else
        echo "    $_a: present (rpm unavailable for content check)"
      fi
      ;;
    *.AppImage)
      if [ -x "dist/$_a" ]; then echo "    $_a: executable PASS"; else
        echo "ERROR: ${_a} is not executable" >&2; exit 1
      fi
      ;;
    *.dmg)
      if command -v hdiutil >/dev/null 2>&1; then
        hdiutil verify "dist/$_a" >/dev/null \
          || { echo "ERROR: ${_a} failed hdiutil verify" >&2; exit 1; }
        echo "    $_a: verify PASS"
      else
        echo "    $_a: present (hdiutil unavailable for verify)"
      fi
      ;;
    *.msi)
      if [ -s "dist/$_a" ]; then echo "    $_a: non-empty PASS"; else
        echo "ERROR: ${_a} is empty" >&2; exit 1
      fi
      ;;
  esac
done
echo "    Artifact validation: PASS"

echo ""
echo "==> Release artifacts ready in dist/:"
ls -lh dist/

echo ""
echo "==> Release build complete: $BIN_NAME v${VERSION} channel=${CHANNEL} dirty=${DIRTY}"
echo "    formats:$FORMATS"
echo "    Next steps: run scripts/release-check.sh to run all quality gates."
