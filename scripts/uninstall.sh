#!/usr/bin/env bash
# scripts/uninstall.sh — remove M31A binaries installed by scripts/install.sh
# or scripts/install-local.sh (Linux and macOS).
#
# Usage:
#   ./scripts/uninstall.sh [--channel production|development|all]
#                          [--prefix DIR]... [--purge] [--yes]
#
# Removes the `m31a` (production) and/or `m31a-dev` (development) binaries
# (plus `<binary>.prev` rollback backups) from the known install locations:
# /usr/local/bin, $CARGO_HOME/bin, ~/.cargo/bin, ~/.local/bin, and any
# explicit --prefix directories (repeatable).
#
# User state (config/data/cache/state) is PRESERVED by default, mirroring
# the installers' "never touches user config" contract. Pass --purge to
# also remove channel state directories (only exact `m31a` / `m31a-dev`
# leaf directories are ever deleted — parent directories are untouched).
# Project-local `.m31a/` workspace directories are never touched.
# --purge prompts for confirmation unless --yes is given.
#
# For Windows, use scripts/uninstall.ps1 instead.

set -euo pipefail

CHANNEL="all"
PREFIXES=()
PURGE=0
YES=0

while [ $# -gt 0 ]; do
  case "$1" in
    --channel=*) CHANNEL="${1#--channel=}"; shift ;;
    --channel)
      shift
      if [ $# -eq 0 ]; then echo "ERROR: --channel requires production|development|all" >&2; exit 1; fi
      CHANNEL="$1"; shift ;;
    production|development|all) CHANNEL="$1"; shift ;;
    --prefix=*) PREFIXES+=("${1#--prefix=}"); shift ;;
    --prefix)
      shift
      if [ $# -eq 0 ]; then echo "ERROR: --prefix requires a directory" >&2; exit 1; fi
      PREFIXES+=("$1"); shift ;;
    --purge) PURGE=1; shift ;;
    -y|--yes) YES=1; shift ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "ERROR: unknown argument '$1'" >&2; exit 1 ;;
  esac
done

case "$CHANNEL" in
  production|development|all) ;;
  *) echo "ERROR: unknown channel '$CHANNEL'" >&2; exit 1 ;;
esac

BIN_NAMES=()
if [ "$CHANNEL" = "production" ] || [ "$CHANNEL" = "all" ]; then
  BIN_NAMES+=("m31a")
fi
if [ "$CHANNEL" = "development" ] || [ "$CHANNEL" = "all" ]; then
  BIN_NAMES+=("m31a-dev")
fi
APP_DIRS=()
if [ "$CHANNEL" = "production" ] || [ "$CHANNEL" = "all" ]; then
  APP_DIRS+=("m31a")
fi
if [ "$CHANNEL" = "development" ] || [ "$CHANNEL" = "all" ]; then
  APP_DIRS+=("m31a-dev")
fi

SEARCH_DIRS=("/usr/local/bin")
if [ -n "${CARGO_HOME:-}" ]; then
  SEARCH_DIRS+=("$CARGO_HOME/bin")
fi
if [ -d "$HOME/.cargo/bin" ]; then
  SEARCH_DIRS+=("$HOME/.cargo/bin")
fi
SEARCH_DIRS+=("$HOME/.local/bin")
for p in ${PREFIXES[@]+"${PREFIXES[@]}"}; do
  [ -n "$p" ] && SEARCH_DIRS+=("$p")
done

removed=0
echo "==> Removing M31A binaries (channel=${CHANNEL})..."
for d in "${SEARCH_DIRS[@]}"; do
  for b in "${BIN_NAMES[@]}"; do
    for f in "$d/$b" "$d/$b.prev"; do
      if [ -f "$f" ] || [ -L "$f" ]; then
        rm -f "$f"
        echo "    removed $f"
        removed=$((removed + 1))
      fi
    done
  done
done
if [ "$removed" -eq 0 ]; then
  echo "    no M31A binaries found in searched locations."
fi

# ── Optional state purge ────────────────────────────────────────────────
# Candidate parents follow the `directories` crate conventions used by
# DeploymentPaths (ProjectDirs::from("com", "m31a", <app>)) plus the
# M31A_*_DIR env overrides. Only exact leaf matches are deleted.
purge_targets=()
if [ "$PURGE" -eq 1 ]; then
  OS="$(uname -s)"
  parents=()
  case "$OS" in
    Linux)
      parents+=("$HOME/.config" "$HOME/.local/share" "$HOME/.cache" "$HOME/.local/state")
      [ -n "${XDG_CONFIG_HOME:-}" ] && parents+=("$XDG_CONFIG_HOME")
      [ -n "${XDG_DATA_HOME:-}" ] && parents+=("$XDG_DATA_HOME")
      [ -n "${XDG_CACHE_HOME:-}" ] && parents+=("$XDG_CACHE_HOME")
      [ -n "${XDG_STATE_HOME:-}" ] && parents+=("$XDG_STATE_HOME")
      ;;
    Darwin)
      parents+=("$HOME/Library/Application Support" "$HOME/Library/Caches")
      ;;
    *)
      echo "Warning: unknown OS '$OS'; skipping standard state locations." >&2
      ;;
  esac
  for parent in "${parents[@]}"; do
    for app in "${APP_DIRS[@]}"; do
      for leaf in "$app" "com.m31a.$app"; do
        if [ -d "$parent/$leaf" ]; then
          purge_targets+=("$parent/$leaf")
        fi
      done
    done
  done
  for var in M31A_CONFIG_DIR M31A_DATA_DIR M31A_CACHE_DIR M31A_STATE_DIR \
             M31A_DEV_CONFIG_DIR M31A_DEV_DATA_DIR M31A_DEV_CACHE_DIR M31A_DEV_STATE_DIR; do
    if [ -n "${!var:-}" ] && [ -d "${!var}" ]; then
      purge_targets+=("${!var}")
    fi
  done

  # De-duplicate while preserving order.
  unique=()
  for t in ${purge_targets[@]+"${purge_targets[@]}"}; do
    skip=0
    for u in ${unique[@]+"${unique[@]}"}; do
      if [ "$t" = "$u" ]; then skip=1; break; fi
    done
    [ "$skip" -eq 0 ] && unique+=("$t")
  done
  purge_targets=()
  for u in ${unique[@]+"${unique[@]}"}; do
    purge_targets+=("$u")
  done

  if [ "${#purge_targets[@]}" -eq 0 ]; then
    echo "==> --purge: no M31A state directories found."
  else
    echo "==> --purge: the following state directories will be permanently deleted:"
    for t in "${purge_targets[@]}"; do
      echo "    $t"
    done
    if [ "$YES" -ne 1 ]; then
      printf "Delete these directories? [y/N] "
      read -r answer
      case "$answer" in
        y|Y|yes|YES) ;;
        *) echo "Aborted; binaries were still removed, state preserved."; exit 0 ;;
      esac
    fi
    for t in "${purge_targets[@]}"; do
      rm -rf "$t"
      echo "    purged $t"
    done
  fi
else
  echo "User state preserved (config/data/cache/state untouched). Re-run with --purge to remove it."
fi

echo ""
echo "==> Uninstall complete."
