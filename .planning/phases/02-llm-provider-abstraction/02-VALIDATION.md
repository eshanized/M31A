---
phase: 2
slug: llm-provider-abstraction
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-24
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Go testing (stdlib) + testify/assert |
| **Config file** | None — `go test ./...` |
| **Quick run command** | `make test-fast` |
| **Full suite command** | `make test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `make test-fast`
- **After every plan wave:** Run `make test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | LLM-01 | — | Interface methods present | unit | `go test ./internal/integrations/provider/... -run TestLLMProviderInterface` | ❌ W0 | ⬜ pending |
| 02-02-01 | 01 | 1 | LLM-02 | — | NVIDIA client implements ultra model, streaming, reasoning | integration | `go test ./internal/integrations/provider/nvidia/... -run TestNVIDIAClient` | ❌ W0 | ⬜ pending |
| 02-03-01 | 01 | 1 | LLM-03 | V11 | Profile merging applies provider defaults → model overrides → request | unit | `go test ./internal/integrations/provider/... -run TestProfileMerging` | ❌ W0 | ⬜ pending |
| 02-04-01 | 01 | 1 | LLM-04 | V7 | Streaming handles empty deltas, reasoning chunks, retry policy | unit | `go test ./internal/integrations/provider/... -run TestStreamingResilience` | ❌ W0 | ⬜ pending |
| 02-05-01 | 01 | 1 | LLM-05 | V2, V8, V13 | API keys from env/keychain; never in logs; redacted in diagnostics | unit | `go test ./internal/core/config/... -run TestCredentialResolution` | ❌ W0 | ⬜ pending |
| 02-06-01 | 01 | 1 | LLM-06 | V11 | FetchModels enriches ModelInfo with API metadata (context, tools, reasoning) | integration | `go test ./internal/integrations/provider/... -run TestCapabilityDetection` | ❌ W0 | ⬜ pending |
| 02-07-01 | 02 | 2 | LLM-02 | — | CLI `models list` command with table/JSON output | unit | `go test ./cmd/m31a/... -run TestModelsListCommand` | ❌ W0 | ⬜ pending |
| 02-08-01 | 02 | 2 | LLM-01 | V11 | Provider selection with --provider flag, ChatRequest override, config default, FallbackMode | unit | `go test ./internal/integrations/provider/... -run TestProviderSelection` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `internal/integrations/provider/interface_test.go` — stubs for LLM-01
- [ ] `internal/integrations/provider/nvidia/client_test.go` — stubs for LLM-02
- [ ] `internal/integrations/provider/base_client_test.go` — stubs for LLM-03 (profile merging)
- [ ] `internal/integrations/provider/sse_test.go` — stubs for LLM-04 (streaming resilience)
- [ ] `internal/core/config/loader_test.go` — stubs for LLM-05 (credential resolution)
- [ ] `internal/integrations/provider/capabilities_test.go` — stubs for LLM-06 (capability detection)
- [ ] `internal/core/types/types_test.go` — ChatResponse, ModelInfo extensions

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| NVIDIA Build live streaming with reasoning chunks visible in TUI | LLM-02 | Requires NVIDIA_API_KEY; external API dependency | Set `NVIDIA_API_KEY`, run `m31a chat "hello"`, verify streaming response with reasoning |
| Credential never persisted to config.toml or logs | LLM-05 | Requires inspecting config file and log output | Run with `--debug`, verify no API key in logs or `.m31a/config.toml` |
| Network interruption triggers retry with exponential backoff | LLM-04 | Requires network manipulation | Simulate network drop during streaming, verify retry and recovery |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending