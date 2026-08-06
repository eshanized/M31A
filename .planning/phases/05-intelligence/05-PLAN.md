---
phase: 05-intelligence
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - internal/integrations/metrics/types.go
  - internal/integrations/metrics/collector.go
  - internal/engine/workflow/engine_messages.go
  - internal/engine/workflow/engine_verify.go
  - internal/engine/workflow/execute.go
  - internal/integrations/metrics/metrics_test.go
  - internal/engine/workflow/engine_verify_test.go
  - internal/engine/workflow/execute_test.go
autonomous: true
requirements: [INT-01, INT-02, INT-03, INT-04]
must_haves:
  truths:
    - PlanOutcome records are created after every task execution with success, duration, heals, and error type
    - VerificationResult includes a Confidence float64 field computed from check results
    - computeConfidence produces a deterministic score between 0.0 and 1.0 based on files, syntax, tests, lint, warnings, heals, and acceptance criteria
    - CurrentSessionCost sums LLM costs across all phases for the session
    - Cost check blocks LLM calls when budget is exceeded
    - Outcome recording and confidence scoring are thread-safe via collector mutex
  artifacts:
    - internal/integrations/metrics/types.go (PlanOutcome type added)
    - internal/integrations/metrics/collector.go (RecordPlanOutcome, CurrentSessionCost methods)
    - internal/engine/workflow/engine_messages.go (Confidence field in VerificationResult)
    - internal/engine/workflow/engine_verify.go (computeConfidence function, wired into verifyTask)
    - internal/engine/workflow/execute.go (outcome recording after task completion, cost check before LLM)
  key_links:
    - execute.go → collector.RecordPlanOutcome (outcome recording after task completion)
    - engine_verify.go → computeConfidence (confidence computation after verification checks)
    - execute.go → costTracker.BudgetExceeded (cost gate before LLM calls)
    - collector.RecordPlanOutcome → decision.CategoryPlan receipt (decision logging integration)
---

<objective>
Establish the metrics and scoring foundation for all intelligence improvements: plan outcome recording (D-04), verification confidence scoring (D-13), and session cost tracking (D-12).

Purpose: Every subsequent intelligence feature (task merging, re-plan, complexity routing) depends on this foundation to record outcomes and make cost-aware decisions.
Output: PlanOutcome type, confidence scoring, cost tracking, and outcome recording wired into execute/verify phases.
</objective>

<execution_context>
@/home/snigdha/.config/opencode/gsd-core/workflows/execute-plan.md
@/home/snigdha/.config/opencode/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/05-05-intelligence/05-CONTEXT.md
@.planning/phases/05-05-intelligence/05-RESEARCH.md

@internal/integrations/metrics/types.go
@internal/integrations/metrics/collector.go
@internal/engine/workflow/engine_messages.go
@internal/engine/workflow/engine_verify.go
@internal/engine/workflow/execute.go
@internal/engine/workflow/cost_tracker.go
@internal/engine/decision/receipt.go
@internal/core/types/types.go
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add PlanOutcome type, RecordPlanOutcome, and CurrentSessionCost to metrics</name>
  <files>internal/integrations/metrics/types.go, internal/integrations/metrics/collector.go, internal/integrations/metrics/metrics_test.go</files>
  <read_first>internal/integrations/metrics/types.go, internal/integrations/metrics/collector.go, internal/integrations/metrics/metrics_test.go, internal/engine/decision/receipt.go</read_first>
  <action>
Per D-04: Add a PlanOutcome struct to metrics/types.go that captures per-task execution results. The struct must include fields for TaskID (int), Action (string), Description (string), Files ([]string), Success (bool), DurationMs (int64), HealsUsed (int), ToolCalls (int), ErrorType (string, classified via error pattern matching), and Timestamp (time.Time). Add a PlanOutcomes []PlanOutcome field to SessionMetrics.

Per D-04 and D-12: In collector.go, add a RecordPlanOutcome method that appends a PlanOutcome to the PlanOutcomes slice under the collector mutex. Add a CurrentSessionCost method that sums the Cost field across all LLMMetric entries in the snapshot (thread-safe via snapshot). Both methods must be no-ops when disabled.

Add tests to metrics_test.go: TestRecordPlanOutcome (records and retrieves), TestRecordPlanOutcome_Accumulates (multiple outcomes), TestCurrentSessionCost (sums across LLM entries), TestCurrentSessionCost_Disabled (returns 0).

Follow existing patterns: the RecordPlanOutcome method should use the same lock-then-append pattern as RecordToolCall. The CurrentSessionCost method should use Snapshot() then sum, matching the pattern in Stop().
  </action>
  <verify>
    <automated>cd /home/snigdha/Desktop/Helix/M31A && go test ./internal/integrations/metrics/ -run "TestRecordPlanOutcome|TestCurrentSessionCost" -x -v</automated>
  </verify>
  <acceptance_criteria>
    - PlanOutcome struct exists with all required fields
    - PlanOutcomes field added to SessionMetrics
    - RecordPlanOutcome method exists and is thread-safe
    - CurrentSessionCost method returns sum of LLM costs
    - All 4 new tests pass
    - go vet ./internal/integrations/metrics/ passes
  </acceptance_criteria>
  <done>
PlanOutcome type defined with TaskID, Action, Description, Files, Success, DurationMs, HealsUsed, ToolCalls, ErrorType, Timestamp fields. PlanOutcomes slice added to SessionMetrics. RecordPlanOutcome and CurrentSessionCost methods implemented and tested. Tests pass with -race flag.
  </done>
</task>

<task type="auto">
  <name>Task 2: Add Confidence to VerificationResult, computeConfidence, and wire outcome recording + cost check</name>
  <files>internal/engine/workflow/engine_messages.go, internal/engine/workflow/engine_verify.go, internal/engine/workflow/execute.go, internal/engine/workflow/engine_verify_test.go, internal/engine/workflow/execute_test.go</files>
  <read_first>internal/engine/workflow/engine_messages.go, internal/engine/workflow/engine_verify.go, internal/engine/workflow/execute.go, internal/engine/workflow/cost_tracker.go, internal/integrations/metrics/collector.go, internal/core/types/types.go</read_first>
  <action>
Per D-13: In engine_messages.go, add a Confidence float64 field to the VerificationResult struct (after LintOK). This field holds a value between 0.0 and 1.0.

Per D-13: In engine_verify.go, add a computeConfidence function (exported, takes VerificationResult + healsUsed int + criteriaPassed int + criteriaTotal int, returns float64). Formula: start at 1.0, subtract 0.25 for each failed check (FilesExist, SyntaxOK, TestsOK, LintOK), subtract 0.05 per warning, subtract 0.1 per heal attempt, add 0.1 bonus when all acceptance criteria pass. Clamp to [0.0, 1.0]. Wire this into verifyTask by computing confidence after all checks and assigning it to result.Confidence.

Per D-04: In execute.go, after the execFn completes for each task (around line 134-145 where task results are processed), record a PlanOutcome via e.collector.RecordPlanOutcome. The outcome should capture task.ID, task.Action, task.Description, task.Files, result.Success, duration (time.Since(start).Milliseconds()), task.HealsAttempted, result.ToolCalls, and classify the error type (empty string on success, or a category like "llm_error", "tool_error", "heal_failed" based on result.Error patterns). Use the decision.CategoryPlan category for the decision receipt.

Per D-12: In execute.go, add a cost check before each LLM call in executeTaskWithTools (around the streamLLMWithTools call). When e.costTracker is not nil and e.costTracker.BudgetExceeded() is true, return a TaskResult with Error set to "session budget exceeded" and Success false. Do not check mid-task — only at the LLM call boundary.

Add tests: TestComputeConfidence_AllPass (score >= 0.9), TestComputeConfidence_SomeFail (score between 0.4 and 0.7), TestComputeConfidence_AllFail (score near 0), TestComputeConfidence_Clamped (never below 0 or above 1). In execute_test.go, add TestExecuteTaskWithTools_CostCapExceeded (mock provider, set budget to 0.001, verify task returns budget error).
  </action>
  <verify>
    <automated>cd /home/snigdha/Desktop/Helix/M31A && go test ./internal/engine/workflow/ -run "TestComputeConfidence|TestExecuteTaskWithTools_CostCap" -x -v</automated>
  </verify>
  <acceptance_criteria>
    - VerificationResult has Confidence float64 field
    - computeConfidence is deterministic and returns values in [0.0, 1.0]
    - verifyTask sets result.Confidence after all checks
    - PlanOutcome is recorded after every task completion in execute.go
    - Cost check blocks LLM calls when budget is exceeded
    - All new tests pass
    - go vet ./internal/engine/workflow/ passes
    - make test-fast passes (no regressions)
  </acceptance_criteria>
  <done>
Confidence field added to VerificationResult. computeConfidence function implements the specified formula and is wired into verifyTask. PlanOutcome recording is integrated into executeTaskWithTools. Cost gate blocks LLM calls when budget is exceeded. All tests pass including new unit tests for confidence scoring and cost cap.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| LLM response → confidence scoring | LLM output influences verification results, which affect confidence scores |
| Cost tracker → execution gate | Budget check blocks execution, could be bypassed if tracker is nil |
| Plan outcome → decision log | Outcomes feed into decision receipts, could leak sensitive task data |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-05-01 | Tampering | computeConfidence | low | accept | Confidence is informational only; does not gate execution |
| T-05-02 | Information Disclosure | PlanOutcome | low | accept | Outcomes stored in session-local metrics, not transmitted |
| T-05-03 | Denial of Service | BudgetExceeded check | medium | mitigate | Check only at LLM call boundary, not mid-task; nil-safe |
| T-05-04 | Elevation of Privilege | collector mutex | low | accept | Existing mutex pattern is proven; new methods follow same pattern |
</threat_model>

<verification>
1. `go test ./internal/integrations/metrics/ -race -x` — all metrics tests pass
2. `go test ./internal/engine/workflow/ -race -x -run "TestComputeConfidence|TestCostCap|TestVerifyTask"` — confidence and cost tests pass
3. `make test-fast` — no regressions across full suite
4. `go vet ./internal/integrations/metrics/ ./internal/engine/workflow/` — no vet issues
</verification>

<success_criteria>
- PlanOutcome type exists and is usable by subsequent plans
- Confidence scoring produces deterministic, bounded scores
- Cost tracking enforces budget limits at LLM call boundaries
- Outcome recording captures all task execution results
- No regressions in existing tests
- All code is gofmt-clean and follows AGENTS.md conventions
</success_criteria>

<output>
Create `.planning/phases/05-05-intelligence/05-01-SUMMARY.md` when done
</output>

---

---
phase: 05-intelligence
plan: 02
type: execute
wave: 2
depends_on: ["05-01"]
files_modified:
  - internal/engine/workflow/plan.go
  - internal/engine/workflow/execute.go
  - internal/engine/workflow/engine_model.go
  - internal/engine/workflow/plan_merge.go
  - internal/engine/workflow/plan_merge_test.go
  - internal/engine/workflow/replan.go
  - internal/engine/workflow/replan_test.go
  - internal/engine/workflow/engine_model_test.go
autonomous: true
requirements: [INT-01, INT-02, INT-05, INT-06]
must_haves:
  truths:
    - Tasks with overlapping file sets are merged into fewer, larger tasks
    - Task merging preserves the union of acceptance criteria from merged tasks
    - Merged task size is capped at a configurable threshold (default 10 files)
    - On task failure after heal exhaustion, re-planning generates replacement tasks
    - Re-plan injects current task state (what succeeded, what failed, why) as context
    - Complexity routing selects cheaper models for simple tasks and powerful models for complex tasks
    - AutoArbitrage config flag enables/disables complexity-based routing
    - Outcome summaries from past sessions are injected into plan context as "lessons learned"
  artifacts:
    - internal/engine/workflow/plan_merge.go (mergeRelatedTasks function)
    - internal/engine/workflow/replan.go (replanFromFailure method)
    - internal/engine/workflow/plan.go (merge step after validation, outcome injection in buildPlanContext)
    - internal/engine/workflow/execute.go (re-plan trigger after heal exhaustion)
    - internal/engine/workflow/engine_model.go (arbitrage layer between AgentsConfig and default)
  key_links:
    - plan.go validateTasks → mergeRelatedTasks (post-processing step)
    - execute.go heal exhaustion → replanFromFailure (recovery path)
    - engine_model.go modelForPhase → arbitrage.Recommend (complexity routing)
    - plan.go buildPlanContext → outcome summary injection (learning loop)
---

<objective>
Implement task merging (D-01), re-plan from failure (D-02), complexity-adaptive routing (D-09, D-11), and outcome learning injection (D-04).

Purpose: These four features improve plan quality by reducing task count, recovering from failures intelligently, routing to appropriate models, and learning from past execution outcomes.
Output: mergeRelatedTasks post-processor, replanFromFailure method, arbitrage integration in modelForPhase, and outcome context injection in buildPlanContext.
</objective>

<execution_context>
@/home/snigdha/.config/opencode/gsd-core/workflows/execute-plan.md
@/home/snigdha/.config/opencode/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/05-05-intelligence/05-CONTEXT.md
@.planning/phases/05-05-intelligence/05-RESEARCH.md
@.planning/phases/05-05-intelligence/05-01-SUMMARY.md

@internal/engine/workflow/plan.go
@internal/engine/workflow/execute.go
@internal/engine/workflow/engine_model.go
@internal/integrations/arbitrage/arbitrage.go
@internal/core/types/types.go
@internal/core/config/types.go
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add task merging post-processor and outcome learning injection</name>
  <files>internal/engine/workflow/plan_merge.go, internal/engine/workflow/plan_merge_test.go, internal/engine/workflow/plan.go</files>
  <read_first>internal/engine/workflow/plan.go, internal/core/types/types.go, internal/integrations/metrics/types.go</read_first>
  <action>
Per D-01: Create plan_merge.go with a mergeRelatedTasks function that takes []Task and returns []Task. Merge criteria: tasks in the same group with >50% file overlap, tasks with no inter-dependencies and same Action category. When merging: union the Files slices (deduplicated), union the AcceptanceCriteria (deduplicated), keep the first task's ID and Description, set Action to the most common action among merged tasks, set Dependencies to the union of all dependencies (excluding IDs of merged tasks). Cap merged task size at 10 files — if merging would exceed this, skip the merge for that pair. Log merge decisions at debug level. The function must be deterministic (same input produces same output regardless of call order).

Per D-04: In plan.go, after the validateTasks call in runPlan (around line 150-160 where tasks are validated), add a call to mergeRelatedTasks. Insert it after validation succeeds and before saving tasks. Also in buildPlanContext (around line 460-500), after the research output injection, add outcome learning: query the metrics collector for recent PlanOutcomes (last 10), format them as a "Lessons Learned" section with task descriptions, success/failure, error types, and inject into the plan context string.

Add tests to plan_merge_test.go: TestMergeRelatedTasks_NoOverlap (tasks with disjoint files unchanged), TestMergeRelatedTasks_Overlap (tasks with >50% overlap merged), TestMergeRelatedTasks_CapExceeded (merge skipped when result >10 files), TestMergeRelatedTasks_PreservesCriteria (acceptance criteria union), TestMergeRelatedTasks_Deterministic (same output on repeated calls).
  </action>
  <verify>
    <automated>cd /home/snigdha/Desktop/Helix/M31A && go test ./internal/engine/workflow/ -run "TestMergeRelatedTasks" -x -v</automated>
  </verify>
  <acceptance_criteria>
    - mergeRelatedTasks function exists and is deterministic
    - Tasks with >50% file overlap are merged
    - Merged task preserves union of acceptance criteria
    - Merged task size capped at 10 files
    - Outcome summary injected into buildPlanContext when collector has data
    - All 5 new tests pass
    - go vet ./internal/engine/workflow/ passes
  </acceptance_criteria>
  <done>
mergeRelatedTasks post-processor merges tasks with overlapping file sets, preserves acceptance criteria, caps at 10 files. Outcome learning injection adds "Lessons Learned" section to plan context from recent PlanOutcomes. Tests verify merge behavior, caps, and determinism.
  </done>
</task>

<task type="auto">
  <name>Task 2: Add re-plan from failure and complexity-adaptive routing</name>
  <files>internal/engine/workflow/replan.go, internal/engine/workflow/replan_test.go, internal/engine/workflow/execute.go, internal/engine/workflow/engine_model.go, internal/engine/workflow/engine_model_test.go</files>
  <read_first>internal/engine/workflow/execute.go, internal/engine/workflow/engine_model.go, internal/integrations/arbitrage/arbitrage.go, internal/core/config/types.go</read_first>
  <action>
Per D-02: Create replan.go with a replanFromFailure method on Engine. Signature: func (e *Engine) replanFromFailure(ctx context.Context, failedTask m31types.Task, remainingTasks []m31types.Task, failureReason string, goal string) ([]m31types.Task, error). The method builds a plan context that includes: the original goal, the failed task's details and failure reason, the remaining tasks list, and a summary of what succeeded. It calls streamLLM to generate replacement tasks, parses them via ParsePlan, validates via validateTasks, and returns the new tasks. On any error, return the error (do not panic).

In execute.go, in the executeTaskWithTools function, after the heal loop is exhausted and the task is marked StatusFailed or StatusUnrecoverable (around lines 300-320 in the existing code), check if tasks remain after this one. If so, call replanFromFailure. If re-planning succeeds, replace the remaining tasks in the tasks slice and save them. If re-planning fails, log a warning and continue with the original remaining tasks.

Per D-09 and D-11: In engine_model.go, modify modelForPhase to add an arbitrage layer. After the AgentsConfig check (priority 2) and before the global default (priority 3), when e.cfg.Model.AutoArbitrage is enabled and phase is PhaseExecute, call arbitrage.Recommend with the available models from the registry. If a recommendation is returned, use its RecommendedModel.ModelID. Log the routing decision at debug level with complexity, savings, and reason. The arbitrage layer should be gated behind the AutoArbitrage config flag (D-11).

Add tests: TestReplanFromFailure_GeneratesTasks (mock provider returns valid plan), TestReplanFromFailure_LLMFails (returns error, original tasks preserved), TestModelForPhase_AutoArbitrage (when enabled, uses arbitrage recommendation), TestModelForPhase_AutoArbitrage_Disabled (when disabled, falls through to default).
  </action>
  <verify>
    <automated>cd /home/snigdha/Desktop/Helix/M31A && go test ./internal/engine/workflow/ -run "TestReplanFromFailure|TestModelForPhase_AutoArbitrage" -x -v</automated>
  </verify>
  <acceptance_criteria>
    - replanFromFailure method exists and returns replacement tasks
    - execute.go triggers re-planning after heal exhaustion when tasks remain
    - modelForPhase uses arbitrage.Recommend when AutoArbitrage is enabled
    - modelForPhase falls through to default when AutoArbitrage is disabled
    - All 4 new tests pass
    - go vet ./internal/engine/workflow/ passes
    - make test-fast passes (no regressions)
  </acceptance_criteria>
  <done>
replanFromFailure generates replacement tasks from failure context. execute.go triggers re-planning after heal exhaustion. modelForPhase integrates arbitrage for complexity-adaptive routing when AutoArbitrage is enabled. Tests verify re-plan generation, error handling, and routing behavior.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| LLM re-plan output → task replacement | Re-planned tasks could be invalid or malicious |
| Arbitrage recommendation → model selection | Complex routing could select inappropriate models |
| Outcome history → plan context | Past outcomes could bias future plans |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-05-05 | Tampering | replanFromFailure | medium | mitigate | Re-plan output validated via validateTasks before use |
| T-05-06 | Spoofing | arbitrage.Recommend | low | accept | Complex tasks require 64K+ context window check |
| T-05-07 | Denial of Service | mergeRelatedTasks | low | accept | Cap at 10 files prevents overly large tasks |
| T-05-08 | Information Disclosure | outcome injection | low | accept | Outcomes are session-local, not cross-session |
</threat_model>

<verification>
1. `go test ./internal/engine/workflow/ -race -x -run "TestMergeRelatedTasks|TestReplanFromFailure|TestModelForPhase"` — all new tests pass
2. `make test-fast` — no regressions
3. `go vet ./internal/engine/workflow/` — no vet issues
</verification>

<success_criteria>
- Task merging reduces task count when files overlap
- Re-planning generates valid replacement tasks after failure
- Complexity routing selects appropriate models based on task complexity
- Outcome learning improves future plan quality
- No regressions in existing tests
</success_criteria>

<output>
Create `.planning/phases/05-05-intelligence/05-02-SUMMARY.md` when done
</output>

---

---
phase: 05-intelligence
plan: 03
type: execute
wave: 2
depends_on: ["05-01"]
files_modified:
  - internal/engine/compaction/compaction.go
  - internal/engine/workflow/verify.go
  - internal/engine/compaction/compaction_test.go
  - internal/engine/workflow/verify_test.go
autonomous: true
requirements: [INT-07, INT-08]
must_haves:
  truths:
    - KeepTokens is configurable via CompactionConfig instead of hardcoded 8000
    - Tool-role messages are preserved verbatim in the recent set during compaction
    - Verification allows shipping when 90% or more of tasks pass
    - Verification reports which tasks failed when below 90% threshold
    - Configurable KeepTokens defaults to 8000 when not set
    - Tool result preservation does not break existing compaction behavior
  artifacts:
    - internal/engine/compaction/compaction.go (configurable KeepTokens, tool result preservation)
    - internal/engine/workflow/verify.go (90% success threshold)
    - internal/engine/compaction/compaction_test.go (KeepTokens config test)
    - internal/engine/workflow/verify_test.go (90% threshold test)
  key_links:
    - compaction.go Config.KeepTokens → SplitMessages (configurable token preservation)
    - verify.go runVerify → 90% threshold check (allow partial success)
    - compaction.go SerializeMessages → tool-role message preservation (actionable data survival)
---

<objective>
Make compaction configurable and improve verification resilience by allowing partial success.

Purpose: These changes improve context window management (D-05, D-06, D-07) and verification confidence (D-16) by making compaction behavior tunable and allowing shipments when most tasks succeed.
Output: Configurable KeepTokens, tool result preservation in compaction, and 90% success threshold in verification.
</objective>

<execution_context>
@/home/snigdha/.config/opencode/gsd-core/workflows/execute-plan.md
@/home/snigdha/.config/opencode/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/05-05-intelligence/05-CONTEXT.md
@.planning/phases/05-05-intelligence/05-RESEARCH.md
@.planning/phases/05-05-intelligence/05-01-SUMMARY.md

@internal/engine/compaction/compaction.go
@internal/engine/compaction/serialize.go
@internal/engine/workflow/verify.go
@internal/core/config/types.go
</context>

<tasks>

<task type="auto">
  <name>Task 1: Make compaction KeepTokens configurable and preserve tool results</name>
  <files>internal/engine/compaction/compaction.go, internal/engine/compaction/compaction_test.go</files>
  <read_first>internal/engine/compaction/compaction.go, internal/engine/compaction/serialize.go, internal/core/config/types.go</read_first>
  <action>
Per D-05 and D-07: In compaction.go, modify the Config struct to document that KeepTokens defaults to 8000 when not explicitly set. Add a validation step in New() that clamps KeepTokens to a reasonable range (min 2000, max 32000) — if the provided value is outside this range, log a warning and use the default. The DefaultConfig() already returns KeepTokens=8000, so this is about documentation and safety clamping.

Per D-07: In serialize.go (or wherever SplitMessages is defined), modify the message splitting logic to preserve tool-role messages verbatim in the "recent" set. Currently, all messages beyond the KeepTokens boundary are summarized. The change: when building the "recent" messages to preserve, include ALL tool-role messages regardless of their position, and only summarize system/user/assistant messages in the "head." This ensures actionable tool results survive compaction. The summary template should note that tool results are preserved in full.

Add tests: TestCompaction_KeepTokensDefault (DefaultConfig returns 8000), TestCompaction_KeepTokensClamped (values outside range are clamped), TestCompaction_ToolResultsPreserved (tool messages survive compaction), TestCompaction_SummaryExcludesToolResults (summary template does not mention tool outputs).
  </action>
  <verify>
    <automated>cd /home/snigdha/Desktop/Helix/M31A && go test ./internal/engine/compaction/ -run "TestCompaction_KeepTokens|TestCompaction_ToolResults" -x -v</automated>
  </verify>
  <acceptance_criteria>
    - KeepTokens is documented as configurable with 8000 default
    - Values outside 2000-32000 are clamped with warning
    - Tool-role messages are preserved verbatim in recent set
    - Summary template does not include tool outputs
    - All 4 new tests pass
    - go vet ./internal/engine/compaction/ passes
  </acceptance_criteria>
  <done>
KeepTokens is configurable with documented default and safety clamping. Tool-role messages are preserved verbatim during compaction. Summary template excludes tool outputs. Tests verify configuration, clamping, and tool result preservation.
  </done>
</task>

<task type="auto">
  <name>Task 2: Add 90% success threshold to verification</name>
  <files>internal/engine/workflow/verify.go, internal/engine/workflow/verify_test.go</files>
  <read_first>internal/engine/workflow/verify.go, internal/engine/workflow/engine_messages.go</read_first>
  <action>
Per D-16: In verify.go, modify the runVerify function's final check (around lines 172-180 where allOK is computed). After counting failed tasks, calculate the pass rate: passRate = float64(passed) / float64(total). If passRate >= 0.90 and total > 0, set allOK = true (override the zero-failure requirement). Log the pass rate at info level. The failedTasks list should still be populated for the result.Error field so users can see which tasks failed.

Per D-16: Add a VerifySuccessThreshold constant (0.90) at the top of verify.go. When the threshold is met, the phase result should indicate partial success with a warning message listing the failed tasks, rather than returning an error. When the threshold is not met, maintain the current behavior (return error with failed task details).

Per D-13: In the same section, compute an aggregate confidence score across all verified tasks by averaging the Confidence fields from the VerificationResult map. Include this average in the verification report if VerifyReport is enabled.

Add tests: TestRunVerify_AllPass (100% pass rate succeeds), TestRunVerify_90PercentPass (90% pass rate succeeds with warning), TestRunVerify_Below90Percent (80% pass rate fails), TestComputeConfidence_Aggregate (average of multiple task confidences).
  </action>
  <verify>
    <automated>cd /home/snigdha/Desktop/Helix/M31A && go test ./internal/engine/workflow/ -run "TestRunVerify_.*Pass|TestRunVerify_Below|TestComputeConfidence_Aggregate" -x -v</automated>
  </verify>
  <acceptance_criteria>
    - VerifySuccessThreshold constant is 0.90
    - 90%+ pass rate results in phase success with warning
    - Below 90% pass rate results in phase failure
    - Aggregate confidence is computed and logged
    - Failed tasks are listed in result.Error even when threshold is met
    - All 4 new tests pass
    - go vet ./internal/engine/workflow/ passes
    - make test-fast passes (no regressions)
  </acceptance_criteria>
  <done>
Verification allows shipping at 90% success rate with warning. Below 90% maintains current failure behavior. Aggregate confidence is computed across all tasks. Tests verify threshold behavior at boundary conditions.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Compaction summary → LLM context | Summarized context could lose critical information |
| 90% threshold → shipment decision | Partial success could ship broken features |
| Tool result preservation → context size | Preserving all tool results could exceed context window |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-05-09 | Tampering | compaction summary | low | accept | Summary is informational; tool results preserved in full |
| T-05-10 | Denial of Service | tool result preservation | medium | mitigate | Cap tool result preservation at KeepTokens budget |
| T-05-11 | Elevation of Privilege | 90% threshold | medium | mitigate | Threshold only applies to non-critical tasks; critical tasks always require 100% |
| T-05-12 | Information Disclosure | aggregate confidence | low | accept | Confidence scores are session-local |
</threat_model>

<verification>
1. `go test ./internal/engine/compaction/ -race -x` — compaction tests pass
2. `go test ./internal/engine/workflow/ -race -x -run "TestRunVerify|TestComputeConfidence_Aggregate"` — threshold tests pass
3. `make test-fast` — no regressions
4. `go vet ./internal/engine/compaction/ ./internal/engine/workflow/` — no vet issues
</verification>

<success_criteria>
- KeepTokens is configurable with documented defaults and safety bounds
- Tool results survive compaction for actionable context
- 90% threshold allows partial success shipments
- Aggregate confidence is computed and reported
- No regressions in existing tests
</success_criteria>

<output>
Create `.planning/phases/05-05-intelligence/05-03-SUMMARY.md` when done
</output>
