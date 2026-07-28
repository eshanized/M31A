# Phase 5: Fix Wiring Issues - Research

**Researched:** 2026-07-29
**Domain:** Codebase wiring audit — dead code removal, config merge, provider parity, permission extraction, narrative events, rollback integration
**Confidence:** HIGH

## Summary

This phase resolves 50 wiring issues (W01-W50) from a comprehensive codebase audit. The issues span 6 categories: never-consumed producers (A, 33 items), never-satisfied consumers (B, 1), partial registrations (C, 5), config/env drift (D, 4), interface implementation orphans (E, 1), and message/event routing gaps (F, 5). The research confirms all issues are genuine wiring gaps — the code compiles and passes lint, but functional connections are missing.

**Primary recommendation:** Follow the established merge/registration/handler patterns exactly — the codebase has consistent idioms for each wiring category. Wire first (D-01), delete confirmed dead code second (D-02), batch by severity per D-03.

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Wire in everything — dead producers that have intended consumers get connected, not deleted. Applies to W02 (rollback), W03 (InstructionsSource), W04-W06 (metrics), W07 (StreamChunkMsg), W08 (arbitrager), W18-W19 (narrative events), W20-W22 (workflow placeholders), W25 (StreamingMockProvider), W47 (TokenEstimator).
- **D-02:** Delete confirmed dead code with zero references — packages, functions, constants, test utilities that have no intended consumer. Applies to W09 (pkg/errors), W10 (logging package), W11 (screens/ dir), W16-W17 (dead constants/errors), W23 (exec constants), W30-W32 (layout primitives), W33-W43 (test mocks/fixtures), W44-W50 (unused methods/interfaces/constants).
- **D-03:** Batch by severity per the audit ratings: Critical (W01, W02) → High (W03-W15) → Medium (W16-W32) → Low (W33-W50). Four batches, one commit per issue (50 atomic commits).
- **D-04:** Integration tests — verify wiring works end-to-end. Test that InstructionsSource injects into LLM context, rollback.SoftReset is called on bisect failure, metrics methods record actual data, StreamChunkMsg renders in REPL.
- **D-05:** W01 — Full config merge in one pass. Add all ~30 missing fields across 6 sections (Narrative, ModelCapabilities, Prompts, Templates, ProviderConfig, FeaturesConfig, ToolsConfig, CompactionConfig, VerifyConfig) to merge.go. Match the existing merge pattern field-by-field.
- **D-06:** W02 — Rollback integration via SoftReset. After failed bisect identifies offender, call rollback.SoftReset() to undo bad commits.
- **D-07:** W03 — Auto-inject InstructionsSource. Register alongside DateTimeSource, EnvironmentSource, GitSource in engine.go:665-669.
- **D-08:** W04-W06 — Wire all three metrics methods. RecordLLMInteractionWithPrompt in engine.go after each LLM call. RecordHealDuration and RecordHealLoop in heal paths.
- **D-09:** W07 — Wire StreamChunkMsg into REPL. Handle in app_update.go Update() to stream LLM responses into the REPL display.
- **D-10:** W12 — Fix MetricsTool case: change `case "MetricsTool"` to `case "Metrics"` in permissions.go:681.
- **D-11:** W13 — Add missing `case "Git"` branch in extractFromParams for permission extraction.
- **D-12:** W14 — Add M31A_NVIDIA_API_KEY to usage.go help output.
- **D-13:** W15 — Fix knownConfigKeys: replace Go type literal strings (`"types.ProviderOpenRouter"`) with actual config key strings.
- **D-14:** W18-W19 — Add WorkflowEvent methods (EventType(), EventData()) to AgentSwitchMsg and DecisionsSnapshotMsg so narrative engine can classify them.
- **D-15:** W23 — Remove all ~23 dead constants from exec/bash.go. Only BashKillGracePeriod and BashWaitTimeout are used.
- **D-16:** W24 — Consolidate duplicate isReservedIP from websearch.go into canonical IsReservedIP from ip_filter.go. Remove the duplicate.
- **D-17:** W26-W27 — Full Zen provider parity. Add retry loop with exponential backoff (matching OpenRouter/NVIDIA) and credit detection via HandleChatHTTPErrorWithCredits.
- **D-18:** W29 — Add FallbackPriority validation to config_validate.go. Check that each entry is a valid provider name.
- **D-19:** W20-W22 — Wire or remove workflow placeholders. ModelForPhase/ProviderForPhase (W20) return empty/nil — either implement or remove. BuildAgentSwitchMessage (W21) and IsPlanComplete (W22) — either wire callers or remove.

### the agent's Discretion
- Exact test helper design (integration test structure, table-driven vs individual)
- Whether a fix needs additional related tests beyond the mandatory regression test
- Exact merge field order in config_merge.go (match existing alphabetical or logical grouping)
- Whether to add FallbackPriority validation as a warning or hard error
- Order of removal within each severity batch

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| W01 | Config merge: ~30 missing fields across 6 sections | Merge pattern documented, all missing fields identified in types.go |
| W02 | Rollback integration via SoftReset on bisect failure | rollback.go API documented, engine.go integration point identified |
| W03 | InstructionsSource registration | Registration pattern at engine.go:665-669, source already implemented |
| W04-W06 | Metrics wiring (RecordLLMInteractionWithPrompt, RecordHealDuration, RecordHealLoop) | Existing callers identified, insertion points documented |
| W07 | StreamChunkMsg handling in Update() | TUI handler pattern documented, msg type alias confirmed |
| W12-W13 | Permission extraction fixes (MetricsTool→Metrics, add Git case) | Switch/case structure documented, tool names confirmed |
| W14 | NVIDIA_API_KEY in help output | usage.go pattern documented |
| W15 | knownConfigKeys fix | Go type literal strings identified, correct key strings documented |
| W18-W19 | WorkflowEvent methods for AgentSwitchMsg/DecisionsSnapshotMsg | Existing event methods pattern documented in engine_event_methods.go |
| W23 | Dead exec constants removal | Used constants identified (BashKillGracePeriod, BashWaitTimeout only) |
| W24 | Duplicate isReservedIP consolidation | Canonical IsReservedIP in ip_filter.go, duplicate in websearch.go identified |
| W26-W27 | Zen provider retry + credit detection | OpenRouter/NVIDIA retry pattern documented, HandleChatHTTPErrorWithCredits API documented |
| W29 | FallbackPriority validation | validateConfig pattern documented, provider name constants from types package |
| W20-W22 | Workflow placeholders (ModelForPhase, ProviderForPhase, BuildAgentSwitchMessage, IsPlanComplete) | All dead code, no callers outside tests |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Config merge (W01) | Core/Config | — | merge.go is the single entry point for overlay merging |
| Rollback integration (W02) | Engine/Workflow | Engine/Bisect | Rollback called after bisect identifies offender in execute.go |
| InstructionsSource (W03) | Engine/Workflow | Integrations/Context | Engine registers context sources at init time |
| Metrics wiring (W04-W06) | Engine/Workflow | Integrations/Metrics | Engine has the heal paths and LLM call sites |
| StreamChunkMsg (W07) | UI/TUI | Engine/Workflow | TUI Update() dispatches all messages, engine emits |
| Permission extraction (W12-W13) | Tools | — | permissions.go is self-contained |
| Config help (W14) | CLI/Entry | — | usage.go prints env vars |
| Config validation (W15, W29) | Core/Config | — | validateConfig in config_validate.go |
| Narrative events (W18-W19) | Engine/Workflow | Engine/Narrative | Messages implement WorkflowEvent interface |
| Dead code removal (W23, W30-W50) | Various | — | Delete from wherever defined |
| IP dedup (W24) | Tools/Search | — | websearch.go imports from ip_filter.go |
| Zen parity (W26-W27) | Integrations/Provider | — | Follow OpenRouter/NVIDIA pattern |
| Workflow placeholders (W20-W22) | Engine/Workflow | — | Dead code, remove |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Go stdlib | 1.25+ | All wiring | Project requirement (CGO_ENABLED=0) |
| Bubble Tea | v0.25+ | TUI message dispatch | Established Elm architecture pattern |
| go-toml | — | Config parsing | Existing dependency |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `internal/core/errors` | — | Sentinel errors | All error returns |
| `internal/core/types` | — | Shared type vocabulary | All inter-package communication |
| `internal/integrations/provider` | — | BaseClient, IsRetryable | Provider wiring |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Field-by-field merge | Reflection-based merge | Current pattern is explicit and type-safe; reflection would be fragile |
| SoftReset for rollback | HardReset | SoftReset preserves non-offending work (D-06 decision) |
| HandleChatHTTPError | HandleChatHTTPErrorWithCredits | WithCredits adds 402→ErrNoCredits mapping (D-17 decision) |

**Installation:** No new packages needed — all fixes use existing dependencies.

## Package Legitimacy Audit

No external packages are installed in this phase. All fixes operate on existing code within the project.

## Architecture Patterns

### Pattern 1: Config Merge Field-by-Field
**What:** Each config section has a dedicated merge helper that copies non-zero overlay values to base.
**When to use:** Whenever a config section has fields that users can override via TOML.
**Example:**
```go
// Source: internal/core/config/merge.go:99-110
func (h mergeHelper) mergeProviderConfig(base, overlay *ProviderConfig, prefix string) {
    h.stringField(&base.Default, &overlay.Default, prefix+".default")
    h.boolField(&base.AutoFallback, &overlay.AutoFallback, prefix+".auto_fallback")
    // ... field-by-field for each ProviderConfig field
}
```

### Pattern 2: Context Source Registration
**What:** Context sources are registered in engine.go as arguments to `ctxsrc.NewRegistry()`.
**When to use:** Adding a new context source that injects data into LLM context.
**Example:**
```go
// Source: internal/engine/workflow/engine.go:665-669
contextRegistry: ctxsrc.NewRegistry(
    ctxsrc.DateTimeSource{},
    ctxsrc.EnvironmentSource{WorkDir: opts.WorkDir},
    ctxsrc.GitSource{WorkDir: opts.WorkDir},
),
// W03: Add ctxsrc.InstructionsSource{ProjectRoot: projectRoot, WorkDir: opts.WorkDir}
```

### Pattern 3: Provider Retry Loop
**What:** `const maxRetries = 2` with exponential backoff on `IsRetryable()` errors.
**When to use:** Any provider ChatCompletionStream method.
**Example:**
```go
// Source: internal/integrations/provider/openrouter/client.go:136-156
func (c *Client) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
    const maxRetries = 2
    for attempt := 0; attempt <= maxRetries; attempt++ {
        iter, err := c.doChatStream(ctx, req)
        if err == nil { return iter, nil }
        if attempt < maxRetries && provider.IsRetryable(err) {
            delay := time.Duration(1<<uint(attempt)) * time.Second
            select {
            case <-ctx.Done(): return nil, ctx.Err()
            case <-time.After(delay): continue
            }
        }
        return nil, err
    }
    return nil, fmt.Errorf("max retries exceeded")
}
```

### Pattern 4: Narrative WorkflowEvent Interface
**What:** Message types implement `EventType() string` and `EventData() map[string]interface{}`.
**When to use:** Any message emitted by the workflow engine that the narrative engine should classify.
**Example:**
```go
// Source: internal/engine/workflow/engine_event_methods.go:7-13
func (m TaskStartMsg) EventType() string { return "task_start" }
func (m TaskStartMsg) EventData() map[string]interface{} {
    return map[string]interface{}{
        "description": m.Task.Description,
        "task_id":     m.Task.ID,
    }
}
```

### Pattern 5: TUI Message Dispatch
**What:** `Update()` in app_update.go has a switch/case on `tea.Msg` types, each delegating to a handler function.
**When to use:** Adding handling for a new message type in the TUI.
**Example:**
```go
// Source: internal/ui/tui/app_update.go:93-101
case StreamMsg:
    _, cmd := handleStreamMsg(m, msg)
    cmds = append(cmds, cmd)
case StreamDoneMsg:
    _, cmd := handleStreamDoneMsg(m, msg)
    cmds = append(cmds, cmd)
// W07: Add case StreamChunkMsg: with handler that streams into REPL
```

### Pattern 6: Permission Extract Switch/Case
**What:** `extractFromParams` in permissions.go has a switch/case per tool name, extracting human-readable strings from params.
**When to use:** Any tool that requires permission prompts.
**Example:**
```go
// Source: internal/tools/permissions.go:597-687
func extractFromParams(toolName string, params map[string]any) string {
    switch toolName {
    case "Bash":
        if cmd, ok := params["command"].(string); ok { return cmd }
    case "FileRead":
        if path, ok := params["path"].(string); ok { return fmt.Sprintf("read %s", path) }
    // W13: Add case "Git": extracting repo/action from params
    // W12: Change case "MetricsTool": to case "Metrics":
    }
}
```

### Anti-Patterns to Avoid
- **Hand-rolling config merge with reflection:** Use the explicit `mergeHelper` methods; reflection is fragile and type-unsafe
- **Skipping `handleKey` in permission extraction:** Every registered tool needs a case in `extractFromParams`
- **Using `HandleChatHTTPError` instead of `HandleChatHTTPErrorWithCredits`:** The latter handles 402→ErrNoCredits for providers that support credit detection
- **Adding retry without `IsRetryable()` check:** Only transient errors (500, 502, 503, connection reset) should trigger retry

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Config field merging | reflection-based merge | `mergeHelper.stringField/boolField/intField` | Type-safe, explicit, handles zero-value overrides |
| Provider retry | custom retry logic | `provider.IsRetryable()` + exponential backoff | Handles all transient error types consistently |
| IP filtering | custom IP checks | `search.IsReservedIP()` from ip_filter.go | Canonical implementation covers all RFC 5735 ranges |
| HTTP error handling | switch on status codes | `HandleChatHTTPErrorWithCredits()` | Handles 401, 402, 429, 503 consistently |
| Context source injection | manual LLM prompt construction | `ctxsrc.NewRegistry()` registration | Automatic injection into all LLM contexts |
| Narrative event classification | custom event routing | `WorkflowEvent` interface (EventType/EventData) | Bridge pattern converts to RawEvent automatically |

**Key insight:** The codebase already has all the building blocks for each wiring fix. The work is connecting existing implementations to their intended consumers, not building new functionality.

## Common Pitfalls

### Pitfall 1: Config Merge Missing Fields
**What goes wrong:** Users set fields in project-level TOML but they silently have no effect.
**Why it happens:** `MergeConfig` only calls merge helpers for fields that were explicitly added; new config fields added to `types.go` were never added to `merge.go`.
**How to avoid:** After adding any new field to a config struct, immediately add the corresponding merge helper call in `merge.go`.
**Warning signs:** TOML parsing succeeds but runtime behavior doesn't reflect the setting.

### Pitfall 2: Permission Case Mismatch
**What goes wrong:** Tool name in `extractFromParams` doesn't match `Tool.Name()` return value.
**Why it happens:** `extractCommandString` capitalizes the first letter (`strings.ToUpper(toolName[:1]) + toolName[1:]`), so `"metrics"` becomes `"Metrics"`, not `"MetricsTool"`.
**How to avoid:** Check the exact `Name()` return value of each tool; the case string must match the capitalized form.
**Warning signs:** Permission prompts show raw JSON instead of human-readable strings.

### Pitfall 3: Zen Provider Missing Retry
**What goes wrong:** Transient 502/503 from Zen immediately returns error instead of retrying.
**Why it happens:** Zen's `ChatCompletionStream` was implemented without the retry loop that OpenRouter and NVIDIA have.
**How to avoid:** Copy the exact retry pattern from `openrouter/client.go:136-156`.
**Warning signs:** Zen fails on transient errors while OpenRouter/NVIDIA succeed.

### Pitfall 4: WorkflowEvent Interface Missing
**What goes wrong:** Narrative engine silently ignores messages that don't implement `WorkflowEvent`.
**Why it happens:** New message types were added without the `EventType()`/`EventData()` methods.
**How to avoid:** Every message type emitted by the workflow engine should have event methods in `engine_event_methods.go`.
**Warning signs:** Narrative timeline is missing expected events.

### Pitfall 5: Rollback Never Called
**What goes wrong:** Failed bisect leaves bad commits in place; tasks marked `StatusUnrecoverable` but code is broken.
**Why it happens:** `rollback.SoftReset` is implemented but never imported by the workflow engine.
**How to avoid:** After bisect identifies offender, call `rollback.SoftReset(offenderHash, nil)` to undo.
**Warning signs:** Bad commits remain after failed execution + failed bisect.

## Code Examples

### W01: Config Merge — Adding Missing ProviderConfig Fields
```go
// Source: internal/core/config/merge.go:99-110
// ADD these three lines to mergeProviderConfig:
func (h mergeHelper) mergeProviderConfig(base, overlay *ProviderConfig, prefix string) {
    h.stringField(&base.Default, &overlay.Default, prefix+".default")
    h.boolField(&base.AutoFallback, &overlay.AutoFallback, prefix+".auto_fallback")
    // ... existing lines 102-109 ...
    // ADD:
    h.sliceField(&base.FallbackPriority, &overlay.FallbackPriority, prefix+".fallback_priority")
    h.intField(&base.HealthCheckTimeoutSecs, &overlay.HealthCheckTimeoutSecs, prefix+".health_check_timeout_secs")
    h.sliceField(&base.RegistrationOrder, &overlay.RegistrationOrder, prefix+".registration_order")
}
```

### W03: InstructionsSource Registration
```go
// Source: internal/engine/workflow/engine.go:665-669
// ADD InstructionsSource to the NewRegistry call:
contextRegistry: ctxsrc.NewRegistry(
    ctxsrc.DateTimeSource{},
    ctxsrc.EnvironmentSource{WorkDir: opts.WorkDir},
    ctxsrc.GitSource{WorkDir: opts.WorkDir},
    ctxsrc.InstructionsSource{ProjectRoot: projectRoot, WorkDir: opts.WorkDir},  // W03
),
```

### W07: StreamChunkMsg Handler
```go
// Source: internal/ui/tui/app_update.go:93-101
// ADD case after StreamDoneMsg:
case StreamChunkMsg:
    // Route workflow streaming chunks into the REPL display
    if m.replModel != nil {
        // StreamChunkMsg is a types.StreamChunkMsg alias; append chunk to REPL
        m.replModel.AppendStreamChunk(msg)
        cmds = append(cmds, m.replModel drainCmd())
    }
```

### W12-W13: Permission Extraction Fixes
```go
// Source: internal/tools/permissions.go:597-687
// W12: Change line 681 from:
case "MetricsTool":
// To:
case "Metrics":

// W13: Add after the Metrics case:
case "Git":
    if action, ok := params["action"].(string); ok && action != "" {
        return fmt.Sprintf("git %s", action)
    }
    return "git operation"
```

### W18-W19: WorkflowEvent Methods
```go
// Source: internal/engine/workflow/engine_event_methods.go
// ADD at end of file:
func (m AgentSwitchMsg) EventType() string { return "agent_switch" }
func (m AgentSwitchMsg) EventData() map[string]interface{} {
    return map[string]interface{}{
        "from_agent": m.FromAgent,
        "to_agent":   m.ToAgent,
        "plan_path":  m.PlanPath,
    }
}

func (m DecisionsSnapshotMsg) EventType() string { return "decisions_snapshot" }
func (m DecisionsSnapshotMsg) EventData() map[string]interface{} {
    return map[string]interface{}{
        "decision_count": len(m.Decisions),
    }
}
```

### W26-W27: Zen Provider Retry + Credit Detection
```go
// Source: internal/integrations/provider/zen/client.go:125
// CHANGE ChatCompletionStream to use retry loop + doChatStream pattern:
func (c *Client) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
    const maxRetries = 2
    for attempt := 0; attempt <= maxRetries; attempt++ {
        iter, err := c.doChatStream(ctx, req)
        if err == nil { return iter, nil }
        if attempt < maxRetries && provider.IsRetryable(err) {
            delay := time.Duration(1<<uint(attempt)) * time.Second
            select {
            case <-ctx.Done(): return nil, ctx.Err()
            case <-time.After(delay): continue
            }
        }
        return nil, err
    }
    return nil, fmt.Errorf("max retries exceeded")
}

// RENAME current ChatCompletionStream body to doChatStream
// CHANGE line 158 from HandleChatHTTPError to HandleChatHTTPErrorWithCredits
```

### W29: FallbackPriority Validation
```go
// Source: internal/core/config/config_validate.go:25-35
// ADD after existing provider validation:
validProviders := map[string]bool{
    types.ProviderOpenRouter: true,
    types.ProviderZen:        true,
    types.ProviderNvidia:     true,
}
for i, name := range cfg.Provider.FallbackPriority {
    if !validProviders[name] {
        errs = append(errs, ValidationError{
            Field:        fmt.Sprintf("provider.fallback_priority[%d]", i),
            ExpectedType: fmt.Sprintf("one of: %s, %s, %s", types.ProviderOpenRouter, types.ProviderZen, types.ProviderNvidia),
            ActualValue:  name,
        })
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Reflection-based config merge | Type-safe field-by-field merge | Phase 01 | Explicit, handles zero-value overrides |
| Hard-coded retry counts | `IsRetryable()` + exponential backoff | Phase 01 | Consistent transient error handling |
| Manual narrative event routing | `WorkflowEvent` interface + Bridge | Phase 01 | Decouples workflow from narrative |
| Shared permission response channel | Per-request response channels | Phase 01 | Avoids deadlock on concurrent tools |

**Deprecated/outdated:**
- `HandleChatHTTPError` without credit detection: use `HandleChatHTTPErrorWithCredits` for providers supporting 402
- Manual IP range checking: use `search.IsReservedIP()` from canonical ip_filter.go

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Zen provider should support the same retry pattern as OpenRouter/NVIDIA | W26-W27 | Low — all three providers face the same transient errors |
| A2 | `HandleChatHTTPErrorWithCredits` is the correct upgrade for Zen's error handling | W27 | Low — Zen returns 402 for insufficient credits |
| A3 | `AgentSwitchMsg` and `DecisionsSnapshotMsg` need narrative event methods | W18-W19 | Low — narrative engine silently ignores non-implementing messages |
| A4 | The ~23 dead exec constants (W23) are truly unused outside tests | W23 | Medium — verify with `rg` before deletion |
| A5 | `FallbackPriority` validation should be a hard error, not a warning | W29 | Low — typos in provider names cause silent fallback failure |

## Open Questions

1. **W08 (arbitrager field always nil):** The `arbitrager` field in `app_state.go:209` is never assigned. D-01 says wire it in, but there's no clear consumer. Recommend: remove the field (it's dead code) unless the user wants to wire in the `arbitrage.Scorer`.
   - What we know: Field exists but no assignment anywhere in codebase
   - What's unclear: Whether this was intended for future use or is genuinely dead
   - Recommendation: Remove (D-02 applies — zero references outside definition)

2. **W20-W22 (workflow placeholders):** `ModelForPhase()` returns `""`, `ProviderForPhase()` returns `nil`, `BuildAgentSwitchMessage` is never called, `IsPlanComplete` is never called. D-01 says wire them in, but there are no intended consumers.
   - What we know: All four are dead placeholders with no callers
   - What's unclear: Whether they were intended for future use
   - Recommendation: Remove (D-02 applies — zero references outside definition)

3. **W25 (StreamingMockProvider):** Never imported. D-01 says wire it in, but there's no consumer.
   - What we know: Mock exists but no test imports it
   - What's unclear: Whether it was intended for a test that was never written
   - Recommendation: Remove (D-02 applies — zero references)

4. **W47 (TokenEstimator interface):** Never wired into autodream. D-01 says wire it in.
   - What we know: Interface defined but never used as parameter
   - What's unclear: Whether autodream was intended to use it
   - Recommendation: Remove (D-02 applies — zero references as parameter)

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go 1.25+ | Build/test | ✓ | 1.25 | — |
| golangci-lint | Lint | ✓ | — | — |
| make | Build targets | ✓ | — | — |

**Missing dependencies with no fallback:** None

**Missing dependencies with fallback:** None

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go built-in testing (`go test`) |
| Config file | None (uses Makefile targets) |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| W01 | Config merge covers all fields | unit | `go test ./internal/core/config/ -run TestMergeConfig -x` | ✅ merge_test.go |
| W02 | Rollback.SoftReset called on bisect failure | integration | `go test ./internal/engine/workflow/ -run TestBisectRollback -x` | ❌ Wave 0 |
| W03 | InstructionsSource in LLM context | integration | `go test ./internal/engine/workflow/ -run TestInstructionsSource -x` | ❌ Wave 0 |
| W04 | RecordLLMInteractionWithPrompt called | unit | `go test ./internal/integrations/metrics/ -run TestRecordLLMWithPrompt -x` | ✅ metrics_test.go |
| W05 | RecordHealDuration called | unit | `go test ./internal/integrations/metrics/ -run TestRecordHealDuration -x` | ✅ metrics_test.go |
| W06 | RecordHealLoop called | unit | `go test ./internal/integrations/metrics/ -run TestRecordHealLoop -x` | ✅ metrics_test.go |
| W07 | StreamChunkMsg handled in TUI | unit | `go test ./internal/ui/tui/ -run TestStreamChunkMsg -x` | ❌ Wave 0 |
| W12 | MetricsTool case matches "Metrics" | unit | `go test ./internal/tools/ -run TestExtractFromParams_Metrics -x` | ❌ Wave 0 |
| W13 | Git case in permission extraction | unit | `go test ./internal/tools/ -run TestExtractFromParams_Git -x` | ❌ Wave 0 |
| W14 | NVIDIA_API_KEY in help | unit | `go test ./cmd/m31a/ -run TestUsage -x` | ❌ Wave 0 |
| W15 | knownConfigKeys has correct strings | unit | `go test ./internal/core/config/ -run TestKnownConfigKeys -x` | ❌ Wave 0 |
| W18-W19 | AgentSwitchMsg/DecisionsSnapshotMsg implement WorkflowEvent | unit | `go test ./internal/engine/workflow/ -run TestWorkflowEvent -x` | ❌ Wave 0 |
| W26 | Zen retry loop works | unit | `go test ./internal/integrations/provider/zen/ -run TestRetry -x` | ❌ Wave 0 |
| W27 | Zen credit detection triggers fallback | unit | `go test ./internal/integrations/provider/zen/ -run TestCreditDetection -x` | ❌ Wave 0 |
| W29 | FallbackPriority validates provider names | unit | `go test ./internal/core/config/ -run TestValidateFallbackPriority -x` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make test`
- **Phase gate:** `make check` green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `tests/testutil/integration/bisect_rollback_test.go` — covers W02 rollback integration
- [ ] `tests/testutil/integration/instructions_source_test.go` — covers W03 context injection
- [ ] `tests/testutil/integration/stream_chunk_test.go` — covers W07 TUI handling
- [ ] `internal/tools/permissions_extract_test.go` — covers W12-W13 permission extraction
- [ ] `cmd/m31a/usage_test.go` — covers W14 env var help
- [ ] `internal/core/config/known_keys_test.go` — covers W15 knownConfigKeys
- [ ] `internal/engine/workflow/narrative_events_test.go` — covers W18-W19 WorkflowEvent
- [ ] `internal/integrations/provider/zen/retry_test.go` — covers W26-W27 Zen parity
- [ ] `internal/core/config/fallback_priority_test.go` — covers W29 validation

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | config_validate.go validates all config fields |
| V4 Access Control | yes | permissions.go controls tool execution |
| V6 Cryptography | no | No crypto operations in this phase |

### Known Threat Patterns for Go/Bubble Tea Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Config injection via malformed TOML | Tampering | validateConfig rejects invalid field types |
| Permission bypass via missing tool case | Elevation of Permission | extractFromParams must cover every registered tool |
| Provider key exposure in error messages | Information Disclosure | SanitizeProviderError masks sensitive data |

## Sources

### Primary (HIGH confidence)
- `internal/core/config/merge.go` — Config merge pattern (all 351 lines read)
- `internal/core/config/types.go` — All config struct definitions (all 571 lines read)
- `internal/engine/workflow/engine.go:640-739` — Engine init, context registry registration
- `internal/engine/rollback/rollback.go` — Full rollback API (all 362 lines read)
- `internal/integrations/context/sources.go` — All 4 context source implementations (all 131 lines read)
- `internal/integrations/metrics/collector.go` — All metrics methods (all 494 lines read)
- `internal/tools/permissions.go` — Permission extraction switch/case (all 688 lines read)
- `internal/ui/tui/app_update.go` — TUI message dispatch (all 576 lines read)
- `internal/integrations/provider/openrouter/client.go` — Retry pattern (all 192 lines read)
- `internal/integrations/provider/nvidia/client.go` — Retry pattern (all 264 lines read)
- `internal/integrations/provider/zen/client.go` — Zen client (all 167 lines read)
- `internal/engine/workflow/engine_event_methods.go` — WorkflowEvent pattern (all 259 lines read)
- `internal/engine/narrative/event.go` — WorkflowEvent interface (all 11 lines read)
- `internal/core/config/config_validate.go` — Validation pattern (all 402 lines read)
- `cmd/m31a/usage.go` — Help output pattern (all 60 lines read)
- `internal/tools/exec/bash.go:24-68` — Dead constants identified
- `internal/tools/search/ip_filter.go` — Canonical IsReservedIP (all 52 lines read)
- `internal/tools/search/websearch.go:272-324` — Duplicate isReservedIP identified

### Secondary (MEDIUM confidence)
- WIRING_ISSUES.md — Full audit report with evidence for all 50 issues
- 05-CONTEXT.md — User decisions from discuss-phase

### Tertiary (LOW confidence)
None — all findings verified against source code.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all patterns read from source code
- Architecture: HIGH — engine.go, app_update.go, permissions.go fully understood
- Pitfalls: HIGH — each pitfall verified against actual code structure
- Config merge: HIGH — all missing fields identified by comparing types.go vs merge.go
- Provider retry: HIGH — OpenRouter/NVIDIA patterns documented, Zen gap confirmed

**Research date:** 2026-07-29
**Valid until:** 2026-08-28 (30 days — stable codebase, no fast-moving dependencies)
