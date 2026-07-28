# Phase 5: Fix Wiring Issues - Context

**Gathered:** 2026-07-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Resolve all 50 wiring issues (W01-W50) from the wiring audit (WIRING_ISSUES.md). Wire in dead producers that should be connected, delete confirmed dead code, fix config drift, complete partial registrations, and close message routing gaps. All CI checks must remain clean after each batch.

</domain>

<decisions>
## Implementation Decisions

### Fix Strategy
- **D-01:** Wire in everything — dead producers that have intended consumers get connected, not deleted. Applies to W02 (rollback), W03 (InstructionsSource), W04-W06 (metrics), W07 (StreamChunkMsg), W18-W19 (narrative events).
- **D-02:** Delete confirmed dead code — items with zero references OR zero intended consumers. Applies to W08 (arbitrager, field never assigned), W09 (pkg/errors), W10 (logging package), W11 (screens/ dir), W16-W17 (dead constants/errors), W20-W22 (workflow placeholders, no callers), W23 (exec constants), W25 (StreamingMockProvider, never imported), W30-W32 (layout primitives), W33-W43 (test mocks/fixtures), W44-W50 (unused methods/interfaces/constants), W47 (TokenEstimator, never used as parameter).

### Batching Strategy
- **D-03:** Batch by severity per the audit ratings: Critical (W01, W02) → High (W03-W15) → Medium (W16-W32) → Low (W33-W50). Four batches, one commit per issue (50 atomic commits).

### Test Strategy
- **D-04:** Integration tests — verify wiring works end-to-end. Test that InstructionsSource injects into LLM context, rollback.SoftReset is called on bisect failure, metrics methods record actual data, StreamChunkMsg renders in REPL. Each wiring fix has a test that fails on current code and passes after fix.

### Critical Issues
- **D-05:** W01 — Full config merge in one pass. Add all ~30 missing fields across 6 sections (Narrative, ModelCapabilities, Prompts, Templates, ProviderConfig, FeaturesConfig, ToolsConfig, CompactionConfig, VerifyConfig) to merge.go. Match the existing merge pattern field-by-field.
- **D-06:** W02 — Rollback integration via SoftReset. After failed bisect identifies offender, call rollback.SoftReset() to undo bad commits. Less aggressive than HardReset, preserves non-offending work.

### High-Severity Issues
- **D-07:** W03 — Auto-inject InstructionsSource. Register alongside DateTimeSource, EnvironmentSource, GitSource in engine.go:665-669. Project instruction files automatically injected into LLM context.
- **D-08:** W04-W06 — Wire all three metrics methods. RecordLLMInteractionWithPrompt in engine.go after each LLM call. RecordHealDuration and RecordHealLoop in heal paths.
- **D-09:** W07 — Wire StreamChunkMsg into REPL. Handle in app_update.go Update() to stream LLM responses into the REPL display.
- **D-10:** W12 — Fix MetricsTool case: change `case "MetricsTool"` to `case "Metrics"` in permissions.go:681.
- **D-11:** W13 — Add missing `case "Git"` branch in extractFromParams for permission extraction.
- **D-12:** W14 — Add M31A_NVIDIA_API_KEY to usage.go help output.
- **D-13:** W15 — Fix knownConfigKeys: replace Go type literal strings (`"types.ProviderOpenRouter"`) with actual config key strings.

### Medium-Severity Issues
- **D-14:** W18-W19 — Add WorkflowEvent methods (EventType(), EventData()) to AgentSwitchMsg and DecisionsSnapshotMsg so narrative engine can classify them.
- **D-15:** W23 — Remove all ~23 dead constants from exec/bash.go. Only BashKillGracePeriod and BashWaitTimeout are used.
- **D-16:** W24 — Consolidate duplicate isReservedIP from websearch.go into canonical IsReservedIP from ip_filter.go. Remove the duplicate.
- **D-17:** W26-W27 — Full Zen provider parity. Add retry loop with exponential backoff (matching OpenRouter/NVIDIA) and credit detection via HandleChatHTTPErrorWithCredits.
- **D-18:** W29 — Add FallbackPriority validation to config_validate.go. Check that each entry is a valid provider name.
- **D-19:** W20-W22 — Wire or remove workflow placeholders. ModelForPhase/ProviderForPhase (W20) return empty/nil — either implement or remove. BuildAgentSwitchMessage (W21) and IsPlanComplete (W22) — either wire callers or remove.

### Agent's Discretion
- Exact test helper design (integration test structure, table-driven vs individual)
- Whether a fix needs additional related tests beyond the mandatory regression test
- Exact merge field order in config_merge.go (match existing alphabetical or logical grouping)
- Whether to add FallbackPriority validation as a warning or hard error
- Order of removal within each severity batch

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Wiring Audit
- `WIRING_ISSUES.md` — Full audit report with all 50 wiring issue descriptions, file locations, categories, severity, and evidence

### Architecture & Structure
- `.planning/codebase/ARCHITECTURE.md` — System overview, component responsibilities, data flow (shows where wiring connects)
- `.planning/codebase/STRUCTURE.md` — Package layout, key file locations
- `.planning/codebase/INTEGRATIONS.md` — External integrations, provider details, metrics system
- `.planning/codebase/CONCERNS.md` — Known technical debt and fragile areas

### Key Source Files (referenced by wiring issues)
- `internal/core/config/merge.go` — Config merge (W01)
- `internal/engine/rollback/rollback.go` — Rollback package (W02)
- `internal/integrations/context/sources.go` — InstructionsSource (W03)
- `internal/integrations/metrics/collector.go` — Metrics methods (W04-W06)
- `internal/ui/tui/tuitypes/tuitypes.go` — StreamChunkMsg (W07)
- `internal/tools/permissions.go` — Permission extraction (W12-W13)
- `cmd/m31a/usage.go` — Help output (W14)
- `internal/core/config/config_validate.go` — Config validation (W15, W29)
- `internal/engine/workflow/agent_switch.go` — AgentSwitchMsg, placeholders (W18-W22)
- `internal/tools/exec/bash.go` — Dead constants (W23)
- `internal/tools/search/websearch.go` — Duplicate isReservedIP (W24)
- `internal/integrations/provider/zen/client.go` — Zen provider (W26-W27)

### Project Config
- `AGENTS.md` — Build commands, code style, conventional commits, gotchas
- `Makefile` — Build/test/lint targets (make check, make test, make lint)

### Prior Phase Context
- `.planning/phases/04-audit-and-remove-unused-irrelevant-code-from-m31a-codebase/04-CONTEXT.md` — Phase 4 decisions (dead code removal patterns)
- `.planning/phases/01-audit-fixes/01-CONTEXT.md` — Phase 1 decisions (one commit per fix, strict literal)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `internal/engine/rollback/rollback.go` — SoftReset, HardReset, SafeReset, Chain functions ready to wire
- `internal/integrations/context/sources.go` — InstructionsSource already implemented, just needs registration
- `internal/integrations/metrics/collector.go` — RecordLLMInteractionWithPrompt, RecordHealDuration, RecordHealLoop implemented, just need callers
- `internal/ui/tui/tuitypes/tuitypes.go` — StreamChunkMsg type defined, needs Update() handler
- `internal/engine/workflow/agent_switch.go` — AgentSwitchMsg, BuildAgentSwitchMessage, IsPlanComplete implemented, need wiring or removal
- `internal/integrations/provider/zen/client.go` — Zen client, needs retry and credit detection added

### Established Patterns
- Provider retry pattern: `const maxRetries = 2` with exponential backoff on `IsRetryable()` errors (see openrouter/client.go, nvidia/client.go)
- Context source registration: engine.go:665-669 registers DateTime, Environment, Git — follow same pattern for Instructions
- Permission extraction: extractFromParams switch/case per tool name in permissions.go:597-687
- Config merge: merge.go field-by-field copy pattern for each config section
- TUI message handling: app_update.go switch/case on tea.Msg types

### Integration Points
- `internal/engine/workflow/engine.go` — RunPhase, heal paths, LLM call sites (W02, W04-W06, W07)
- `internal/ui/tui/app_update.go` — Update() message routing (W07)
- `internal/tools/permissions.go` — Tool permission extraction (W12-W13)
- `internal/core/config/merge.go` — Config merge logic (W01)
- `internal/integrations/provider/zen/client.go` — Zen API client (W26-W27)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — fixes are driven by the wiring audit report. Each issue has clear evidence of the wiring gap and the expected vs actual behavior.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 5-Fix Wiring Issues*
*Context gathered: 2026-07-29*
