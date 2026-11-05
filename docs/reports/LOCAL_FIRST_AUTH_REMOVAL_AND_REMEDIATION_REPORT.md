# M31A — Local-First Authentication Removal & Complete Production Remediation Report

**Date:** 2026-11-05  
**Version:** 0.1.13  
**Status:** COMPLETE  

---

## Executive Summary

M31A has been transformed into a strictly local-first, single-user autonomous coding agent. All obsolete first-party user authentication, user accounts, registration, login/logout screens, pairing tokens, M31A access tokens, refresh-token lifecycles, and first-party authentication middleware have been completely removed. Security enforcement has transitioned to OS-level boundaries: restrictive filesystem and Unix domain socket permissions (`0600`), strict loopback-only network binding (`127.0.0.1` / `::1`), host and origin validation, workspace trust, sandbox policies, ExecPolicy, Guardian gating, and explicit host-mediated user approvals.

Simultaneously, all outstanding security and implementation findings identified in the audit (P0 through P2) have been remediated, verified, and accompanied by comprehensive regression tests. Every retained feature is genuinely implemented without synthetic success fabrication.

---

## Audit Finding & Remediation Status Matrix

| ID | Component / Area | Description | Status | Verification Summary |
|---|---|---|---|---|
| **PART-01** | First-Party Auth | Complete removal of M31A login, accounts, pairing, token lifecycle, and auth middleware | `FIXED_AND_VERIFIED` | Deleted `oauth_flow.rs`, `workload_identity.rs`, `pairing.rs`; removed CLI login/logout commands; verified 278 core + 38 app-server tests pass |
| **PART-02** | External Provider Secrets | Encrypted vault migration from `auth.vault` to `provider.vault`; preservation of AI provider keys | `FIXED_AND_VERIFIED` | `LocalSecretsNamespace::Provider` added; migration purges obsolete account tokens; provider keys preserved and verified |
| **PART-03** | P0: Plugin Consent | Removal of model-controlled `user_confirmed` spoofing; introduction of host-only `HostPluginApproval` | `FIXED_AND_VERIFIED` | Model-supplied `user_confirmed` removed; cryptographically bound `HostPluginApproval` (120s TTL, single-use); adversarial test verified |
| **PART-04** | P0: Tool Dispatcher | Centralized authoritative `ToolEffect` policy engine; fail-closed unknown tools; strict Plan Mode; ExecPolicy override defense | `FIXED_AND_VERIFIED` | Unknown tools fail closed; Plan Mode strictly read-only; ExecPolicy hard denials cannot be overridden; cancellation token selects around execution |
| **PART-05** | P1: Extension Plugin ABI | Asymmetric Ed25519 signature verification; removal of trait object fallback; versioned C ABI `m31a_plugin_v1` | `FIXED_AND_VERIFIED` | `ring::signature::ED25519` verifies SHA-256 binary digests; dynamic trait object fallback deleted; stable C ABI verified |
| **PART-06** | P1: Cloud Agent Tasks | Elimination of fabricated `TaskStatusEnum::Completed`; genuine execution and exit code propagation | `FIXED_AND_VERIFIED` | Real process execution via `tokio::process::Command`; exit code 42 returns `Failed`; zero-exit returns `Completed` |
| **PART-07** | P1: Image Generation | Elimination of synthetic dummy PNG fallback; require configured external AI provider or explicit offline preview | `FIXED_AND_VERIFIED` | Failed or missing provider triggers typed error; offline preview explicitly tags outputs with `[SYNTHETIC TEST IMAGE]` |
| **PART-08** | P1: Browser Security | Host-context session isolation; SSRF and DNS-rebinding protection against private networks and metadata | `FIXED_AND_VERIFIED` | Context `session_id` injected into all browser executions; private IP ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 127.0.0.1, 169.254.169.254, IPv4-mapped IPv6) blocked |
| **PART-09** | P2: Realtime Voice | Genuine audio stream establishment required for `Connected` status | `FIXED_AND_VERIFIED` | Device enumeration sets `Connecting`; active stream establishment or SDP handshake required to reach `Connected` |
| **PART-10** | P2: Memory & Learning | Hybrid semantic embedding retrieval & DPO trajectory verification evidence grounding | `FIXED_AND_VERIFIED` | `search_memories_hybrid` integrated in tool registry; trajectory recorder verifies diagnostic errors and test proof |
| **PART-11** | P2: Feature Flags | Removal of obsolete auth flags; unified feature flag evaluation | `FIXED_AND_VERIFIED` | `OAuthDeviceFlow` and `WorkloadIdentity` purged; all feature flag tests pass |
| **PART-12** | P2: Mutation Testing | Mutants failing compilation classified as `CompileFailed` (unviable), never counted as killed | `FIXED_AND_VERIFIED` | Compilation error detection verified; zero compiler errors classified as killed; unit tests pass |
| **PART-13** | P2: Bazel Rules | Accurate documentation of Bazel rules wrapping host Cargo toolchain without false hermetic claims | `FIXED_AND_VERIFIED` | `m31a_rules.bzl` and `BUILD.bazel` clearly document non-hermetic local Cargo execution |

---

## Detailed Remediation Actions

### Part 1 & 2: Local-First Application Architecture & Secret Vault Migration
1. **Modules Deleted:**
   - `crates/m31a-core/src/oauth_flow.rs`
   - `crates/m31a-core/src/workload_identity.rs`
   - `crates/m31a-app-server/src/pairing.rs`
2. **App Server Simplification (`crates/m31a-app-server`):**
   - Removed M31A pairing tokens, access tokens, token revocation, stream tickets, and handshake negotiations.
   - Retained explicit resource validation: RPC methods validate that requests supply real resource IDs, thread IDs match active turns, and checkpoints resolve to genuine threads.
   - HTTP transport enforces loopback binding (`127.0.0.1` / `::1`), rejects wildcard or public interface bindings, and validates Host/Origin headers to defend against DNS rebinding and malicious browser origins.
   - Unix Domain Socket binds with `0600` permissions and checks for symlink attacks before binding (`crates/m31a-core/src/uds.rs`).
3. **Encrypted Vault Migration (`crates/m31a-core/src/secrets.rs`):**
   - Introduced `LocalSecretsNamespace::Provider` (`provider.vault`).
   - Implemented automated migration from `auth.vault` to `provider.vault`: preserves external provider keys (such as `NVIDIA_API_KEY`) while purging obsolete account tokens (`OAUTH_ACCESS_TOKEN`, `OAUTH_REFRESH_TOKEN`, etc.).
   - Updated CLI commands to remove `login`, `logout`, and `pair` while preserving external provider configuration.

### Part 3: P0 — Plugin Consent Vulnerability Fix
1. **Vulnerability Remediated:** The model previously supplied `user_confirmed: true` or `_trusted_approval` in JSON arguments to install plugins or extensions without real user consent.
2. **Implementation (`crates/m31a-tools/src/plugin_install.rs`, `crates/m31a-tools/src/registry.rs`):**
   - Completely removed `user_confirmed` from `RequestPluginInstallArgs`.
   - Introduced `HostPluginApproval` containing tool ID, action (`Install` or `Enable`), optional session/turn/call ID bindings, issuance timestamp, and a strict 120-second TTL.
   - Only the trusted host UI/CLI or local approval handler can issue `HostPluginApproval`.
   - Tool execution consumes approvals atomically (single-use). Model JSON inputs cannot spoof approvals.

### Part 4: P0 — Authoritative Tool Dispatcher
1. **Implementation (`crates/m31a-agent-engine/src/dispatcher.rs`):**
   - Unknown tools fail closed (`!self.tools.is_known_tool(fn_name)`).
   - Plan Mode strictly enforces read-only behavior; any non-read-only tool call is rejected immediately.
   - ExecPolicy hard denial cannot be overridden by user approvals.
   - Full `ToolEffect` policy engine: `ReadOnly`, `WorkspaceMutation`, `ExternalSideEffect`, and `PrivilegedOperation`.
   - Under `ApprovalPolicy::Never`, operations requiring approval fail closed.
   - `ApproveForMe` requires evaluation by Guardian; Guardian errors never silently approve.
   - Wrapped tool execution in `tokio::select!` on cancellation token for immediate cancellation propagation.

### Part 5: P1 — Plugin System & Cryptographic Signature Verification
1. **Implementation (`crates/m31a-extension-api`):**
   - Added `ring = "0.17"` and `hex = "0.4"`.
   - Implemented Ed25519 asymmetric signature verification (`ring::signature::ED25519`) against SHA-256 digests and raw binary payloads.
   - Removed unstable dynamic trait object entrypoint fallback (`Symbol<unsafe extern "C" fn() -> *mut dyn M31APlugin>`).
   - Standardized on stable versioned C ABI entrypoint: `m31a_plugin_v1`.

### Part 6: P1 — Cloud Agent Tasks
1. **Implementation (`crates/m31a-cloud-tasks`):**
   - Removed fabricated `TaskStatusEnum::Completed` in fallback client.
   - Implemented real process execution via `tokio::process::Command`.
   - Captures real exit codes and stdout/stderr; non-zero exit codes (e.g. 42) return `TaskStatusEnum::Failed`. Empty or missing input returns `TaskStatusEnum::Failed`.

### Part 7: P1 — Image Generation
1. **Implementation (`crates/m31a-tools/src/image_gen.rs`):**
   - Removed synthetic PNG generation fallback from the production path.
   - Missing or failing external AI image providers return explicit typed errors.
   - Synthetic generation is retained solely for explicit offline preview/test mode (`offline_preview: true` or `M31A_OFFLINE_PREVIEW=1`), and outputs are labeled with `[SYNTHETIC TEST IMAGE]`.

### Part 8: P1 — Browser Security & Session Isolation
1. **Implementation (`crates/m31a-tools/src/browser.rs`, `crates/m31a-browser/src/session.rs`, `crates/m31a-tools/src/registry.rs`):**
   - Removed model-controlled `session_id` from browser tool schemas.
   - Injected authoritative session identity from host context (`ToolRegistry::session_id`).
   - Implemented SSRF and DNS-rebinding protection blocking private IP ranges (RFC 1918 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16), loopback (127.0.0.1, `::1`, `localhost`), link-local (169.254.0.0/16), IPv4-mapped IPv6, and cloud metadata services.

### Part 9: P2 — Realtime Voice Stream Establishment
1. **Implementation (`crates/m31a-core/src/realtime_voice.rs`):**
   - Hardware device enumeration sets session status to `Connecting`.
   - `Connected` status is only reported when the active audio stream or SDP handshake is genuinely established.

### Part 10: P2 — Memory & Continuous Learning
1. **Implementation (`crates/m31a-core`, `crates/m31a-tools/src/registry.rs`):**
   - Connected `search_memories_hybrid` to `search_memories` tool in registry, enabling both lexical and dense vector semantic retrieval.
   - Verified continuous learning trajectory recorder enforces grounded diagnostic logs and compiler evidence.

### Part 11: P2 — Feature Flags Unification
1. **Implementation (`crates/m31a-core/src/features.rs`, `crates/m31a-features`):**
   - Purged obsolete auth flags (`OAuthDeviceFlow`, `WorkloadIdentity`).
   - Verified unified feature flag evaluation across workspace crates.

### Part 12: P2 — Mutation Testing Classification
1. **Implementation (`crates/m31a-verify/src/integrity.rs`):**
   - Mutants failing compilation are categorized as `MutationStatus::CompileFailed` and are never counted as killed mutants.

### Part 13: P2 — Bazel Build Integrity
1. **Implementation (`bazel/rules/m31a_rules.bzl`, `bazel/BUILD.bazel`):**
   - Added clear notices clarifying that Bazel targets wrap the host Cargo toolchain with non-hermetic local execution (`local = 1`, `no-sandbox = 1`).

---

## Workspace Verification Results

All verification suites were run against the entire workspace with zero regressions:

1. **Code Formatting Check:**
   ```bash
   cargo fmt --all -- --check
   ```
   *Result:* PASSED (exit code 0)

2. **Full Workspace Compilation & Targets Check:**
   ```bash
   cargo check --workspace --all-targets
   ```
   *Result:* PASSED (exit code 0 across all 40+ workspace crates)

3. **Workspace Clippy Linting (Zero Warnings Enforced):**
   ```bash
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```
   *Result:* PASSED (exit code 0, 0 warnings)

4. **Full Workspace Test Suite:**
   ```bash
   cargo test --workspace
   ```
   *Result:* PASSED (exit code 0, all tests passed across all crates)

### Key Test Count Highlights:
- `m31a-core`: 278 unit tests + 4 integration tests PASSED
- `m31a-tools`: 112 unit tests + 4 integration tests PASSED
- `m31a-app-server`: 38 unit & security integration tests PASSED
- `m31a-agent-engine`: 81 unit & turn loop integration tests PASSED
- `m31a-browser`: 18 unit & session integration tests PASSED
- `m31a-code-mode`: 42 unit & supervisor integration tests PASSED
- `m31a-verify`: 69 unit & evidence integrity tests PASSED
- `m31a-cloud-tasks`: 7 unit & process lifecycle tests PASSED
- `m31a-extension-api`: 9 unit & cryptographic signature tests PASSED
- `m31a-features`: 10 unit & rollout tests PASSED
- `m31a-cli`: 25 unit & command integration tests PASSED
