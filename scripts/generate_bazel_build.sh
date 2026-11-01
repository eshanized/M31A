#!/usr/bin/env bash
# Generate BUILD.bazel files from Cargo.toml using cargo2bazel
# This script helps bootstrap Bazel configuration from existing Cargo workspace

set -euo pipefail

WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${WORKSPACE_ROOT}"

# Install cargo2bazel if not present
if ! command -v cargo2bazel &> /dev/null; then
    echo "Installing cargo2bazel..."
    cargo install cargo2bazel
fi

echo "Generating BUILD.bazel files from Cargo.toml..."

# Generate per-crate BUILD.bazel files
cargo2bazel generate \
    --workspace-root "${WORKSPACE_ROOT}" \
    --output-dir "${WORKSPACE_ROOT}/bazel/generated" \
    --generate-rust-binaries \
    --generate-rust-libraries \
    --generate-rust-tests \
    --generate-cargo-targets

echo "Generated BUILD.bazel files in bazel/generated/"
echo "Review and merge with bazel/BUILD.bazel manually."

# Generate BUILD.bazel for root workspace
cargo2bazel generate \
    --workspace-root "${WORKSPACE_ROOT}" \
    --output-dir "${WORKSPACE_ROOT}" \
    --generate-workspace