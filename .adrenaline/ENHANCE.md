# M31A Autonomous Agent — Enhancement Analysis

> **Constraint**: No new features. All improvements are quality, reliability, or reasoning upgrades to existing mechanisms.

---

## 1. Prompt Engineering

### 1.1 `base.md` — Identity Sharpening

**File**: [`prompts/base.md`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/prompts/base.md)

**Problem**: The agent's identity is generic. It says "think before acting" but gives no mental model for *how* to reason through a problem before using tools.

**Change**: Add a structured inner-monologue protocol — before any tool use, the agent should articulate *(a)* the goal of this step, *(b)* why this tool is the right choice, *(c)* what success looks like. This costs zero tokens at runtime (it's a reasoning instruction, not output format) but dramatically sharpens decision quality.

```diff
- Think before acting. Plan your approach before making changes.
+ Think before acting. Before calling any tool, state: (1) what this step
+ accomplishes, (2) why this tool is correct for it, (3) what a successful
+ result looks like. This reasoning is for you — keep it concise.
```

---

### 1.2 `execute-task.md` — Dependency Output Verification Gap

**File**: [`prompts/execute-task.md`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/prompts/execute-task.md)

**Problem**: Step 1 says "read dependency outputs first" but gives no guidance on *what to verify* in those outputs. The agent reads them passively, missing the opportunity to detect upstream breakage before wasting an LLM call on downstream work.

**Change**: Add explicit verification gate:

```diff
- 1. **Read dependency outputs first**. If this task depends on prior tasks, read the files
-    those tasks created to understand the context.
+ 1. **Read and validate dependency outputs first**. If this task depends on prior tasks,
+    read the files those tasks created. Verify they are syntactically correct and contain
+    the constructs your task will use (e.g., the exported function, the config key, the
+    schema field). If a dependency file is malformed or missing a required piece, stop and
+    report the issue rather than proceeding on a broken foundation.
```

---

### 1.3 `self-heal.md` — Missing Root Cause Classification

**File**: [`prompts/self-heal.md`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/prompts/self-heal.md)

**Problem**: The diagnostic steps are good but flat. When the LLM reaches step 3 ("identify the root cause"), it has no structured taxonomy to apply, so it guesses. A taxonomy primes it to think systematically.

**Change**: Add an explicit cause taxonomy after step 3:

```markdown
**Root cause taxonomy** (check in order):
- **Wrong type**: a type, field, or method was referenced that doesn't exist
- **Missing import**: a package was used but not imported  
- **Interface mismatch**: a method was defined with the wrong signature
- **Stale reference**: a renamed/deleted symbol is still referenced
- **State dependency**: the code assumes prior state that wasn't established
- **Environment**: a command/binary is unavailable in the current environment
```

---

### 1.4 `plan-format.md` — Task Granularity Guidance

**File**: [`prompts/plan-format.md`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/prompts/plan-format.md)

**Problem**: No guidance on task granularity. LLMs either produce monolithic "implement everything" tasks (one task does too much, fails, and loses all work) or atomic-to-the-point-of-absurdity tasks (100 tasks for a simple CRUD app).

**Change**: Add a granularity rule to the Task List section:

```diff
+ Task granularity rule: Each task should be completable in one LLM call
+ (< 200 lines of code). If a task's file list has more than 3 files, split it.
+ If a task description contains "and", consider splitting. Group related
+ one-line changes into a single task.
```

---

### 1.5 `discuss-questions.md` — Question Quality Signal

**File**: [`prompts/discuss-questions.md`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/prompts/discuss-questions.md)

**Problem**: The Discuss phase produces generic questions like "What framework?" even when the answer is obvious from the project structure (e.g., `go.mod` clearly identifies a Go project, `package.json` already specifies a framework). The agent ignores observable facts.

**Change**: Add an observation-first rule:

```diff
+ Before asking questions, check what the project already tells you:
+ inspect the file listing for lock files, config files, and framework indicators.
+ Do not ask about technology choices that are already committed in the project.
+ Only ask about decisions that genuinely cannot be inferred from the codebase.
```

This change is already partially supported by `buildPlanContext` which calls `detectProjectType` and passes it in — the discuss phase just needs to honor it.

---

### 1.6 `autonomous.md` — Missing Error Budget Discipline

**File**: [`prompts/autonomous.md`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/prompts/autonomous.md)

**Problem**: The autonomous REPL agent has no guidance on when to stop and ask vs. when to keep trying. It may silently retry the same broken approach 5 times.

**Change**: Add an escalation heuristic:

```diff
+ ## Error Budget
+ If the same command fails twice in a row with the same error, stop retrying
+ and either: (a) try a fundamentally different approach, or (b) ask one
+ clarifying question. Repeated identical failures are a signal of a wrong
+ assumption, not a transient error.
```

---

## 2. Execution Quality

### 2.1 `execute.go` — Heal Loop Uses Stale System Prompt

**File**: [`execute.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/execute.go#L468-L483)

**Problem**: `healTask()` builds its system prompt with only `e.prompts.SelfHeal`:
```go
healPrompt := e.buildSystemPrompt(e.prompts.SelfHeal)
```
But `SelfHeal` doesn't include `ToolUse`, `ContextAwareness`, or `CodeQuality` — so the heal LLM call is missing the read-before-write rule, the tool use guide, and the correctness checklist. This is the most impactful bug in the execution layer: the agent that's supposed to fix broken code is running with a stripped-down prompt.

**Fix**:
```diff
- healPrompt := e.buildSystemPrompt(e.prompts.SelfHeal)
+ healPrompt := e.buildSystemPrompt(e.prompts.ToolUse, e.prompts.SelfHeal, e.prompts.CodeQuality)
```

---

### 2.2 `execute.go` — Tool Error Aggregation Masks Individual Failures

**File**: [`execute.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/execute.go#L275-L293)

**Problem**: When multiple tools fail in parallel, `toolErrMessages` are concatenated into a single string joined with `"; "`. The heal context then gets this opaque blob. A better format would structure each failure so the LLM can reason about them independently.

**Fix**: Format tool errors with tool name, input summary, and output:
```go
// Instead of:
toolErrMessages = append(toolErrMessages, fmt.Sprintf("tool %s failed: %v", tr.call.Name, tr.err))

// Use:
toolErrMessages = append(toolErrMessages, fmt.Sprintf("Tool: %s\nInput: %s\nError: %v",
    tr.call.Name, summarizeInput(tr.call.Input, 200), tr.err))
```
This is purely a string formatting change — no new data, no new tools.

---

### 2.3 `execute.go` — `buildExecuteContext` File Context Cap is Too Low

**File**: [`execute.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/execute.go#L453-L462)

**Problem**:
```go
const maxFileCtx = 12000
if len(fileCtx) > maxFileCtx {
    fileCtx = fileCtx[:maxFileCtx] + "\n... (truncated)"
```
12,000 bytes is roughly 3,000 tokens. For a task touching even a moderately-sized Go file (e.g., `engine.go` at 28KB), the agent gets a severely truncated view and will write code that conflicts with the parts it can't see.

**Fix**: Raise the cap to 32,000 bytes (≈8K tokens) and add per-file truncation logic that reads the *first* 4KB of each file rather than head-truncating the concatenated blob. This gives the agent a useful view of every file rather than a complete view of none:
```go
const maxFileBytesPerFile = 4096
const maxFileCtxTotal = 32000
```

---

### 2.4 `engine_verify.go` — Verify Only Checks `StatusDone` Tasks

**File**: [`verify.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/verify.go#L41-L44)

**Problem**:
```go
for i, task := range tasks {
    if task.Status != m31types.StatusDone {
        continue
    }
```
Tasks that were skipped because their dependencies failed are silently ignored. This means the verify phase passes even when significant work wasn't done. At minimum, skipped tasks should be flagged in the `PhaseResult` as expected-but-missing deliverables.

**Fix**: Collect skipped tasks separately and include them in the result's `Error` field if the plan had files associated with them:
```go
var skippedWithFiles []string
for _, task := range tasks {
    if task.Status == m31types.StatusSkipped && len(task.Files) > 0 {
        skippedWithFiles = append(skippedWithFiles, fmt.Sprintf("task %d (%s)", task.ID, task.Description))
    }
}
```

---

## 3. Context Management

### 3.1 `engine.go` — `cachedBasePromptOnce` Prevents Runtime Prompt Updates

**File**: [`engine.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/engine.go#L638-L649)

**Problem**:
```go
func (e *Engine) buildSystemPrompt(extra ...string) string {
    e.cachedBasePromptOnce.Do(func() {
        e.cachedBasePrompt = e.prompts.Base
    })
```
The base prompt is cached on first call and **never updated**. If the prompts registry is ever refreshed (e.g., for multi-session agent reuse), the stale prompt is served indefinitely. The `sync.Once` is correct for a single session, but the comment should document this lifetime constraint to prevent silent bugs in future code.

**Fix**: Document the invariant explicitly:
```go
// cachedBasePromptOnce caches e.prompts.Base for the lifetime of this Engine
// instance. Since prompts are loaded once at engine creation and never reloaded,
// this is correct. Do not reuse an Engine across different prompt configurations.
```

---

### 3.2 `autodream/autodream.go` — Summary is Truncated to ~500 Tokens Without LLM Summarization

**File**: [`autodream.go`](file:///home/snigdha/Desktop/Helix/M31A/pkg/autodream/autodream.go#L177-L185)

**Problem**: The AutoDream consolidation does not call the LLM to produce a summary. It takes the raw concatenated content, truncates it to ~384 words, and injects it as a "memory" segment. This means the "summary" is just the first portion of old messages — not a semantic distillation. As a result, the critical information (decisions made, code written, errors resolved) that tends to appear later in a conversation is truncated away.

**Fix without a new feature**: Change the selection strategy from "oldest 50% truncated to head" to "oldest 50% sampled by role distribution". Specifically: keep the first and last message from each role (user, assistant, tool) in the candidate window. This is a pure in-process transformation — no LLM call needed:

```go
// Instead of allWords[:maxWords], select representative samples:
// - First user message (sets the goal)  
// - Last user message (most recent instruction)
// - Last assistant message (most recent reasoning)
// This produces a denser, more informative summary from the same token budget.
```

---

### 3.3 `plan.go` — Previous Plan Truncated to 4000 Bytes Before Refinement

**File**: [`plan.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/plan.go#L200-L204)

**Problem**:
```go
prevPlan := e.planMarkdown
if len(prevPlan) > 4000 {
    prevPlan = prevPlan[:4000] + "\n... (truncated)"
}
```
When the user requests a refinement, the agent receives a truncated version of the plan it's supposed to be revising. For any plan with more than ~1K tokens, the agent is revising a fragment. The task list — which is the most critical part for the agent to preserve/modify correctly — is typically at the *end* of the plan document, so it's the first thing to get truncated.

**Fix**: Reverse the truncation priority — keep the *end* of the plan (task list) and truncate the *beginning* (narrative summary):
```go
if len(prevPlan) > 4000 {
    // Keep the end (task list) rather than the beginning (narrative)
    prevPlan = "... (summary truncated)\n" + prevPlan[len(prevPlan)-4000:]
}
```

---

## 4. Planning Intelligence

### 4.1 `classify.go` — Complexity Classifier is Purely Keyword-Based

**File**: [`classify.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/classify.go)

**Problem**: `ClassifyPrompt` uses keyword lists to determine whether to run Full/Fast/Direct mode. This produces false positives ("add auth bypass" → Trivial, despite the code complexity signals catching `auth`... but "add rate limiting middleware" → Simple, even though it requires understanding the entire HTTP stack).

The classifier also ignores **code structure signals**. A project with 200 files in multiple packages is inherently more risky to modify than a 5-file project, regardless of the goal phrasing.

**Improvements**:
1. Increase the project-size threshold that triggers moderate classification from 50 to 30 files — most codebases with 30+ files have enough interdependencies that "simple" goals need a plan:
   ```diff
   - if fileCount > 50 {
   + if fileCount > 30 {
   ```

2. Add a "cross-package modification" signal: if the goal mentions package or module names that exist in the project, classify as at least Moderate:
   ```go
   // If the goal references a known package/module name, it's cross-cutting
   if referencesKnownPackage(lower, workDir) {
       return m31types.ComplexityModerate
   }
   ```

---

### 4.2 `plan_parser.go` — Missing Acceptance Criteria Validation

**File**: [`plan_parser.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/plan_parser.go)

**Problem**: `validateTasks` (in `engine_parse.go`) checks for IDs, descriptions, and dependency integrity, but does not validate that tasks have `acceptance_criteria`. Tasks with empty acceptance criteria cannot be meaningfully verified — the verify phase has nothing to check against.

**Fix**: Add to `validateTasks`:
```go
if len(t.AcceptanceCriteria) == 0 {
    errs = append(errs, fmt.Sprintf("task %d: missing acceptance_criteria", t.ID))
}
```
This causes the plan loop to retry generation if any task lacks criteria — no new feature, just stricter validation of existing schema.

---

## 5. Self-Healing

### 5.1 `execute.go` — Heal Result Not Re-verified Before Continuing

**File**: [`execute.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/execute.go#L160-L165)

**Problem**: After a successful heal, the loop calls `continue` immediately:
```go
healResult := e.healTask(ctx, *task, failureReason, goal)
// ...
if !healResult.Success {
    return healResult
}
continue  // ← re-enters loop without verifying the fix actually worked
```
The heal might have "succeeded" in the sense that the LLM called some tools and they ran without error — but that doesn't mean the *task* succeeded. The next iteration will call `streamLLMWithTools` again with the same task context, potentially re-encountering the same problem or making a different mistake.

**Fix**: Add a lightweight file-existence check after heal before continuing:
```go
if healResult.Success {
    // Verify at least the expected files were created/exist
    allExist := true
    for _, f := range task.Files {
        if _, err := os.Stat(filepath.Join(e.workDir, f)); os.IsNotExist(err) {
            allExist = false
            break
        }
    }
    if !allExist {
        // Files still missing after "successful" heal — increment and try again
        task.HealsAttempted++
        continue
    }
}
```

---

### 5.2 `engine.go` / `HealTask` — Manual Heal Doesn't Preserve Context

**File**: [`engine.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/engine.go#L436-L485)

**Problem**: When the user manually triggers heal via `HealTask`, the failure message is:
```go
"Manual heal triggered by user.\nTask description: %s\nFiles: %v\nAcceptance criteria: %v\n"+
    "Inspect the files listed above, identify any issues, and apply a fix."
```
This context is much weaker than the `healTask()` invocation from the execute loop, which includes the original failure reason. Manual heal has no failure reason — it just says "something is wrong, go find it."

**Fix**: Before calling `e.healTask`, attempt to re-run `verifyTask` to get fresh failure evidence, then include it:
```go
// Get current verification state to provide context to heal
verifyResult := e.verifyTask(ctx, task)
failure := fmt.Sprintf(
    "Manual heal triggered.\nTask %d failed verification: %v\n"+
    "Task: %s\nFiles: %v\nAcceptance criteria: %v",
    taskID, verifyResult.Errors, task.Description, task.Files, task.AcceptanceCriteria,
)
```

---

## 6. Codebase Intelligence (CodeMap)

### 6.1 `codeintel.go` — `FormatContext` Hardcodes `min(3, reasons)` — Too Few Reasons

**File**: [`codeintel.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/codeintel/codeintel.go#L183-L184)

**Problem**:
```go
sb.WriteString(" — " + strings.Join(sf.Reasons[:min(3, len(sf.Reasons))], "; "))
```
Only 3 reasons are shown per file. For files with high relevance, the agent sees a truncated explanation of *why* the file was recommended, reducing the signal. Bumping this to 5 would provide richer context for a tiny cost.

**Fix**: `min(3, len(sf.Reasons))` → `min(5, len(sf.Reasons))`

---

### 6.2 `codeintel.go` — `codeIntelOnce` Prevents Rebuild After File Changes

**File**: [`engine.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/engine.go#L651-L665)

**Problem**: The code intelligence index is built exactly once per session:
```go
e.codeIntelOnce.Do(func() {
    idx := codeintel.NewIndexer(e.workDir)
    // ...build...
    e.codeIntel = idx
})
```
After the Execute phase writes new files, the index is stale — it reflects the pre-execution state of the codebase. Later tasks (especially in multi-task plans where task N depends on files written by task N-1) get recommendations based on the old file structure.

**Fix**: Rebuild the index between task groups (not between every task — that would be too expensive):
```go
// In runExecute, after each group completes:
e.codeIntelOnce = sync.Once{} // reset
e.codeIntel = nil             // invalidate
// getCodeIntel() will rebuild on next call
```

---

## 7. Provider & Streaming

### 7.1 `engine.go` — `consumeStreamWithTools` Drops Partial Content on Error

**File**: [`engine.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/engine.go#L720-L726)

**Problem**:
```go
if err != nil {
    if chunk != nil && chunk.Delta != "" {
        content.WriteString(chunk.Delta)
    }
    return content.String(), nil, err
}
```
On stream error, the partially-accumulated tool calls (`builders` map) are discarded. If the LLM had successfully emitted 3 of 4 tool calls before the stream broke, those 3 calls are lost. The method returns an error, triggering a full heal retry, even though 75% of the work was done.

**Fix**: Finalize and return whatever tool calls were accumulated before the error:
```go
if err != nil {
    if chunk != nil && chunk.Delta != "" {
        content.WriteString(chunk.Delta)
    }
    // Return partial tool calls — caller can execute what was received
    partialCalls := finalizeToolCalls(builders, e)
    return content.String(), partialCalls, err
}
```

---

## 8. Session & Memory

### 8.1 `session/planning.go` — `MEMORY.md` is Read in Plan Phase But Never Written

**File**: [`plan.go`](file:///home/snigdha/Desktop/Helix/M31A/internal/workflow/plan.go#L174-L177)

**Problem**:
```go
memPath := filepath.Join(sessionDir, "MEMORY.md")
if mem, err := os.ReadFile(memPath); err == nil {
    ctx += "## Cross-Session Memory\n" + string(mem) + "\n\n"
}
```
The MEMORY.md is injected into the planning context but M31A has no code that *writes* to it at session end. The ledger tracks outcomes, but no persistent memory of patterns, preferred approaches, or lessons learned is accumulated. The cross-session memory infrastructure exists but is inert.

**Fix**: In `runShip`, after generating the demonstration, extract 3-5 key facts from the current session and append them to MEMORY.md:
```go
// Append a memory entry summarizing this session's key learnings
memEntry := fmt.Sprintf(
    "\n## Session %s (%s)\n- Goal: %s\n- Tasks done: %d/%d\n- Project: %s (%s)\n",
    e.sessionID, time.Now().Format("2006-01-02"),
    goal, done, total, project.ProjectType, project.Framework,
)
appendToMemory(memPath, memEntry)
```
This is using an existing file path and existing session data — zero new features.

---

## Summary Table

| # | Component | File | Effort | Impact |
|---|-----------|------|--------|--------|
| 1.1 | Inner-monologue reasoning protocol | `base.md` | Tiny | High |
| 1.2 | Dependency output verification | `execute-task.md` | Tiny | High |
| 1.3 | Root cause taxonomy in heal | `self-heal.md` | Tiny | High |
| 1.4 | Task granularity rule | `plan-format.md` | Tiny | Medium |
| 1.5 | Observation-first discuss questions | `discuss-questions.md` | Tiny | Medium |
| 1.6 | Error budget for autonomous mode | `autonomous.md` | Tiny | Medium |
| **2.1** | **Heal loop missing ToolUse/CodeQuality prompts** | **`execute.go`** | **1 line** | **Critical** |
| 2.2 | Structured tool error format | `execute.go` | Small | High |
| 2.3 | File context cap too low (12K→32K) | `execute.go` | Small | High |
| 2.4 | Verify skipped tasks with files | `verify.go` | Small | Medium |
| 3.1 | Document `cachedBasePromptOnce` lifetime | `engine.go` | Tiny | Low |
| 3.2 | AutoDream: smart sampling vs. head-truncation | `autodream.go` | Small | Medium |
| **3.3** | **Plan refinement: end-truncation not head-truncation** | **`plan.go`** | **1 line** | **High** |
| 4.1 | Lower project-size threshold for moderate | `classify.go` | 1 line | Medium |
| 4.2 | Validate `acceptance_criteria` in task schema | `engine_parse.go` | Small | High |
| 5.1 | File-existence check after heal | `execute.go` | Small | High |
| 5.2 | Manual heal with fresh verification context | `engine.go` | Small | Medium |
| 6.1 | More reasons in FormatContext | `codeintel.go` | 1 char | Low |
| **6.2** | **Invalidate code intel after task group** | **`engine.go`** | **Small** | **High** |
| 7.1 | Return partial tool calls on stream error | `engine.go` | Small | Medium |
| 8.1 | Write to MEMORY.md at session end | `ship.go` | Small | Medium |

---

## Prioritized Starting Points

### Highest ROI (most impact, least effort):

1. **`execute.go` L471** — Add `ToolUse` and `CodeQuality` to the heal system prompt. Single line change. Fixes the most critical agent capability gap.

2. **`plan.go` L200-204** — Reverse truncation direction for plan refinement (keep end, not beginning). Single line. Immediately improves plan refinement quality.

3. **`engine_parse.go` validateTasks** — Require `acceptance_criteria`. Forces the plan LLM to always provide verifiable success conditions.

4. **`engine.go` code intel invalidation** — Reset `codeIntelOnce` between task groups. Multi-task plans currently run later tasks with stale intelligence.

5. **Prompt upgrades (1.1, 1.2, 1.3)** — All three are single-paragraph additions to existing prompt files. No code changes needed. Dramatically sharpens reasoning quality for all three core phases.
