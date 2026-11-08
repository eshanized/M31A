# M31A Root Justfile - Developer interface and workflow orchestration

set shell := ["bash", "-euo", "pipefail", "-c"]

# Default recipe: list all available commands
default:
    @just --list

# Build the complete Rust workspace in development mode
build:
    cargo build --workspace

# Build the production M31A CLI binary using Cargo release profile
release:
    cargo build --release -p m31a-cli --bin m31a

# Run the complete supported Rust test suite including doctests
test: test-fast test-doc

# Run workspace unit and integration tests using cargo-nextest
test-fast:
    cargo nextest run --workspace --all-targets

# Run documentation tests using Cargo
test-doc:
    cargo test --workspace --doc

# Format all workspace Rust source files
fmt:
    cargo fmt --all

# Check formatting across all workspace Rust source files
fmt-check:
    cargo fmt --all -- --check

# Run strict Clippy verification with -D warnings
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run the complete local verification pipeline (fmt, clippy, check, tests, doctests)
check: fmt-check lint test

# Extract and validate the canonical App Server protocol JSON schema
schema output="schemas/app_server/schema.json":
    python3 scripts/extract_protocol_schema.py {{output}}

# Run E2E benchmark harness with statistical latency and throughput analysis
bench name="bench_full_session" iterations="5":
    python3 scripts/benchmark_runner.py --name {{name}} --iterations {{iterations}} --output benchmark_results.json

# Create distributable M31A release artifacts using cargo-dist
package:
    cargo dist build --artifacts=host
