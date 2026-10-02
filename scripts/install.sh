#!/usr/bin/env bash
# M31A Standalone Binary Installer for Linux and macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/scripts/install.sh | bash

set -euo pipefail

REPO="eshanized/M31A"
DEFAULT_TAG="v0.1.1"

echo "==> Detecting host system..."
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Linux)
        case "${ARCH}" in
            x86_64)  TARGET="linux-x64" ;;
            aarch64|arm64) TARGET="linux-arm64" ;;
            *) echo "Error: Unsupported Linux architecture: ${ARCH}" >&2; exit 1 ;;
        esac
        ;;
    Darwin)
        case "${ARCH}" in
            x86_64)  TARGET="darwin-x64" ;;
            arm64|aarch64) TARGET="darwin-arm64" ;;
            *) echo "Error: Unsupported macOS architecture: ${ARCH}" >&2; exit 1 ;;
        esac
        ;;
    *)
        echo "Error: Unsupported operating system: ${OS}. For Windows, please run install.ps1 in PowerShell." >&2
        exit 1
        ;;
esac

TAG="${M31A_VERSION:-$DEFAULT_TAG}"
ARCHIVE="m31a-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}"
CHECKSUM_URL="${URL}.sha256"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "==> Downloading M31A ${TAG} for ${TARGET}..."
curl -fsSL "${URL}" -o "${TMP_DIR}/${ARCHIVE}"
curl -fsSL "${CHECKSUM_URL}" -o "${TMP_DIR}/${ARCHIVE}.sha256"

echo "==> Verifying SHA-256 checksum..."
cd "${TMP_DIR}"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "${ARCHIVE}.sha256"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "${ARCHIVE}.sha256"
else
    echo "Warning: Neither sha256sum nor shasum found; skipping checksum verification."
fi

echo "==> Extracting binary..."
tar -xzf "${ARCHIVE}"

INSTALL_DIR="/usr/local/bin"
if [ ! -w "${INSTALL_DIR}" ]; then
    INSTALL_DIR="${HOME}/.local/bin"
    mkdir -p "${INSTALL_DIR}"
fi

cp "m31a-${TARGET}/m31a" "${INSTALL_DIR}/m31a"
chmod +x "${INSTALL_DIR}/m31a"

echo ""
echo "==> Successfully installed M31A to ${INSTALL_DIR}/m31a!"
"${INSTALL_DIR}/m31a" --version

if ! echo "${PATH}" | tr ':' '\n' | grep -qx "${INSTALL_DIR}"; then
    echo ""
    echo "Notice: ${INSTALL_DIR} is not in your PATH."
    echo "Add it to your shell configuration:"
    echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
