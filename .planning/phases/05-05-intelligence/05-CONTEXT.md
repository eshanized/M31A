# Phase 5: Intelligence - Context

**Gathered:** 2026-08-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Improve decision quality — not just model capability. This phase covers smarter planning (task granularity, failure handling, outcome learning), context window management (compaction strategy, token counting), model routing intelligence (complexity-adaptive routing, cost optimization), and verification confidence (automated checks with confidence scoring). Exit criteria: users trust M31A's decisions even on complex projects.

</domain>

<decisions>
## Implementation Decisions

### Plan Generation Strategy
- **D-01:** Merge related file edits, tool calls, and verification into fewer, larger tasks — **Reversibility:** reversible — can split tasks later without breaking changes
- **D-02:** On task failure, re-plan from the failure point onward with new context — **Reversibility:** reversible — can switch to retry-failed-only behavior
- **D-03:** Keep plan format as markdown task lists (no structured metadata sidecar) — **Reversibility:** reversible — can add metadata later
- **D-04:** Record plan execution outcomes (success/failure, duration) for learning — **Reversibility:** reversible — can disable recording

### Context Window Management
- **D-05:** Compaction strategy: keep most recent messages plus summary of older context — **Reversibility:** reversible — can change to relevance scoring
- **D-06:** Compact context before each phase transition (proactive) — **Reversibility:** reversible — can switch to threshold-based timing
- **D-07:** Summary format: keep full tool results (actionable), summarize everything else — **Reversibility:** reversible — can change summary scope
- **D-08:** Token counting: use provider-specific tokenizers for accurate counts — **Reversibility:** reversible — can revert to generic tiktoken

### Model Routing Intelligence
- **D-09:** Complexity-adaptive routing: simple tasks use cheap models, complex tasks use powerful models — **Reversibility:** reversible — can switch to quality-first or cost-first
- **D-10:** Auto-fallback to next provider on failure (no user prompt) — **Reversibility:** reversible — can add user prompt on failure
- **D-11:** Rule-based complexity scoring for routing decisions — **Reversibility:** reversible — can switch to history-based routing
- **D-12:** Per-session cost cap for cost optimization — **Reversibility:** reversible — can change to per-task cap or remove limits

### Verification Confidence
- **D-13:** Automated checks (build, test, lint, vet) plus confidence scoring based on results — **Reversibility:** reversible — can remove scoring
- **D-14:** Always run full verification suite regardless of change size — **Reversibility:** reversible — can scale to change size
- **D-15:** Auto-heal and retry on verification failure (bisect, fix, retry) — **Reversibility:** reversible — can switch to show-failure-ask-user
- **D-16:** Present results at 90% task success rate threshold — **Reversibility:** reversible — can adjust threshold

### the agent's Discretion
- Agent may choose specific complexity scoring rules for model routing
- Agent may design confidence scoring formula based on verification results
- Agent may select auto-heal strategies based on failure type
- Agent may decide outcome recording format for plan learning

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Codebase
- `.planning/codebase/ARCHITECTURE.md` — System overview, component responsibilities, workflow engine structure
- `.planning/codebase/CONCERNS.md` — Tech debt, known bugs, test coverage gaps, fragile areas
- `.planning/codebase/STACK.md` — Technology stack, dependencies, provider implementations

### Key Source Files
- `internal/engine/workflow/plan.go` — Plan generation from LLM, task structure
- `internal/engine/workflow/execute.go` — Task scheduling, tool dispatch, self-heal
- `internal/engine/workflow/verify.go` — Acceptance checking, self-heal loops, bisect
- `internal/engine/workflow/context_builder.go` — System prompt assembly from dynamic sources
- `internal/engine/workflow/engine_model.go` — Per-phase model selection and routing
- `internal/integrations/context/registry.go` — Dynamic context source management
- `internal/engine/compaction/compaction.go` — Automatic context window management
- `internal/integrations/codeintel/codeintel.go` — Source code indexing, relevance scoring
- `internal/integrations/metrics/collector.go` — Tool/LLM/phase metrics collection
- `internal/integrations/arbitrage/arbitrage.go` — Model selection optimization
- `internal/engine/tokens/` — Token counting via tiktoken/provider-specific

### Standards
- `AGENTS.md` — Build commands, code style, conventional commits, lint config

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `internal/engine/compaction/compaction.go` — Existing compaction logic; extend with provider-specific tokenizers
- `internal/integrations/metrics/collector.go` — Existing metrics collection; extend for plan outcome recording
- `internal/integrations/arbitrage/arbitrage.go` — Existing model selection; extend for complexity-adaptive routing
- `internal/engine/workflow/verify.go` — Existing verification; extend with confidence scoring
- `internal/engine/workflow/execute.go` — Existing task execution; extend for re-plan-on-failure

### Established Patterns
- Bubble Tea Elm architecture: all state mutations through Update(), goroutines communicate via tea.Cmd/tea.Msg
- Engine split by concern: same-package file splitting, all files share Engine receiver
- Sentinel errors + typed wrappers in `internal/core/errors/errors.go`
- Metrics collection via `internal/integrations/metrics/collector.go`

### Integration Points
- `internal/engine/workflow/engine.go:RunPhase()` — Phase execution dispatch; hook for re-plan-on-failure
- `internal/engine/workflow/engine_model.go:modelForPhase()` — Model selection; hook for complexity-adaptive routing
- `internal/engine/workflow/verify.go:VerifyPhase()` — Verification; hook for confidence scoring
- `internal/engine/compaction/compaction.go:Compact()` — Context compaction; hook for provider-specific tokenizers

</code_context>

<specifics>
## Specific Ideas

- Plan outcome recording should feed into future plan generation (not just logging)
- Complexity scoring should consider: number of files, dependency depth, test coverage, provider-specific factors
- Confidence scoring formula: (tests_passing / total_tests) * (build_success ? 1.0 : 0.8) * (lint_clean ? 1.0 : 0.9)
- Auto-heal should use existing bisect capability for failure isolation
- Provider-specific tokenizers: cl100k for OpenAI-compatible, custom for others

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 5-Intelligence*
*Context gathered: 2026-08-06*
