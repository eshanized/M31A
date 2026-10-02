#!/usr/bin/env bash
# scripts/install-local.sh — user-local channel-aware installer (build from source).
#
# Usage:
#   ./scripts/install-local.sh [--channel development|production] [--prefix DIR]
#
# Installs the freshly built binary as `m31a` (production) or `m31a-dev`
# (development) into a user-local bin directory without requiring root.
# Default prefix resolution (platform-aware):
#   - $CARGO_HOME/bin when set, else ~/.cargo/bin when it exists,
#   - else ~/.local/bin (Unix).
# Pass --prefix explicitly on macOS/Windows when the default is unsuitable.
# Never touches user config/data/state: installation replaces only the binary,
# preserving the previous file as `<binary>.prev` for rollback.
#
# For installing published release archives, use scripts/install.sh instead.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

CHANNEL="production"
PREFIX=""

while [ $# -gt 0 ]; do
  case "$1" in
    --channel=*) CHANNEL="${1#--channel=}"; shift ;;
    --channel)
      shift
      if [ $# -eq 0 ]; then echo "ERROR: --channel requires development|production" >&2; exit 1; fi
      CHANNEL="$1"; shift ;;
    development|production) CHANNEL="$1"; shift ;;
    --prefix=*) PREFIX="${1#--prefix=}"; shift ;;
    --prefix)
      shift
      if [ $# -eq 0 ]; then echo "ERROR: --prefix requires a directory" >&2; exit 1; fi
      PREFIX="$1"; shift ;;
    -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
    *) echo "ERROR: unknown argument '$1'" >&2; exit 1 ;;
  esac
done

case "$CHANNEL" in
  development|production) ;;
  *) echo "ERROR: unknown channel '$CHANNEL'" >&2; exit 1 ;;
esac

if [ "$CHANNEL" = "development" ]; then
  BIN_NAME="m31a-dev"
  FEATURES="--features development"
else
  BIN_NAME="m31a"
  FEATURES=""
fi

if [ -z "$PREFIX" ]; then
  if [ -n "${CARGO_HOME:-}" ]; then
    PREFIX="$CARGO_HOME/bin"
  elif [ -d "$HOME/.cargo/bin" ]; then
    PREFIX="$HOME/.cargo/bin"
  else
    PREFIX="$HOME/.local/bin"
  fi
fi

echo "==> Ensuring $BIN_NAME is built (channel=${CHANNEL})..."
# shellcheck disable=SC2086
cargo build --release $FEATURES

SRC="target/release/m31a"
if [ ! -f "$SRC" ]; then
  echo "ERROR: built binary missing at $SRC" >&2
  exit 1
fi

mkdir -p "$PREFIX"
DEST="$PREFIX/$BIN_NAME"
if [ -f "$DEST" ]; then
  cp "$DEST" "$DEST.prev"
  echo "    previous binary preserved at $DEST.prev"
fi
cp "$SRC" "$DEST"
chmod 755 "$DEST"
echo "==> Installed $DEST"
"$DEST" --version
if [ "$CHANNEL" = "development" ]; then
  "$DEST" deployment 2>/dev/null | head -8 || true
fi
echo "    User state untouched (config/data/cache/state preserved)."
