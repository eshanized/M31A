# Phase 5: Intelligence - Research

**Researched:** 2026-08-06
**Domain:** Workflow intelligence — plan generation, context management, model routing, verification
**Confidence:** HIGH

## Summary

This phase targets four areas where the existing codebase already has substantial foundations. The plan generation pipeline (`plan.go`) already has retry loops, plan checking, coverage gates, and chunked planning. Context management (`compaction.go`, `engine_context.go`) already implements proactive compaction, progressive truncation, and provider-specific token estimation. Model routing (`engine_model.go`, `arbitrage.go`) already has per-phase model selection, complexity scoring, and cost-based arbitrage. Verification (`verify.go`, `execute_quality.go`) already has self-heal, bisect, and behavioral verification. The intelligence phase should extend these foundations with outcome learning, smarter task merging, cost-aware routing, and confidence scoring — not build from scratch.

**Primary recommendation:** Extend existing subsystems with feedback loops and scoring mechanisms. The four areas share a common pattern: collect metrics during execution, analyze them post-hoc, and feed insights back into the next planning cycle.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Plan generation (task granularity) | Workflow/Plan | — | `plan.go` owns the retry loop and plan checker |
| Outcome learning (D-04) | Workflow/Plan | Metrics/Decision | `plan.go` + `decision/` + `metrics/collector.go` — record and query outcomes |
| Task merging (D-01) | Workflow/Plan | — | `plan.go` post-processing step after plan generation |
| Re-plan on failure (D-02) | Workflow/Execute | Workflow/Plan | `execute.go` self-heal loop already triggers `healTask()` — extend to re-plan |
| Context compaction (D-05–D-07) | Compaction | Engine/Context | `compaction/compaction.go` + `engine_context.go` already implement this |
| Token counting (D-08) | Tokens | — | `tokens/estimator.go` with provider-specific estimation + EMA calibration |
| Complexity routing (D-09, D-11) | Arbitrage | Engine/Model | `arbitrage/arbitrage.go` has `Scorer` + `Recommend()` |
| Provider fallback (D-10) | Provider | Engine/Model | `provider/fallback.go` already handles auto-fallback |
| Cost caps (D-12) | Metrics | Engine | `metrics/collector.go` tracks cost; needs cap enforcement |
| Verification confidence (D-13) | Workflow/Verify | — | `verify.go` + `execute_quality.go` — extend with scoring |
| Auto-heal/bisect (D-15) | Workflow/Verify | — | `verify.go` already has `tryBisectHeal()` |
| Success threshold (D-16) | Workflow/Verify | Metrics | `verify.go` pass rate calculation + `metrics/collector.go` |

## Standard Stack

### Core (existing — extend, don't replace)

| Component | Location | Purpose | Extension Point |
|-----------|----------|---------|-----------------|
| `plan.go` | `internal/engine/workflow/` | Plan generation, retry, plan checker | Add outcome feedback loop, task merging post-processor |
| `compaction.go` | `internal/engine/compaction/` | Auto-compaction with LLM summarization | Already implements D-05–D-07; tune parameters |
| `engine_model.go` | `internal/engine/workflow/` | Per-phase model selection (4-level priority) | Add complexity-based auto-routing layer |
| `arbitrage.go` | `internal/integrations/arbitrage/` | Cost-based model recommendation | Enhance `Scorer` with file-level and dependency heuristics |
| `estimator.go` | `internal/engine/tokens/` | Provider-specific token counting + EMA | Already implements D-08; extend EMA calibration |
| `verify.go` | `internal/engine/workflow/` | Verification + self-heal + bisect | Add confidence scoring and outcome recording |
| `collector.go` | `internal/integrations/metrics/` | Metrics collection | Add plan outcome tracking, cost cap enforcement |
| `decision/` | `internal/engine/decision/` | Decision logging with ring buffer | Add plan decision category for outcome learning |
| `fallback.go` | `internal/integrations/provider/` | Auto-fallback with parallel health checks | Already implements D-10; enhance with cost awareness |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tiktoken-go` | existing | Token counting for OpenAI models | Already imported in `estimator.go` |

**No new packages required.** All intelligence improvements extend existing Go code.

## Plan Generation Strategy

### Current State

The plan pipeline in `plan.go` has four sub-steps:
1. **Pre-plan research** (optional) — runs `runResearch()` to gather context
2. **Chunked planning** (optional) — `runChunkedPlan()` generates outline then expands per-wave
3. **Standard plan generation** — retry loop (up to `MaxPlanRetries=3`) with `buildPlanContext()` + `streamLLM()` + `ParsePlan()` + `validateTasks()`
4. **Plan checker + revision loop** — `runPlanChecker()` calls `checkPlan()` and `revisePlan()` up to `maxPlanRevisions()` iterations
5. **Coverage gates** — `runCoverageGates()` runs granularity, security, gap analysis, and requirements coverage gates

**Task structure** (`types.Task`): ID, Description, Action, Category, PlanSection, Dependencies, Files, AcceptanceCriteria, Status, HealsAttempted, CommitHash.

**Existing validation** (`validateTasks`): checks for missing descriptions, actions, duplicate IDs, invalid dependencies.

### Proposed Changes

#### D-01: Merge related file edits into fewer, larger tasks

**Implementation:** Add a post-processing step after `validateTasks()` in `runPlan()` that merges tasks with overlapping file sets.

```go
// In plan.go, after validation succeeds:
tasks = mergeRelatedTasks(tasks)
```

**Merge criteria:**
- Tasks in the same wave/group with >50% file overlap
- Tasks with no inter-dependencies and same Action category
- Preserve the union of AcceptanceCriteria from merged tasks

**Risk:** Over-merging could make bisect harder (larger changesets). Mitigate: cap merged task size at a configurable threshold (e.g., max 10 files per merged task).

#### D-02: Re-plan from failure point on task failure

**Implementation:** Extend `executeTaskWithTools()` in `execute.go`. When a task fails after all heal attempts, instead of returning immediately, trigger a re-plan from that task onward.

```go
// In execute.go, after heal loop exhausted:
if task failed and tasks remain:
    e.replanFromFailure(ctx, task, remainingTasks, goal)
```

**The re-plan** calls `buildPlanContext()` with the current task state injected as context (what succeeded, what failed, why), then calls the LLM to generate replacement tasks for the remaining work.

**Existing extension point:** The `healTask()` function already builds rich context (git diff, acceptance criteria, failure reason). Re-plan extends this pattern to the task-list level.

#### D-04: Record plan execution outcomes for learning

**Implementation:** After each task completes (success or failure), record an outcome record:

```go
type PlanOutcome struct {
    TaskID      int
    Action      string
    Description string
    Files       []string
    Success     bool
    DurationMs  int64
    HealsUsed   int
    ToolCalls   int
    ErrorType   string  // from classifyError()
    Timestamp   time.Time
}
```

Store in `decision/` as a new `CategoryPlan` receipt, and persist to `metrics/` as a new field in `PhaseMetric`. The next `buildPlanContext()` call can inject a summary of recent outcomes as a "lessons learned" section.

**Extension points:**
- `decision/receipt.go` — add `CategoryPlan` (already exists as `CategoryPlan`)
- `metrics/types.go` — add `PlanOutcomes []PlanOutcome` to `SessionMetrics`
- `plan.go` `buildPlanContext()` — inject outcome summary when available

### Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Task merging creates overly large tasks | MEDIUM | Cap merged task size; log merge decisions |
| Re-plan introduces inconsistency with remaining tasks | MEDIUM | Inject current task list as context; validate re-plan output |
| Outcome learning biases future plans | LOW | Start with logging only, no auto-adjustment; manual review first |
| Plan checker stalls on revised merged tasks | LOW | Existing `isPlanCheckStalled()` guard already handles this |

## Context Window Management

### Current State

The compaction system has three layers:

1. **Auto-compaction** (`compaction.go`): `Compactor.ShouldCompact()` checks if tokens exceed `contextLength - buffer`. `Compactor.Compact()` splits messages via `SplitMessages()` (keeping `KeepTokens=8000` of recent history), serializes the head, and sends it to the LLM for summarization. The summary replaces the head as a system message with `MessageCompaction` segment type.

2. **Proactive compaction** (`engine_context.go`): `proactiveCompactCheck()` triggers before phase transitions and periodically during Execute (every `ToolCallsThreshold` tool calls). Uses `PhaseTransitionPct` config to determine when to trigger.

3. **Progressive truncation** (`engine_context.go`): `preflightContextCheck()` runs before every LLM call. Three-pass truncation: (1) truncate old tool results to 500 chars, (2) truncate old assistant messages to 1000 chars, (3) remove oldest non-system messages. Falls back to `ErrContextExceeded` only if 95% threshold is exceeded.

4. **Token estimation** (`estimator.go`): Provider-specific heuristics for 8 providers (OpenAI via tiktoken, Anthropic, Google, Meta, Mistral, Qwen, DeepSeek, Cohere). EMA calibration corrects estimates against actual API usage.

### Proposed Changes

#### D-05: Keep most recent messages plus summary of older context

**Status: Already implemented.** `SplitMessages()` in `serialize.go` splits at `KeepTokens=8000` boundary. The summary replaces old messages as a system message.

**Tuning opportunity:** The `KeepTokens` value is hardcoded at 8000. Make it configurable via `CompactionConfig.KeepTokens` (already exists in config types but only used as the struct field).

#### D-06: Compact before each phase transition (proactive)

**Status: Already implemented.** `proactiveCompactCheck()` is called before phase transitions when `Compaction.Proactive` is enabled.

**Extension point:** The `StateTransition` in `state_machine.go` could call `proactiveCompactCheck()` directly, rather than relying on each phase's `run*()` function to call it. This would make it truly automatic.

#### D-07: Keep full tool results, summarize everything else

**Status: Partially implemented.** `SerializeMessages()` in `serialize.go` truncates tool outputs to `maxToolOutputChars=2000` chars during serialization. However, the summary template already says "Do NOT include tool call details or raw tool outputs."

**Enhancement:** Modify `SplitMessages()` to preserve tool-role messages verbatim in the "recent" set, and only summarize system/user/assistant messages in the "head." This ensures actionable tool results survive compaction.

#### D-08: Provider-specific tokenizers

**Status: Already implemented.** `estimator.go` has `DetectProviderFamily()` mapping model IDs to provider families, with provider-specific character/token ratios. EMA calibration via `Calibrate()` corrects estimates using actual API response usage.

**Extension point:** The `EstimateMessages()` function counts tool call inputs but doesn't account for tool call IDs or role metadata overhead beyond the fixed 4 tokens per message. Could be refined for higher accuracy.

### Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Compaction loses critical context | MEDIUM | Summary template preserves file paths, error messages, key decisions |
| EMA calibration drifts on new models | LOW | Clamp factor to [0.1, 10.0]; reset on model switch |
| Proactive compaction during Execute adds latency | LOW | Runs in background with 60s timeout; failures are non-blocking |

## Model Routing Intelligence

### Current State

Model routing has four layers in `engine_model.go`:

1. **Per-phase override** (highest priority) — set by TUI at workflow start
2. **AgentsConfig** — per-phase model IDs from `config.toml`
3. **Global agent default** — `cfg.Agents.Default`
4. **Engine model ID** — the active model

**Arbitrage** (`arbitrage.go`): `Scorer.Score()` classifies tasks as simple/moderate/complex using keyword analysis + file/dependency boost. `Recommend()` finds the cheapest model with sufficient context window. `ShouldArbitrage()` decides if switching saves enough.

**Fallback** (`provider/fallback.go`): `FindFallbackProvider()` searches alternative providers in priority order with parallel health checks. `FindFallbackWithRetryAfter()` respects Retry-After headers.

### Proposed Changes

#### D-09: Complexity-adaptive routing

**Implementation:** Insert an arbitrage layer between phases 2 and 3 of `modelForPhase()`. When `cfg.Model.AutoArbitrage` is enabled:

```go
// In engine_model.go, after AgentsConfig check:
if e.cfg.Model.AutoArbitrage {
    rec := arbitrage.Recommend(models, task, e.cfg.Model.ArbitrageThreshold)
    if rec != nil {
        return rec.RecommendedModel.ModelID
    }
}
```

**Challenge:** `modelForPhase()` currently takes only a `WorkflowPhase`, not a `Task`. For Execute phase, the task-specific routing would need the current task's details. Two approaches:
- **Phase-level routing:** Use the goal's overall complexity (from intent classification) to pick a model for the entire phase. Simpler, less disruptive.
- **Task-level routing:** Pass the current task to `modelForPhase()` during Execute. More precise but requires changing the function signature.

**Recommendation:** Start with phase-level routing (use intent complexity). Task-level routing can be added later as a refinement.

#### D-10: Auto-fallback on failure (no user prompt)

**Status: Already implemented.** `FindFallbackProvider()` in `provider/fallback.go` already searches for healthy alternatives with parallel health checks and priority ordering. The `FindFallbackWithRetryAfter()` variant respects rate limits.

**Extension point:** The fallback currently triggers on HTTP errors. Could also trigger on quality failures (e.g., repeated tool call failures from a weak model).

#### D-11: Rule-based complexity scoring

**Status: Already implemented.** `arbitrage.Scorer.Score()` uses keyword classification (`classifyText()`) with file-count and dependency-count boosts. Keywords are grouped by complexity level:
- Complex: "design", "architect", "migrate", "rewrite", "system", "containerize", etc.
- Moderate: "implement", "create", "refactor", "database", "api", "authentication", etc.
- Simple: "fix", "add", "update", "rename", "typo", "format", etc.

**Enhancement:** Add project-type awareness (Go project + "refactor" = moderate, not simple). Add file-scope awareness (touching 10+ files = complex regardless of keywords).

#### D-12: Per-session cost cap

**Implementation:** Add cost tracking in `metrics/collector.go` and enforcement in `runExecute()`:

```go
// In execute.go, before each LLM call:
if e.cfg.Features.BudgetLimitUSD > 0 {
    currentCost := e.collector.CurrentSessionCost()
    if currentCost >= e.cfg.Features.BudgetLimitUSD {
        return TaskResult{Error: "session budget exceeded"}
    }
}
```

**Extension points:**
- `metrics/types.go` — `SessionMetrics` already tracks `LLMMetric.Cost`
- `metrics/collector.go` — add `CurrentSessionCost()` method that sums across all `LLMMetric` entries
- `config/types.go` — `FeaturesConfig.BudgetLimitUSD` already exists (line 384)

### Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Auto-routing selects weak model for complex task | MEDIUM | Context window check for complex tasks (>64K required) |
| Cost cap interrupts mid-execution | MEDIUM | Check at task boundaries, not mid-task; warn at 80% of budget |
| Arbitrage recommendation is stale (model catalog cache) | LOW | Model cache TTL is 5 minutes; refresh on health check failure |

## Verification Confidence

### Current State

Verification in `verify.go` checks each completed task:
1. **File existence** — `os.Stat()` for each task file
2. **Git diff check** — verifies task files appear in `git diff HEAD`
3. **Content validation** — checks for placeholders (TODO, FIXME), empty function bodies, stub implementations
4. **Build/test/lint** — project-type-specific commands (go build, npm test, python3 -m py_compile, cargo check)
5. **Self-heal** — on failure, calls `healTask()` which injects diagnostic context and asks LLM to fix
6. **Bisect** — `tryBisectHeal()` uses git bisect to find the offending commit, then heals with that context
7. **Quality gate** (`execute_quality.go`) — per-task acceptance criteria checks via `checkAcceptanceCriteria()`
8. **Behavioral verification** — `BehavioralVerification()` runs shell commands from acceptance criteria (with blocklist/allowlist)

### Proposed Changes

#### D-13: Confidence scoring based on verification results

**Implementation:** Add a `ConfidenceScore` to `VerificationResult`:

```go
type VerificationResult struct {
    TaskID        int
    FilesExist    bool
    SyntaxOK      bool
    TestsOK       bool
    LintOK        bool
    Confidence    float64  // 0.0 - 1.0
    Errors        []string
    Warnings      []string
}
```

**Scoring formula:**
- Base score: 1.0
- Each failed check: -0.25 (files exist, syntax, tests, lint)
- Each warning: -0.05
- Heals used > 0: -0.1 per heal attempt
- All acceptance criteria pass: +0.1 (bonus)

**Extension points:**
- `engine_verify.go` `VerificationResult` struct — add `Confidence float64`
- `engine_verify.go` `verifyTask()` — compute score after all checks
- `verify.go` `runVerify()` — aggregate per-task scores into phase-level confidence

#### D-14: Always run full verification suite

**Status: Already implemented.** `verifyTask()` always runs file existence, git diff, content validation, and project-type-specific build/test/lint. No change-skipping based on change size.

#### D-15: Auto-heal and retry on verification failure

**Status: Already implemented.** `runVerify()` calls `healTask()` on failure, then `tryBisectHeal()` as fallback. The heal loop is bounded by `MaxHealAttempts=2`.

**Extension point:** The `healTask()` function in `execute_heal.go` already builds rich diagnostic context. Could be enhanced with the bisect result from a previous attempt to avoid re-bisecting the same commit range.

#### D-16: Present results at 90% task success rate threshold

**Implementation:** In `runVerify()`, after all tasks are verified, calculate pass rate:

```go
passRate := float64(passed) / float64(total)
if passRate >= 0.90 {
    // Report success with minor failures noted
    result.Success = true  // Override: allow ship with < 10% failures
}
```

**Current behavior:** `runVerify()` sets `allOK = true` only if zero tasks failed. The 90% threshold would relax this to allow shipping with minor failures.

**Extension point:** `verify.go` line 173-180 already calculates `allOK` and collects `failedTasks`. Add the threshold check here.

### Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Confidence score is misleading (low score but works) | MEDIUM | Log confidence alongside outcome for calibration |
| 90% threshold allows shipping broken features | MEDIUM | Only apply threshold to non-critical tasks; critical tasks always require 100% |
| Auto-heal creates cascading failures | LOW | MaxHealAttempts=2 bounds the loop; bisect provides targeted fix context |

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Token counting | Custom char-count | `tokens.Estimator` | Provider-specific ratios + EMA calibration already handles 8 providers |
| Model cost comparison | Manual pricing lookup | `arbitrage.CompareModels()` | Already handles pricing data, context window checks, and sorting |
| Provider health checking | Custom HTTP probe | `provider.FindFallbackProvider()` | Parallel health checks with priority ordering already implemented |
| Plan parsing | Custom markdown parser | `plan_parser.go` | Handles title, sections, tasks, review notes, verification plan |
| Decision logging | Custom logger | `decision.Logger` | Non-blocking channel + ring buffer + flush semantics |
| Git bisect | Manual bisect loop | `bisect.Run()` | Already handles good/bad commit detection and rollback |

**Key insight:** The M31A codebase already has sophisticated infrastructure in each intelligence area. The phase should focus on connecting these subsystems with feedback loops and scoring, not rebuilding the plumbing.

## Code Examples

### Pattern 1: Recording Plan Outcomes (D-04)

```go
// In execute.go, after task completion:
outcome := PlanOutcome{
    TaskID:      task.ID,
    Action:      task.Action,
    Description: task.Description,
    Files:       task.Files,
    Success:     result.Success,
    DurationMs:  result.DurationMs,
    HealsUsed:   task.HealsAttempted,
    ToolCalls:   result.ToolCalls,
    ErrorType:   classifyError(result.Error),
    Timestamp:   time.Now(),
}
e.collector.RecordPlanOutcome(outcome)

// Log as decision receipt
e.LogDecision(decision.DecisionReceipt{
    Decision:  fmt.Sprintf("task %d completed: success=%v", task.ID, result.Success),
    Rationale: fmt.Sprintf("heals=%d, tools=%d, duration=%dms", outcome.HealsUsed, outcome.ToolCalls, outcome.DurationMs),
    Category:  decision.CategoryPlan,
    Cost: decision.Cost{
        Duration: float64(outcome.DurationMs) / 1000.0,
        Attempts: outcome.HealsUsed + 1,
    },
})
```

### Pattern 2: Confidence Scoring (D-13)

```go
// In engine_verify.go, after verifyTask():
func computeConfidence(vr VerificationResult, healsUsed int, criteriaPassed int, criteriaTotal int) float64 {
    score := 1.0
    if !vr.FilesExist { score -= 0.25 }
    if !vr.SyntaxOK   { score -= 0.25 }
    if !vr.TestsOK    { score -= 0.25 }
    if !vr.LintOK     { score -= 0.25 }
    score -= float64(len(vr.Warnings)) * 0.05
    score -= float64(healsUsed) * 0.1
    if criteriaTotal > 0 && criteriaPassed == criteriaTotal {
        score += 0.1
    }
    if score < 0 { score = 0 }
    if score > 1 { score = 1 }
    return score
}
```

### Pattern 3: Cost Cap Enforcement (D-12)

```go
// In metrics/collector.go:
func (c *Collector) CurrentSessionCost() float64 {
    if !c.enabled { return 0 }
    c.mu.Lock()
    defer c.mu.Unlock()
    total := 0.0
    for _, llm := range c.metrics.LLMs {
        total += llm.Cost
    }
    return total
}

// In execute.go, before LLM call:
if e.cfg.Features.BudgetLimitUSD > 0 && e.collector != nil {
    spent := e.collector.CurrentSessionCost()
    if spent >= e.cfg.Features.BudgetLimitUSD {
        e.logger.Warn("budget limit reached", "spent", spent, "limit", e.cfg.Features.BudgetLimitUSD)
        return taskrunner.TaskResult{Error: fmt.Sprintf("session budget exceeded: $%.2f / $%.2f", spent, e.cfg.Features.BudgetLimitUSD)}
    }
}
```

## Implementation Order

1. **Metrics extensions** — Add `PlanOutcome` tracking to `metrics/types.go` and `metrics/collector.go`. This is foundation work with zero risk to existing behavior.

2. **Decision category for plan outcomes** — Add `CategoryPlan` receipts for task completion events in `execute.go`.

3. **Confidence scoring** — Add `Confidence float64` to `VerificationResult` and compute it in `verifyTask()`. Non-breaking additive change.

4. **Task merging post-processor** — Add `mergeRelatedTasks()` as a post-processing step in `plan.go` after validation. Test with existing plans to verify no regressions.

5. **Outcome learning injection** — Modify `buildPlanContext()` to inject a summary of recent plan outcomes as context for the next planning cycle.

6. **Cost cap enforcement** — Add `CurrentSessionCost()` to `metrics/collector.go` and budget check to `execute.go`.

7. **Complexity-adaptive routing** — Add arbitrage layer to `modelForPhase()` when `AutoArbitrage` is enabled.

8. **Re-plan from failure** — Add `replanFromFailure()` method to `Engine` and wire it into the execute loop after heal exhaustion.

9. **Success threshold (90%)** — Modify `runVerify()` to allow ship with < 10% task failures when threshold is met.

10. **Compaction tuning** — Make `KeepTokens` configurable, preserve tool results in recent set.

## Dependencies

### External
- None. All changes extend existing Go code with no new dependencies.

### Internal Cross-Dependencies
- **metrics → plan**: `PlanOutcome` type used by both `metrics/types.go` and `execute.go`
- **decision → plan**: `CategoryPlan` receipts logged by `execute.go`, queried by `plan.go`
- **arbitrage → engine_model**: `Recommend()` called from `modelForPhase()` for auto-routing
- **tokens → compaction**: `Estimator` used by `Compactor.ShouldCompact()` and `preflightContextCheck()`
- **compaction → engine_context**: `proactiveCompactCheck()` called from `executeTaskWithTools()`

### AGENTS.md Constraints
- All code must be `gofmt`-clean
- Return errors, never panic; wrap with `fmt.Errorf("%w", err)`
- Exported functions need doc comments
- No CGO (CGO_ENABLED=0 hard constraint)
- Bubble Tea single-threaded — use channels, never shared mutable state from goroutines
- `pkg/` must NOT import `internal/`
- Coverage targets: 75% overall, 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard `testing` package |
| Config file | `go.mod` (Go 1.25+) |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| D-01 | Task merging | unit | `go test ./internal/engine/workflow/ -run TestMergeTasks -x` | Wave 0 |
| D-02 | Re-plan from failure | unit | `go test ./internal/engine/workflow/ -run TestReplanFromFailure -x` | Wave 0 |
| D-04 | Outcome recording | unit | `go test ./internal/integrations/metrics/ -run TestPlanOutcome -x` | Wave 0 |
| D-05–D-07 | Compaction behavior | unit | `go test ./internal/engine/compaction/ -run TestCompact -x` | ✅ existing |
| D-08 | Token estimation | unit | `go test ./internal/engine/tokens/ -run TestEstimate -x` | ✅ existing |
| D-09 | Complexity routing | unit | `go test ./internal/integrations/arbitrage/ -run TestRecommend -x` | ✅ existing |
| D-10 | Auto-fallback | unit | `go test ./internal/integrations/provider/ -run TestFallback -x` | ✅ existing |
| D-12 | Cost cap | unit | `go test ./internal/integrations/metrics/ -run TestCostCap -x` | Wave 0 |
| D-13 | Confidence scoring | unit | `go test ./internal/engine/workflow/ -run TestConfidence -x` | Wave 0 |
| D-15 | Auto-heal/bisect | unit | `go test ./internal/engine/workflow/ -run TestBisect -x` | ✅ existing |
| D-16 | 90% threshold | unit | `go test ./internal/engine/workflow/ -run TestVerifyThreshold -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make test`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/engine/workflow/plan_merge_test.go` — covers D-01 task merging
- [ ] `internal/engine/workflow/replan_test.go` — covers D-02 re-plan from failure
- [ ] `internal/integrations/metrics/plan_outcome_test.go` — covers D-04 outcome recording
- [ ] `internal/engine/workflow/confidence_test.go` — covers D-13 confidence scoring
- [ ] `internal/integrations/metrics/cost_cap_test.go` — covers D-12 cost cap

## Sources

### Primary (HIGH confidence)
- `internal/engine/workflow/plan.go` — plan generation, retry, plan checker, coverage gates
- `internal/engine/workflow/execute.go` — task execution, self-heal loop, quality gate
- `internal/engine/workflow/verify.go` — verification, self-heal, bisect fallback
- `internal/engine/workflow/engine_model.go` — per-phase model selection
- `internal/engine/workflow/engine_context.go` — proactive compaction, preflight context check
- `internal/engine/workflow/engine_verify.go` — verifyTask(), VerificationResult
- `internal/engine/workflow/execute_quality.go` — acceptance criteria checking
- `internal/engine/workflow/execute_heal.go` — self-heal with diagnostic context
- `internal/engine/compaction/compaction.go` — Compactor, ShouldCompact, Compact
- `internal/engine/compaction/serialize.go` — SerializeMessages, SplitMessages
- `internal/engine/tokens/estimator.go` — provider-specific token estimation, EMA
- `internal/integrations/arbitrage/arbitrage.go` — Scorer, Recommend, CompareModels
- `internal/integrations/provider/fallback.go` — FindFallbackProvider, parallel health checks
- `internal/integrations/metrics/collector.go` — metrics collection, RecordHeal*, RecordBisect*
- `internal/integrations/metrics/types.go` — SessionMetrics, PhaseMetric, LLMMetric
- `internal/engine/decision/receipt.go` — DecisionReceipt, Category, Cost
- `internal/core/config/types.go` — CompactionConfig, FeaturesConfig, AgentsConfig
- `internal/core/types/types.go` — Task, Message, ModelInfo, WorkflowPhase
- `AGENTS.md` — project constraints and build requirements

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — all components are existing code read directly from source
- Architecture: HIGH — extension points identified from actual function signatures and interfaces
- Pitfalls: HIGH — risks derived from concrete code patterns (e.g., heal loop bounds, context threshold logic)

**Research date:** 2026-08-06
**Valid until:** 2026-09-05 (30 days — stable codebase, no fast-moving dependencies)
