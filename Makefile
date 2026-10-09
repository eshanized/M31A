# ==============================================================================
# M31 Autonomous (M31A) — Canonical Developer & Operational Makefile
#
# "The model proposes. The runtime decides."
#
# Single-crate, high-assurance Rust autonomous software engineering runtime.
# ==============================================================================

SHELL := /usr/bin/env bash
.SHELLFLAGS := -euo pipefail -c
.DEFAULT_GOAL := help

# ── Configuration Variables ───────────────────────────────────────────────────
CARGO            ?= cargo
CARGO_BUILD_JOBS ?= 2
RUST_TEST_THREADS?= 2
CHANNEL          ?= production
PREFIX           ?=
ARGS             ?=
TMPDIR           ?= $(CURDIR)/tmp
export TMPDIR

# Terminal styling
BOLD  := $(shell tput bold 2>/dev/null || printf '')
RESET := $(shell tput sgr0 2>/dev/null || printf '')
CYAN  := $(shell tput setaf 6 2>/dev/null || printf '')
GREEN := $(shell tput setaf 2 2>/dev/null || printf '')
YELLOW:= $(shell tput setaf 3 2>/dev/null || printf '')

# ── Test Groups ───────────────────────────────────────────────────────────────
TUI_TESTS := \
	golden_tui_pty_scenario \
	tui_canonical_reduction \
	tui_first_frame_regression \
	golden_wizard_model_selection \
	onboarding_wizard_remediation \
	golden_tui_2_scenarios \
	golden_tui_agentic_workflow \
	golden_tui_conversation_first \
	phase_11_tui \
	phase_13_pty_scenarios \
	phase_13_cockpit_views \
	phase_13_framebuffer_snapshots \
	phase_13_terminal_tokens \
	phase_13_visual_review \
	phase_36_4_tui_governance \
	phase_36_6_tui_truth_projection \
	remediation_tui_approval \
	tui_3_live_cockpit \
	tui_architecture_principles \
	tui_autocomplete_repair \
	tui_runtime_integration_remediation \
	tui_runtime_state_repair \
	tui_ux_reconstruction \
	tui_workspace_v2

PTY_TESTS := \
	golden_tui_pty_scenario \
	phase_13_pty_scenarios

SMOKE_TESTS := \
	golden_tui_pty_scenario \
	tui_canonical_reduction \
	tui_first_frame_regression \
	docs_contract \
	promptos_v2_behavioral

# ── Phony Targets ─────────────────────────────────────────────────────────────
.PHONY: help bootstrap
.PHONY: fmt fmt-check check check-prod check-dev check-channels build build-dev release release-dev
.PHONY: test test-all-channels test-unit test-fast test-minimal test-tui test-pty smoke test-live
.PHONY: clippy clippy-prod clippy-dev config-guard secret-scan audit verify release-check ci
.PHONY: run tui doctor config-validate session-list
.PHONY: dist install install-dev uninstall rc-evidence verify-release
.PHONY: docs docs-open clean

# ==============================================================================
# HELP & ENVIRONMENT
# ==============================================================================

help:
	@printf "%bM31 Autonomous (M31A) — Canonical Command Interface%b\n\n" "$(BOLD)$(CYAN)" "$(RESET)"
	@printf "%bDevelopment:%b\n" "$(BOLD)" "$(RESET)"
	@printf "  %bmake bootstrap%b        Verify toolchain (Rust 1.85+, Git) & initialize directories\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake fmt%b              Format all Rust source code with cargo fmt\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake fmt-check%b        Check source formatting without modifying files\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake check%b            Type check both channels (production & development)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake check-prod%b       Type check production channel\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake check-dev%b        Type check development channel (--features development)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake check-channels%b   Assert mutual exclusion between deployment channels\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake build%b            Build debug binary (production channel)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake build-dev%b        Build debug binary with development channel\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake release%b          Build optimized release binary (production channel)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake release-dev%b      Build optimized release binary with development channel\n" "$(GREEN)" "$(RESET)"
	@printf "\n%bTesting:%b\n" "$(BOLD)" "$(RESET)"
	@printf "  %bmake test%b             Run full deterministic test suite (production channel)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake test-all-channels%bRun full deterministic test suite across both channels\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake test-unit%b        Run library unit tests (fast, bounded resources)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake test-fast%b        Alias for test-unit\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake test-minimal%b     Run minimal resource test engine script\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake test-tui%b         Run comprehensive TUI and onboarding regression suites\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake test-pty%b         Run real POSIX pseudo-terminal lifecycle & scenario tests\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake smoke%b            Run critical smoke verification (unit, contracts, PTY)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake test-live%b        Run live model inference tests (requires API_KEY_NVIDIA)\n" "$(GREEN)" "$(RESET)"
	@printf "\n%bQuality & Governance:%b\n" "$(BOLD)" "$(RESET)"
	@printf "  %bmake clippy%b           Run cargo clippy on both channels (-D warnings)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake clippy-prod%b      Run cargo clippy on production channel only\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake clippy-dev%b       Run cargo clippy on development channel only\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake config-guard%b     Run configuration authority static drift guard\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake secret-scan%b      Scan codebase for embedded credentials\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake audit%b            Run cargo audit (requires cargo-audit)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake verify%b           Run canonical 4-gate verification (fmt, check, clippy, test)\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake release-check%b    Run authoritative pre-release check script\n" "$(GREEN)" "$(RESET)"
	@printf "\n%bContinuous Integration:%b\n" "$(BOLD)" "$(RESET)"
	@printf "  %bmake ci%b               Run full CI validation pipeline matching release-gates.yml\n" "$(GREEN)" "$(RESET)"
	@printf "\n%bRuntime & Operations:%b\n" "$(BOLD)" "$(RESET)"
	@printf "  %bmake run%b              Run m31a CLI [ARGS=\"...\"]\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake tui%b              Launch interactive Ratatui cockpit [ARGS=\"...\"]\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake doctor%b           Run environmental diagnostic checks [ARGS=\"...\"]\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake config-validate%b  Validate configuration [ARGS=\"...\"]\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake session-list%b     List existing sessions [ARGS=\"...\"]\n" "$(GREEN)" "$(RESET)"
	@printf "\n%bPackaging & Distribution:%b\n" "$(BOLD)" "$(RESET)"
	@printf "  %bmake dist%b             Package release archives into dist/ [CHANNEL=production]\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake install%b          Install binary locally into PATH [CHANNEL=production PREFIX=DIR]\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake install-dev%b      Install m31a-dev locally into PATH [PREFIX=DIR]\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake uninstall%b        Uninstall local binary [CHANNEL=production PREFIX=DIR]\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake rc-evidence%b      Assemble release candidate evidence bundle into dist/\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake verify-release%b   Verify release candidate artifacts in dist/\n" "$(GREEN)" "$(RESET)"
	@printf "\n%bDocumentation & Cleanup:%b\n" "$(BOLD)" "$(RESET)"
	@printf "  %bmake docs%b             Generate rustdoc documentation\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake docs-open%b        Generate and open rustdoc documentation in browser\n" "$(GREEN)" "$(RESET)"
	@printf "  %bmake clean%b            Remove build artifacts, dist/, and tmp/\n" "$(GREEN)" "$(RESET)"
	@printf "\n%bVariables:%b\n" "$(BOLD)" "$(RESET)"
	@printf "  CARGO=%s  CARGO_BUILD_JOBS=%s  RUST_TEST_THREADS=%s  CHANNEL=%s\n" "$(CARGO)" "$(CARGO_BUILD_JOBS)" "$(RUST_TEST_THREADS)" "$(CHANNEL)"

bootstrap:
	@printf "==> %bValidating Toolchain & Environment%b\n" "$(BOLD)" "$(RESET)"
	@command -v $(CARGO) >/dev/null 2>&1 || { echo "ERROR: Cargo is not installed or not in PATH." >&2; exit 1; }
	@command -v rustc >/dev/null 2>&1 || { echo "ERROR: rustc is not installed or not in PATH." >&2; exit 1; }
	@command -v git >/dev/null 2>&1 || { echo "ERROR: git is not installed or not in PATH." >&2; exit 1; }
	@mkdir -p "$(TMPDIR)"
	@printf "  Rust toolchain: %s\n" "$$(rustc --version)"
	@printf "  Cargo:          %s\n" "$$($(CARGO) --version)"
	@printf "  Git:            %s\n" "$$(git --version)"
	@printf "  Workspace tmp:  %s\n" "$(TMPDIR)"
	@if command -v cargo-audit >/dev/null 2>&1; then \
		printf "  cargo-audit:    available\n"; \
	else \
		printf "  cargo-audit:    not installed (optional: cargo install cargo-audit)\n"; \
	fi
	@if command -v cargo-deb >/dev/null 2>&1; then \
		printf "  cargo-deb:      available\n"; \
	else \
		printf "  cargo-deb:      not installed (optional for Debian packaging)\n"; \
	fi
	@printf "==> %bEnvironment Verified%b\n" "$(GREEN)" "$(RESET)"

# ==============================================================================
# DEVELOPMENT TARGETS
# ==============================================================================

fmt:
	$(CARGO) fmt

fmt-check:
	$(CARGO) fmt --check

check: check-prod check-dev

check-prod:
	$(CARGO) check --all-targets

check-dev:
	$(CARGO) check --all-targets --features development

check-channels:
	@printf "==> %bVerifying Channel Mutual Exclusion Invariant%b\n" "$(BOLD)" "$(RESET)"
	@if ($(CARGO) check --lib --features development,production 2>&1 || true) | grep -qi "mutually exclusive"; then \
		printf "  %b[PASS] Mutual exclusion enforced: development+production was rejected%b\n" "$(GREEN)" "$(RESET)"; \
	else \
		printf "  %b[FAIL] ERROR: development+production was NOT rejected%b\n" "$(BOLD)" "$(RESET)" >&2; \
		exit 1; \
	fi

build:
	$(CARGO) build

build-dev:
	$(CARGO) build --features development

release:
	$(CARGO) build --release

release-dev:
	$(CARGO) build --release --features development

# ==============================================================================
# TESTING TARGETS
# ==============================================================================

test:
	@mkdir -p "$(TMPDIR)"
	$(CARGO) test --all-targets -j $(CARGO_BUILD_JOBS) -- --test-threads $(RUST_TEST_THREADS)

test-all-channels:
	@mkdir -p "$(TMPDIR)"
	@printf "==> %bTesting Production Channel%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --all-targets -j $(CARGO_BUILD_JOBS) -- --test-threads $(RUST_TEST_THREADS)
	@printf "==> %bTesting Development Channel%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --all-targets --features development -j $(CARGO_BUILD_JOBS) -- --test-threads $(RUST_TEST_THREADS)

test-unit:
	@mkdir -p "$(TMPDIR)"
	$(CARGO) test --lib -j $(CARGO_BUILD_JOBS) -- --test-threads $(RUST_TEST_THREADS)

test-fast: test-unit

test-minimal:
	bash scripts/test-minimal.sh --all

test-tui:
	@mkdir -p "$(TMPDIR)"
	@printf "==> %bRunning Comprehensive TUI Regression Test Suites%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test $(foreach t,$(TUI_TESTS),--test $(t)) -j $(CARGO_BUILD_JOBS) -- --test-threads $(RUST_TEST_THREADS)

test-pty:
	@mkdir -p "$(TMPDIR)"
	@printf "==> %bRunning POSIX PTY Interactive Lifecycle & Scenario Tests%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test $(foreach t,$(PTY_TESTS),--test $(t)) -j $(CARGO_BUILD_JOBS) -- --test-threads $(RUST_TEST_THREADS)

smoke:
	@mkdir -p "$(TMPDIR)"
	@printf "==> %bRunning Smoke Verification: Unit Tests%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --lib -j $(CARGO_BUILD_JOBS) -- --test-threads $(RUST_TEST_THREADS)
	@printf "==> %bRunning Smoke Verification: Critical Path & PTY Scenarios%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test $(foreach t,$(SMOKE_TESTS),--test $(t)) -j $(CARGO_BUILD_JOBS) -- --test-threads $(RUST_TEST_THREADS)

test-live:
	@mkdir -p "$(TMPDIR)"
	@if [ -z "$${API_KEY_NVIDIA:-$${NVIDIA_API_KEY:-}}" ]; then \
		printf "  %bCREDENTIAL-BLOCKED: API_KEY_NVIDIA / NVIDIA_API_KEY is not set. Live probes require credentials.%b\n" "$(YELLOW)" "$(RESET)" >&2; \
		exit 1; \
	fi
	$(CARGO) test --test live_model_e2e --features live-model-tests -- --ignored --test-threads 1

# ==============================================================================
# QUALITY & GOVERNANCE TARGETS
# ==============================================================================

clippy: clippy-prod clippy-dev

clippy-prod:
	$(CARGO) clippy --all-targets -- -D warnings

clippy-dev:
	$(CARGO) clippy --all-targets --features development -- -D warnings

config-guard:
	bash scripts/check_config_authority.sh

secret-scan:
	@printf "==> %bScanning for Embedded Credentials%b\n" "$(BOLD)" "$(RESET)"
	@if grep -rn "nvapi-[A-Za-z0-9_-]\{30\}" --include="*.rs" --include="*.toml" src/ tests/ 2>/dev/null | grep -v "test" | grep -v "dummy" | grep -v "fake"; then \
		printf "  %bERROR: Possible embedded credentials detected in repository%b\n" "$(BOLD)" "$(RESET)" >&2; \
		exit 1; \
	fi
	@if git ls-files .env 2>/dev/null | grep -q .; then \
		printf "  %bERROR: .env is tracked by git%b\n" "$(BOLD)" "$(RESET)" >&2; \
		exit 1; \
	fi
	@if git ls-files .m31a/ 2>/dev/null | grep -q .; then \
		printf "  %bERROR: .m31a/ directory is tracked by git%b\n" "$(BOLD)" "$(RESET)" >&2; \
		exit 1; \
	fi
	@printf "  %b[PASS] Secret scan: Clean%b\n" "$(GREEN)" "$(RESET)"

audit:
	@if command -v cargo-audit >/dev/null 2>&1 || $(CARGO) audit --version >/dev/null 2>&1; then \
		$(CARGO) audit; \
	else \
		printf "  %bcargo-audit is not installed. Install with: cargo install cargo-audit%b\n" "$(YELLOW)" "$(RESET)" >&2; \
		exit 1; \
	fi

# ==============================================================================
# FOCUSED REMEDIATION GATES (MSN-REMEDIATION)
# ==============================================================================

verify-runtime:
	@printf "==> %bRunning Focused Runtime Verification%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --test remediation_runtime_convergence test_unmanaged_execution_continuation_rejected -- --exact
	$(CARGO) test --test remediation_runtime_convergence test_canonical_action_protocol -- --exact

verify-authority:
	@printf "==> %bRunning Focused Authority Verification%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --test remediation_runtime_convergence test_denied_tools_omitted_and_blocked -- --exact
	$(CARGO) test --test remediation_runtime_convergence test_tool_authority_scope_role_rebinding -- --exact
	$(CARGO) test --test remediation_runtime_convergence test_scoped_tool_authority_visible_equals_executable -- --exact

verify-replan:
	@printf "==> %bRunning Focused Replan Verification%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --test remediation_runtime_convergence test_replan_authority_end_to_end -- --exact
	$(CARGO) test --test remediation_runtime_convergence test_adapt_strategy_tool_is_proposal_only -- --exact

verify-lsp:
	@printf "==> %bRunning Focused LSP Session Lifecycle Verification%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --test remediation_runtime_convergence test_persistent_lsp_session_lifecycle_e2e -- --exact
	$(CARGO) test --test remediation_runtime_convergence test_lsp_provenance_mock_and_absent -- --exact

verify-telemetry:
	@printf "==> %bRunning Focused Telemetry & Cost Provenance Verification%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --test remediation_runtime_convergence test_model_telemetry_and_cost_provenance -- --exact

verify-autonomous-eval:
	@printf "==> %bRunning Focused Autonomous Evaluation Verification%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --test remediation_runtime_convergence test_autonomous_evaluation_runner -- --exact

verify-architecture:
	@printf "==> %bRunning Focused Architecture Verification%b\n" "$(BOLD)" "$(RESET)"
	$(CARGO) test --test architecture_runtime_authority
	$(CARGO) test --test remediation_single_authority

verify-remediation: verify-runtime verify-authority verify-replan verify-lsp verify-telemetry verify-autonomous-eval verify-architecture
	@printf "\n==> %bAll Focused Remediation Gates PASSED%b\n" "$(GREEN)" "$(RESET)"

verify: fmt-check config-guard secret-scan check check-channels clippy verify-remediation test
	@printf "\n==> %bAll Canonical Verification Gates PASSED%b\n" "$(GREEN)" "$(RESET)"

release-check:
	bash scripts/release-check.sh

# ==============================================================================
# CONTINUOUS INTEGRATION TARGET
# ==============================================================================

ci: fmt-check config-guard secret-scan check check-channels clippy test
	@printf "\n==> %bCI Validation Pipeline PASSED%b\n" "$(GREEN)" "$(RESET)"

# ==============================================================================
# RUNTIME & OPERATIONS
# ==============================================================================

run:
	$(CARGO) run -- $(ARGS)

tui:
	$(CARGO) run -- tui $(ARGS)

doctor:
	$(CARGO) run -- doctor $(ARGS)

config-validate:
	$(CARGO) run -- config validate $(ARGS)

session-list:
	$(CARGO) run -- session list $(ARGS)

# ==============================================================================
# PACKAGING & DISTRIBUTION
# ==============================================================================

dist:
	bash scripts/build-release.sh --channel $(CHANNEL)

install:
	bash scripts/install-local.sh --channel $(CHANNEL) $(if $(PREFIX),--prefix $(PREFIX),)

install-dev:
	bash scripts/install-local.sh --channel development $(if $(PREFIX),--prefix $(PREFIX),)

uninstall:
	bash scripts/uninstall.sh --channel $(CHANNEL) $(if $(PREFIX),--prefix $(PREFIX),)

rc-evidence:
	bash scripts/rc-evidence.sh

verify-release:
	bash scripts/verify-release.sh

# ==============================================================================
# DOCUMENTATION & CLEANUP
# ==============================================================================

docs:
	$(CARGO) doc --no-deps

docs-open:
	$(CARGO) doc --no-deps --open

clean:
	$(CARGO) clean
	rm -rf dist "$(TMPDIR)"
