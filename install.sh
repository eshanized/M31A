#!/usr/bin/env bash
set -euo pipefail

# M31A Install Script
# Usage: curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/install.sh | bash
# Or:    bash install.sh [--version VERSION] [--bin-dir DIR]

REPO="eshanized/M31A"
BIN_NAME="m31a"
VERSION="${VERSION:-latest}"
BIN_DIR="${BIN_DIR:-/usr/local/bin}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --bin-dir) BIN_DIR="$2"; shift 2 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "darwin" ;;
        CYGWIN*|MINGW*|MSYS*) echo "windows" ;;
        *) echo "unknown" ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64)  echo "amd64" ;;
        aarch64|arm64) echo "arm64" ;;
        *) echo "unknown" ;;
    esac
}

OS=$(detect_os)
ARCH=$(detect_arch)

if [[ "$OS" == "unknown" || "$ARCH" == "unknown" ]]; then
    echo "Error: Unsupported platform $(uname -s) $(uname -m)"
    echo "Please build from source: CGO_ENABLED=0 go build -o $BIN_NAME ./cmd/m31a"
    exit 1
fi

# Resolve version
if [[ "$VERSION" == "latest" ]]; then
    VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
fi

echo "Installing $BIN_NAME $VERSION for $OS/$ARCH..."

# Determine archive format
EXT="tar.gz"
if [[ "$OS" == "windows" ]]; then
    EXT="zip"
    BIN_NAME="m31a.exe"
fi

URL="https://github.com/$REPO/releases/download/$VERSION/m31a_${VERSION}_${OS}_${ARCH}.${EXT}"

# Create temp directory
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

# Download
echo "Downloading $URL..."
if command -v curl &>/dev/null; then
    curl -fsSL "$URL" -o "$TMPDIR/archive.${EXT}"
elif command -v wget &>/dev/null; then
    wget -q "$URL" -O "$TMPDIR/archive.${EXT}"
else
    echo "Error: curl or wget required"
    exit 1
fi

# Verify checksum if available
CHECKSUM_URL="https://github.com/$REPO/releases/download/$VERSION/checksums.txt"
if curl -fsSL "$CHECKSUM_URL" -o "$TMPDIR/checksums.txt" 2>/dev/null; then
    EXPECTED=$(grep "m31a_${VERSION}_${OS}_${ARCH}.${EXT}" "$TMPDIR/checksums.txt" | awk '{print $1}')
    if [[ -n "$EXPECTED" ]]; then
        ACTUAL=$(shasum -a 256 "$TMPDIR/archive.${EXT}" 2>/dev/null | awk '{print $1}' || sha256sum "$TMPDIR/archive.${EXT}" | awk '{print $1}')
        if [[ "$EXPECTED" != "$ACTUAL" ]]; then
            echo "Error: Checksum mismatch"
            exit 1
        fi
        echo "Checksum verified."
    fi
fi

# Extract
if [[ "$EXT" == "tar.gz" ]]; then
    tar -xzf "$TMPDIR/archive.${EXT}" -C "$TMPDIR"
else
    unzip -q "$TMPDIR/archive.${EXT}" -d "$TMPDIR"
fi

# Install
mkdir -p "$BIN_DIR"
if [[ -f "$TMPDIR/$BIN_NAME" ]]; then
    mv "$TMPDIR/$BIN_NAME" "$BIN_DIR/$BIN_NAME"
elif [[ -f "$TMPDIR/m31a_${VERSION}_${OS}_${ARCH}/$BIN_NAME" ]]; then
    mv "$TMPDIR/m31a_${VERSION}_${OS}_${ARCH}/$BIN_NAME" "$BIN_DIR/$BIN_NAME"
else
    echo "Error: Binary not found in archive"
    find "$TMPDIR" -type f -name "$BIN_NAME"
    exit 1
fi

chmod +x "$BIN_DIR/$BIN_NAME"

echo ""
echo "Installed $BIN_name to $BIN_DIR/$BIN_NAME"
echo "Run: m31a --help"
