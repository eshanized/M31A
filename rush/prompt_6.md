You are a senior Go developer implementing Phase 6 of the M31A terminal AI coding assistant. This is the largest and most complex phase (complexity 9/10, 6 weeks estimated).

## Context

M31A is a Go 1.22+ terminal AI coding assistant. Phases 0-5 are complete with 352+ passing tests. The binary launches a full Bubble Tea TUI with REPL, settings, resume, first-run, and permission screens. Provider layer (OpenRouter + Zen), tool system (Bash, FileRead, FileWrite, Glob, Grep), config/keychain, session management, and token estimation all work.

This phase implements the six-phase workflow engine: Initialize → Discuss → Plan → Execute → Verify → Ship.

## Existing Code You Must Not Break

- `internal/provider/` — LLMProvider interface, registry, OpenRouter/Zen clients, SSE parser, reasoning normalization, model cache, auto-fallback
- `internal/tui/` — Bubble Tea app (app.go), REPL (repl.go), first-run (firstrun.go), settings (settings.go), resume (resume.go), streaming (streaming.go), health (health.go), header (header.go), statusbar (statusbar.go), types (types.go)
- `internal/tui/components/` — message.go, toolcard.go, thinking.go, permission.go
- `internal/tui/theme/` — theme system
- `internal/tools/` — bash.go, fileread.go, filewrite.go, glob.go, grep.go, dispatcher.go, interface.go
- `internal/config/` — loader.go, types.go
- `internal/tokens/` — estimator.go
- `internal/types/` — types.go, constants.go
- `internal/errors/` — errors.go
- `pkg/keychain/` — keychain.go, keychain_linux.go, keychain_darwin.go, keychain_windows.go, errors.go
- `pkg/session/` — manager.go, save.go, checkpoint.go, planning.go
- `cmd/m31a/main.go` — full bootstrap with config, keychain, registry, TUI launch

## What to Build

### 1. Git Operations (`internal/git/`)

Create a git wrapper that handles all git operations needed by the workflow engine.

```go
package git

type Git struct {
    workDir string
}

func New(workDir string) *Git
func (g *Git) Init() error
func (g *Git) IsRepo() bool
func (g *Git) Add(paths ...string) error
func (g *Git) AddAll() error
func (g *Git) Commit(message string) error
func (g *Git) CommitWithFiles(message string, paths ...string) error
func (g *Git) Log(oneline bool, since string) ([]CommitInfo, error)
func (g *Git) Diff(ref1, ref2 string) (string, error)
func (g *Git) DiffStaged() (string, error)
func (g *Git) Status() (string, error)
func (g *Git) HeadHash() (string, error)
func (g *Git) CreateBranch(name string) error
func (g *Git) CurrentBranch() (string, error)
func (g *Git) ResetSoft(commit string) error
func (g *Git) ResetHard(commit string) error
func (g *Git) StashPush(message string) error
func (g *Git) StashPop() error
```

**CommitInfo struct:**
```go
type CommitInfo struct {
    Hash      string    `json:"hash"`
    ShortHash string    `json:"short_hash"`
    Author    string    `json:"author"`
    Message   string    `json:"message"`
    Timestamp time.Time `json:"timestamp"`
}
```

**Implementation details:**
- All operations use `exec.Command("git", ...)` with `Dir: workDir`
- `AddAll()` runs `git add -A`
- `Commit()` runs `git add -A && git commit -m "<message>"`
- `Log(oneline, since)` runs `git log --oneline --since=<since>` or full format
- Parse log output into `[]CommitInfo`
- `Diff(ref1, ref2)` runs `git diff ref1..ref2`
- `DiffStaged()` runs `git diff --cached`
- `HeadHash()` runs `git rev-parse HEAD`
- `IsRepo()` runs `git rev-parse --git-dir` and checks exit code
- `Init()` runs `git init`
- All errors wrapped with context (e.g., `fmt.Errorf("git init: %w", err)`)
- No external git libraries — pure `exec.Command`

**Tests:** 16 tests covering: init repo, is_repo detection, add/commit, log parsing, diff output, head hash, branch operations, status, reset soft/hard, stash. Use temp directory for all tests. Each test creates a temp git repo.

---

### 2. Task Runner (`pkg/taskrunner/`)

Implement the task scheduler with dependency resolution and sequential execution.

```go
package taskrunner

type Runner struct {
    tasks    []types.Task
    status   map[int]types.TaskStatus
    results  map[int]TaskResult
}

type TaskResult struct {
    Success    bool
    Output     string
    Error      string
    CommitHash string
    DurationMs int64
}

func New(tasks []types.Task) *Runner
func (r *Runner) Schedule() ([][]int, error)       // topological sort → execution groups
func (r *Runner) ExecuteGroup(group []int, fn ExecuteFunc) error
func (r *Runner) Status(id int) types.TaskStatus
func (r *Runner) Results() map[int]TaskResult
func (r *Runner) AllDone() bool
func (r *Runner) Summary() (total, done, failed, skipped int)
```

**ExecuteFunc:**
```go
type ExecuteFunc func(ctx context.Context, task types.Task) TaskResult
```

**Topological sort:**
- Build dependency graph from `task.Dependencies`
- Kahn's algorithm: compute in-degree, process zero-in-degree nodes first
- Cycle detection: if processed count < total tasks after algorithm completes → `ErrCircularDependency`
- Self-reference detection: if `task.Dependencies` contains `task.ID` → error
- Return `[][]int` — groups of task IDs that can run sequentially (within each group, tasks run one at a time in V1)
- Groups ordered by dependency depth

**Execution:**
- `ExecuteGroup()` iterates task IDs in the group sequentially
- For each task: check all dependencies completed successfully; if any failed/skipped → mark `StatusSkipped`
- Call `ExecuteFunc` for each runnable task
- Store result, update status
- If task fails → `StatusFailed`
- If task succeeds → `StatusDone`, store commit hash from result

**Dependency blocking:**
- Before executing a task, check all dependencies' status
- If any dep is `StatusFailed` or `StatusSkipped` → mark current task `StatusSkipped`
- If any dep is not `StatusDone` → skip (shouldn't happen with correct grouping)

**Tests:** 14 tests covering: linear dependency chain, diamond dependency, independent tasks (single group), circular dependency detection, self-reference detection, failed dependency blocking, skip propagation, execute function integration, status tracking, summary, empty task list, single task, all tasks independent, complex DAG (3 levels).

---

### 3. Workflow Engine Core (`internal/workflow/engine.go`)

Create the workflow state machine that orchestrates all six phases.

```go
package workflow

type Engine struct {
    sessionID  string
    workDir    string
    backupDir  string
    planningDir string
    provider   provider.LLMProvider
    modelID    string
    git        *git.Git
    dispatcher *tools.Dispatcher
    tokens     *tokens.Estimator
    sessionMgr *session.Manager
}

func NewEngine(sessionID, workDir, backupDir string, p provider.LLMProvider, modelID string, dispatcher *tools.Dispatcher, tokenEst *tokens.Estimator, sessionMgr *session.Manager) *Engine

func (e *Engine) RunPhase(ctx context.Context, phase types.WorkflowPhase, goal string) (*PhaseResult, error)
func (e *Engine) Transition(ctx context.Context, from, to types.WorkflowPhase) error
```

**PhaseResult:**
```go
type PhaseResult struct {
    Phase       types.WorkflowPhase
    Success     bool
    Messages    []types.Message    // conversation messages from this phase
    Tasks       []types.Task       // tasks generated (for Plan phase)
    Error       string
    DurationMs  int64
}
```

**Phase orchestration:**
- Each phase is a separate function: `runInitialize()`, `runDiscuss()`, `runPlan()`, `runExecute()`, `runVerify()`, `runShip()`
- `RunPhase()` dispatches to the correct function based on `phase` parameter
- `Transition()` saves checkpoint before transition, writes `STATE.md` with new phase
- Context pruning: each phase builds its own `ChatRequest` with only relevant context
- All phases log via `slog` (use `internal/log/` patterns)

**Shared system prompt:**
Define a constant system prompt used by all phases:
```
You are M31A, a terminal AI coding assistant. You help users build software through
a structured six-phase workflow. You write clean, correct Go code. You use tools
(Bash, FileRead, FileWrite, Glob, Grep) to interact with the filesystem and shell.
You think before acting — use reasoning to plan your approach.
```

**Tests:** 8 tests covering: phase dispatch, checkpoint on transition, STATE.md update, context pruning per phase, system prompt inclusion, error handling, engine initialization, session directory setup.

---

### 4. Initialize Phase (`internal/workflow/initialize.go`)

```go
func (e *Engine) runInitialize(ctx context.Context, goal string) (*PhaseResult, error)
```

**Behavior:**
1. Parse goal from user input (store in `ProjectState.Goal`)
2. Project type detection: scan cwd for `package.json` (Node.js), `go.mod` (Go), `Cargo.toml` (Rust), `pyproject.toml` (Python), `pom.xml` (Java), `requirements.txt` (Python), `Makefile` (C/C++), `CMakeLists.txt` (C/C++)
3. If cwd not a git repo: `e.git.Init()` automatically
4. Create `planning/` directory via `os.MkdirAll`
5. Write initial `PROJECT.md` with goal, project type, empty framework
6. Write initial `STATE.md` with phase="initialize", timestamp
7. Save checkpoint
8. Return `PhaseResult{Success: true, Phase: PhaseInitialize}`

**Project type detection logic:**
```go
func detectProjectType(workDir string) string {
    detectors := map[string]string{
        "go.mod":            "go",
        "package.json":      "nodejs",
        "Cargo.toml":        "rust",
        "pyproject.toml":    "python",
        "requirements.txt":  "python",
        "pom.xml":           "java",
        "Makefile":          "cc",
        "CMakeLists.txt":    "cc",
    }
    for file, typ := range detectors {
        if _, err := os.Stat(filepath.Join(workDir, file)); err == nil {
            return typ
        }
    }
    return "unknown"
}
```

**Transition to Discuss:** After Initialize completes successfully, the engine automatically transitions to Discuss phase. The TUI handles the screen transition.

**Tests:** 10 tests covering: goal parsing, project type detection (each detector), git init on non-repo, no-init on existing repo, planning directory creation, PROJECT.md content, STATE.md content, checkpoint saved, empty directory detection.

---

### 5. Discuss Phase (`internal/workflow/discuss.go`)

```go
func (e *Engine) runDiscuss(ctx context.Context, goal string) (*PhaseResult, error)
```

**Behavior:**
1. Build context: system prompt + goal + `MEMORY.md` (if exists in session dir) + `PROJECT.md` content
2. Build `ChatRequest` with messages:
   - System message: system prompt
   - User message: "The user wants to build: <goal>. Here is the project context:\n<PROJECT.md content>\n\nAsk 2-4 clarifying questions to understand the requirements better. Number each question. Be specific and concise."
3. Stream LLM via `provider.ChatCompletionStream()`
4. Parse response: extract 2-4 numbered questions (regex: `^\d+\.\s+(.+)$` or `\*\*Q\d+\*\*:\s*(.+)`)
5. Return `PhaseResult` with messages containing the questions
6. TUI renders questions inline, user answers one at a time
7. After all answers collected: append Q&A to `PROJECT.md` under `## Questions` section
8. Save updated `PROJECT.md`
9. Save checkpoint
10. Transition to Plan automatically

**Context building:**
```go
func (e *Engine) buildDiscussContext(goal string) []types.Message {
    var messages []types.Message
    messages = append(messages, types.Message{Role: "system", Content: systemPrompt})

    // Load MEMORY.md if exists
    if mem, err := os.ReadFile(filepath.Join(e.sessionMgr.BaseDir(), e.sessionID, "MEMORY.md")); err == nil {
        messages = append(messages, types.Message{Role: "user", Content: "Memory from previous sessions:\n" + string(mem)})
    }

    // Load PROJECT.md
    project, _ := session.LoadProject(e.planningDir)
    ctx := fmt.Sprintf("Goal: %s\nProject Type: %s\nFramework: %s", project.Goal, project.ProjectType, project.Framework)
    messages = append(messages, types.Message{Role: "user", Content: ctx + "\n\nAsk 2-4 clarifying questions..."})

    return messages
}
```

**Question parsing:**
```go
func parseQuestions(content string) []string {
    // Match numbered questions: "1. What framework..." or "**Q1**: What framework..."
    re := regexp.MustCompile(`(?:^|\n)\d+\.\s+(.+?)(?:\n|$|\*\*)`)
    matches := re.FindAllStringSubmatch(content, -1)
    var questions []string
    for _, m := range matches {
        if len(m) > 1 {
            questions = append(questions, strings.TrimSpace(m[1]))
        }
    }
    // Fallback: split by double newline, filter short lines
    if len(questions) == 0 {
        for _, line := range strings.Split(content, "\n\n") {
            line = strings.TrimSpace(line)
            if len(line) > 10 && (strings.Contains(line, "?") || strings.HasPrefix(line, "What") || strings.HasPrefix(line, "How") || strings.HasPrefix(line, "Which")) {
                questions = append(questions, line)
            }
        }
    }
    // Cap at 4 questions
    if len(questions) > 4 {
        questions = questions[:4]
    }
    return questions
}
```

**Tests:** 12 tests covering: question parsing (numbered format, bold format, fallback), context building with MEMORY.md, context building without MEMORY.md, LLM streaming integration (mocked), answer collection, PROJECT.md update with Q&A, skip with defaults, transition to Plan, max 4 questions cap, empty response handling, multi-turn conversation.

---

### 6. Plan Phase (`internal/workflow/plan.go`)

```go
func (e *Engine) runPlan(ctx context.Context, goal string) (*PhaseResult, error)
```

**Behavior:**
1. Build context: system prompt + goal + Discuss Q&A + `MEMORY.md` + cwd file schema (list of existing files with sizes)
2. Build `ChatRequest` with messages:
   - System message: system prompt
   - User message: full context + "Generate a task list to accomplish the goal. Return a JSON array of tasks. Each task must have: id (int), action (string: Add/Modify/Delete/Create), description (string), dependencies (array of int, empty if none), files (array of string), acceptance_criteria (array of string). Do not include any text outside the JSON array."
3. Stream LLM via `provider.ChatCompletionStream()`
4. Parse response: extract JSON array from response content (strip markdown code blocks if present)
5. Schema validation:
   - All tasks have required fields
   - No self-references (task.ID not in task.Dependencies)
   - No circular dependencies (run topological sort, detect cycles)
   - All dependency IDs reference existing task IDs
   - IDs are unique
6. On validation failure: send errors back to LLM (max `types.MaxPlanRetries` = 3 retries)
   - Error message: "The task list has errors: <list of errors>. Fix and return corrected JSON."
7. After 3 failures: return error (TUI prompts user to enter tasks manually or skip to REPL)
8. On success: serialize to `TASKS.md` via `session.SaveTasks()`
9. Save checkpoint
10. Return `PhaseResult{Success: true, Tasks: tasks}`
11. Transition to Execute automatically

**Task JSON schema:**
```json
[
  {
    "id": 1,
    "action": "Create",
    "description": "Set up the Go module and main package",
    "dependencies": [],
    "files": ["go.mod", "cmd/m31a/main.go"],
    "acceptance_criteria": ["go build succeeds", "binary prints version"]
  }
]
```

**Schema validation:**
```go
func validateTasks(tasks []types.Task) []string {
    var errors []string
    idSet := make(map[int]bool)
    for _, t := range tasks {
        if t.ID == 0 {
            errors = append(errors, fmt.Sprintf("task %d: missing ID", t.ID))
        }
        if idSet[t.ID] {
            errors = append(errors, fmt.Sprintf("task %d: duplicate ID", t.ID))
        }
        idSet[t.ID] = true
        for _, dep := range t.Dependencies {
            if dep == t.ID {
                errors = append(errors, fmt.Sprintf("task %d: self-reference", t.ID))
            }
            if !idSet[dep] {
                errors = append(errors, fmt.Sprintf("task %d: references non-existent dependency %d", t.ID, dep))
            }
        }
        if t.Description == "" {
            errors = append(errors, fmt.Sprintf("task %d: missing description", t.ID))
        }
        if t.Action == "" {
            errors = append(errors, fmt.Sprintf("task %d: missing action", t.ID))
        }
    }
    // Cycle detection via topological sort
    if hasCycle(tasks) {
        errors = append(errors, "circular dependency detected")
    }
    return errors
}
```

**Tests:** 16 tests covering: task parsing from JSON, schema validation (missing fields, self-refs, cycles, invalid deps, duplicate IDs), retry on validation failure, max 3 retries, successful plan, empty task list, markdown code block stripping, context building with file schema, context building without file schema, TASKS.md content, checkpoint saved, transition to Execute, LLM error handling, malformed JSON response.

---

### 7. Execute Phase (`internal/workflow/execute.go`)

```go
func (e *Engine) runExecute(ctx context.Context, goal string) (*PhaseResult, error)
```

**Behavior:**
1. Load tasks from `TASKS.md` via `session.LoadTasks()`
2. Create `taskrunner.Runner` with tasks
3. Schedule: get execution groups via topological sort
4. For each group, execute tasks sequentially:
   - For each task: build pruned context (system prompt + `TASKS.md` + `PROJECT.md` + current task spec)
   - Build `ChatRequest` with tools enabled (dispatcher's tool list)
   - Stream LLM via `provider.ChatCompletionStream()`
   - Parse tool calls from response
   - Dispatch tool calls via `e.dispatcher.Execute()`
   - Collect results, feed back into conversation
   - When task complete: `e.git.CommitWithFiles("feat(task <id>): <description>", task.Files...)`
   - Update task status to `StatusDone`
   - Write `TASKS.md` and `STATE.md` after each task
   - If task fails: attempt self-heal (up to `types.MaxHealAttempts` = 2)
     - Self-heal: send failure + task spec + current file state to LLM, ask for fix
     - Retry task execution
     - After 2 failed heals: mark `StatusFailed`
5. Save checkpoint
6. Return `PhaseResult{Success: allDone, Tasks: tasks}`
7. Transition to Verify automatically

**Pruned context per task:**
```go
func (e *Engine) buildExecuteContext(task types.Task, tasks []types.Task) []types.Message {
    var messages []types.Message
    messages = append(messages, types.Message{Role: "system", Content: systemPrompt})

    // TASKS.md summary (not full content — just the table)
    taskSummary := formatTaskSummary(tasks)
    messages = append(messages, types.Message{Role: "user", Content: "Task list:\n" + taskSummary})

    // Current task spec
    taskSpec := fmt.Sprintf("Execute task %d: %s\nAction: %s\nDescription: %s\nFiles: %v\nDependencies: %v",
        task.ID, task.ID, task.Action, task.Description, task.Files, task.Dependencies)
    if len(task.AcceptanceCriteria) > 0 {
        taskSpec += "\nAcceptance criteria: " + strings.Join(task.AcceptanceCriteria, "; ")
    }
    messages = append(messages, types.Message{Role: "user", Content: taskSpec})

    return messages
}
```

**Tool call loop:**
```go
func (e *Engine) executeTaskWithTools(ctx context.Context, messages []types.Message, task types.Task) TaskResult {
    start := time.Now()

    // Build ChatRequest with tools
    req := provider.ChatRequest{
        Model:    e.modelID,
        Messages: messages,
        Tools:    e.buildToolDefinitions(),
        Stream:   false, // Execute uses non-streaming for tool dispatch simplicity
    }

    // Use non-streaming ChatCompletion (wrap stream iterator to consume fully)
    iterator, err := e.provider.ChatCompletionStream(ctx, req)
    if err != nil {
        return TaskResult{Success: false, Error: err.Error()}
    }
    defer iterator.Close()

    // Consume stream to get full response
    var assistantContent strings.Builder
    var toolCalls []types.ToolCall

    for {
        chunk, err := iterator.Next()
        if err == io.EOF {
            break
        }
        if chunk != nil {
            assistantContent.WriteString(chunk.Delta)
        }
    }

    // Parse tool calls from response (look for tool_use blocks in content)
    // ... parse logic ...

    // Dispatch tool calls
    for _, tc := range toolCalls {
        result, err := e.dispatcher.Execute(ctx, tc)
        // Feed result back into messages
        messages = append(messages, types.Message{
            Role:    "assistant",
            Content: assistantContent.String(),
            ToolCalls: []types.ToolCall{tc},
        })
        messages = append(messages, types.Message{
            Role:    "tool",
            Content: result.Output,
        })
    }

    // If no more tool calls, task is complete
    commitHash, _ := e.git.CommitWithFiles(
        fmt.Sprintf("feat(task %d): %s", task.ID, task.Description),
        task.Files...,
    )

    return TaskResult{
        Success:    true,
        Output:     assistantContent.String(),
        CommitHash: commitHash,
        DurationMs: time.Since(start).Milliseconds(),
    }
}
```

**Self-heal loop:**
```go
func (e *Engine) healTask(ctx context.Context, task types.Task, failure string) TaskResult {
    // Build heal context: failure + task spec + current file state
    messages := []types.Message{
        {Role: "system", Content: systemPrompt},
        {Role: "user", Content: fmt.Sprintf("Task %d failed: %s\n\nCurrent file state:\n%s\n\nFix the issue.", task.ID, failure, readTaskFiles(task.Files))},
    }

    req := provider.ChatRequest{Model: e.modelID, Messages: messages, Tools: e.buildToolDefinitions()}
    // Execute LLM, dispatch tools, commit fix
    // Return TaskResult with success/failure
}
```

**Tests:** 18 tests covering: single task execution, task with dependencies, tool call dispatch, git commit after task, task failure, self-heal attempt, self-heal success, self-heal failure (max attempts), STATE.md update after task, TASKS.md update after task, checkpoint saved, task skipped on dep failure, empty task list, task with no files to commit, tool execution error, LLM streaming error, context building, dependency ordering.

---

### 8. Verify Phase (`internal/workflow/verify.go`)

```go
func (e *Engine) runVerify(ctx context.Context, goal string) (*PhaseResult, error)
```

**Behavior:**
1. Load tasks from `TASKS.md`
2. For each task with `StatusDone`:
   - Check file existence: for each file in `task.Files`, verify it exists on disk
   - Syntax validation: for `.go` files, run `go build ./...`; for `.py` files, run `python -m py_compile`; for `.js/.ts` files, check basic syntax
   - Test execution: if `*_test.go` files exist, run `go test ./...`; if `*_test.py` exists, run `pytest`; if `*.test.js` exists, run the test command from `package.json`
3. If all checks pass: mark task `StatusDone` (already set)
4. If any check fails:
   - Attempt self-heal (same as Execute phase, max 2 attempts)
   - On heal success: re-verify
   - On heal failure (2 attempts): mark `StatusUnrecoverable`
5. If task is `StatusUnrecoverable`: trigger git bisect
   - `bisectResult := bisect.Run(sessionStartHash, headHash, checkFn)`
   - `checkFn` runs the verification check
   - Return offending commit diff for 3rd targeted heal attempt
6. Save updated `TASKS.md`
7. Save checkpoint
8. Return `PhaseResult{Success: allPassedOrSkipped}`
9. Transition to Ship automatically (if all tasks pass or are skipped)

**Verification logic:**
```go
type VerificationResult struct {
    TaskID    int
    FilesExist bool
    SyntaxOK   bool
    TestsOK    bool
    Errors    []string
}

func (e *Engine) verifyTask(task types.Task) VerificationResult {
    result := VerificationResult{TaskID: task.ID}

    // File existence
    for _, f := range task.Files {
        path := filepath.Join(e.workDir, f)
        if _, err := os.Stat(path); os.IsNotExist(err) {
            result.Errors = append(result.Errors, fmt.Sprintf("file not found: %s", f))
        }
    }
    result.FilesExist = len(result.Errors) == 0

    // Syntax validation
    for _, f := range task.Files {
        if strings.HasSuffix(f, ".go") {
            cmd := exec.CommandContext(ctx, "go", "build", "./...")
            cmd.Dir = e.workDir
            if out, err := cmd.CombinedOutput(); err != nil {
                result.Errors = append(result.Errors, fmt.Sprintf("go build failed: %s", string(out)))
                result.SyntaxOK = false
            } else {
                result.SyntaxOK = true
            }
        }
        // Similar for other languages
    }

    // Test execution
    if hasTestFiles(e.workDir, task.Files) {
        cmd := exec.CommandContext(ctx, "go", "test", "./...")
        cmd.Dir = e.workDir
        if out, err := cmd.CombinedOutput(); err != nil {
            result.Errors = append(result.Errors, fmt.Sprintf("go test failed: %s", string(out)))
            result.TestsOK = false
        } else {
            result.TestsOK = true
        }
    }

    return result
}
```

**Tests:** 14 tests covering: file existence check, syntax validation (Go build), test execution (Go test), verification pass, verification fail, self-heal trigger on failure, self-heal success, self-heal failure, bisect trigger on unrecoverable, STATE.md update, TASKS.md update, checkpoint saved, multi-task verification, skip on status_skipped.

---

### 9. Git Bisect (`pkg/bisect/`)

```go
package bisect

type Bisect struct {
    workDir string
    logger  *slog.Logger
}

type BisectResult struct {
    OffendingCommit CommitInfo
    Diff            string
}

func New(workDir string, logger *slog.Logger) *Bisect
func (b *Bisect) Run(sessionStartHash, headHash string, checkFn func() bool) (*BisectResult, error)
```

**Bisect logic:**
1. `git bisect start`
2. `git bisect good <sessionStartHash>`
3. `git bisect bad <headHash>`
4. Loop:
   - `git bisect run <checkFn>` — but since checkFn is a Go function, not a shell command:
   - `git bisect next` → get current commit
   - Check out commit: `git checkout <commit>`
   - Run `checkFn()` → if true (pass), `git bisect good`; if false (fail), `git bisect bad`
   - Continue until bisect complete
5. `git bisect log` → parse to find offending commit
6. `git diff <offending>^..<offending>` → get diff
7. `git bisect reset` → return to original state
8. Return `BisectResult{OffendingCommit, Diff}`

**Implementation:**
```go
func (b *Bisect) Run(sessionStartHash, headHash string, checkFn func() bool) (*BisectResult, error) {
    // Start bisect
    b.runGit("bisect", "start")
    b.runGit("bisect", "good", sessionStartHash)
    b.runGit("bisect", "bad", headHash)

    for {
        // Get current bisect commit
        out, _ := b.runGit("bisect", "log")
        if strings.Contains(out, "first bad commit") {
            // Bisect complete
            break
        }

        current, _ := b.runGit("rev-parse", "HEAD")

        // Run check
        if checkFn() {
            b.runGit("bisect", "good")
        } else {
            b.runGit("bisect", "bad")
        }
    }

    // Parse result
    log, _ := b.runGit("bisect", "log")
    offending := parseBisectLog(log)

    // Get diff
    diff, _ := b.runGit("diff", offending+"^.."+offending)

    // Reset
    b.runGit("bisect", "reset")

    return &BisectResult{
        OffendingCommit: CommitInfo{ShortHash: offending},
        Diff:            diff,
    }, nil
}
```

**Error handling:**
- Any git error → return `ErrBisectFailed`
- Bisect may fail if sessionStartHash is not ancestor of headHash → return descriptive error
- Always call `git bisect reset` in cleanup (use defer)

**Tests:** 10 tests covering: successful bisect (finds offending commit), checkFn always passes (no bad commit), checkFn always fails (first commit is bad), bisect reset on error, parse bisect log, diff extraction, non-ancestor error, empty bisect, bisect with merge commits, cleanup on panic.

---

### 10. Ship Phase (`internal/workflow/ship.go`)

```go
func (e *Engine) runShip(ctx context.Context, goal string) (*PhaseResult, error)
```

**Behavior:**
1. Final git commit: `e.git.Commit("chore(ship): complete session " + e.sessionID)`
2. Build summary:
   - Task count: total, done, failed, skipped
   - Commit log: `e.git.Log(oneline=true, since=sessionStart)`
   - Total duration (from session start timestamp)
3. Update ledger: append entry to `~/.m31a/LEDGER.md`
   ```markdown
   ## Session <id> — <date>
   - Model: <modelID>
   - Provider: <provider>
   - Tasks: <done>/<total>
   - Duration: <duration>
   - Goal: <goal>
   ```
4. Archive session: `e.sessionMgr.ArchiveSession(e.sessionID)`
5. Write final `STATE.md` with phase="ship", status="complete"
6. Save checkpoint
7. Return `PhaseResult{Success: true, Phase: PhaseShip}`

**Ledger update:**
```go
func appendLedgerEntry(sessionID, modelID, provider, goal string, done, total int, duration time.Duration) error {
    ledgerPath := filepath.Join(os.Getenv("HOME"), ".m31a", "LEDGER.md")

    entry := fmt.Sprintf("## Session %s — %s\n- Model: %s\n- Provider: %s\n- Tasks: %d/%d\n- Duration: %s\n- Goal: %s\n\n",
        sessionID, time.Now().Format("2006-01-02"), modelID, provider, done, total, duration, goal)

    // Append atomically: read existing, append, write
    existing, _ := os.ReadFile(ledgerPath)
    content := string(existing) + entry
    return atomicWrite(ledgerPath, []byte(content))
}
```

**Tests:** 10 tests covering: final git commit, summary building, ledger append (new file), ledger append (existing file), session archive, STATE.md final update, checkpoint saved, task summary with mixed statuses, duration calculation, empty task list shipping.

---

### 11. TUI Screen Additions

Add 4 new screen types to `internal/tui/types.go`:

```go
const (
    ScreenFirstRun Screen = iota
    ScreenREPL
    ScreenModelSelector
    ScreenSettings
    ScreenResume
    ScreenPermission
    ScreenPlan       // NEW
    ScreenExecute    // NEW
    ScreenVerify     // NEW
    ScreenShip       // NEW
)
```

### 12. Plan Screen (`internal/tui/plan.go`)

```go
type PlanModel struct {
    theme   theme.Theme
    tasks   []types.Task
    selected int
    width   int
    height  int
    // ... fields for cost/time display
}

func NewPlanModel(tasks []types.Task, t theme.Theme, modelID string, provider string, estCost float64, estTime time.Duration) *PlanModel
func (m *PlanModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg)
func (m *PlanModel) View() string
```

**Visual layout:**
```
┌──────────────────── Plan ────────────────────────┐
│ [A]ccept  [E]dit  [R]etry  [D]iff  Tab=Graph    │
│                                                  │
│  Tasks (left pane)        │  Cost/Time (right)   │
│  ────────────────────────  │  ─────────────────  │
│  [x] 1. Create go.mod      │  Model: claude-...   │
│       deps: -              │  Est cost: $0.12     │
│  [ ] 2. Add main.go        │  Est time: 5 min     │
│       deps: 1              │  Provider: OR        │
│  [ ] 3. Add tests          │                      │
│       deps: 2              │  [A]ccept plan       │
│                            │                      │
└──────────────────────────────────────────────────┘
```

**Keys:**
- `A` = accept → transition to Execute phase, return `AppMsg{Screen: ScreenExecute}`
- `E` = edit inline (V1: placeholder — full editing deferred)
- `R` = retry → re-run Plan phase, return `AppMsg{Screen: ScreenREPL}`
- `D` = diff preview → overlay showing predicted file changes (`+`/`~`/`-` indicators)
- `Tab` = dependency graph → toggle text-based DAG view

**Tests:** 8 tests covering: render plan view, task list display, cost/time panel, accept action, retry action, diff preview toggle, dependency graph toggle, selected task highlighting.

---

### 13. Execute Screen (`internal/tui/execute.go`)

```go
type ExecuteModel struct {
    theme    theme.Theme
    tasks    []types.Task
    current  int
    width    int
    height   int
    paused   bool
    // ... fields for progress bar, tool cards
}

func NewExecuteModel(tasks []types.Task, t theme.Theme) *ExecuteModel
func (m *ExecuteModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg)
func (m *ExecuteModel) View() string
```

**Visual layout:**
```
┌────────────────── Execute ───────────────────────┐
│ 3/8 complete  37%  [████████░░░░░░░░░░░]        │
│                                                  │
│  [x] 1. Create go.mod                            │
│  [>] 2. Add main.go         ← running            │
│  [ ] 3. Add tests                                │
│  [ ] 4. Add handler                              │
│  [-] 5. Add middleware     ← blocked (dep: 3)    │
│                                                  │
│  ┌─ Bash ─────────────────────────────────────┐  │
│  │ $ go build ./...                           │  │
│  │ [OK] Completed in 2.34s                    │  │
│  └────────────────────────────────────────────┘  │
│                                                  │
│  P=Pause  R=Resume  S=Skip                       │
└──────────────────────────────────────────────────┘
```

**Keys:**
- `P` = pause current task execution
- `R` = resume current task execution
- `S` = skip current task → mark `StatusSkipped`, move to next

**Progress bar:**
```
completed / total  XX%  [████████░░░░░░░░░░]
```

**Tests:** 10 tests covering: render execute view, progress bar, task status indicators (done/running/queued/blocked), pause action, resume action, skip action, live tool card display, transition to Verify on completion, selected task highlighting.

---

### 14. Verify Screen (`internal/tui/verify.go`)

```go
type VerifyModel struct {
    theme    theme.Theme
    tasks    []types.Task
    results  map[int]VerificationResult
    width    int
    height   int
}

func NewVerifyModel(tasks []types.Task, results map[int]VerificationResult, t theme.Theme) *VerifyModel
func (m *VerifyModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg)
func (m *VerifyModel) View() string
```

**Visual layout:**
```
┌────────────────── Verify ────────────────────────┐
│                                                  │
│  [x] 1. Create go.mod      ✓ Files exist         │
│                            ✓ Syntax OK            │
│                            ✓ Tests pass           │
│                                                  │
│  [x] 2. Add main.go        ✓ Files exist         │
│                            ✗ Syntax: go build    │
│                            [H] Self-heal          │
│                                                  │
│  [!] 3. Add tests          [UNRECOVERABLE]       │
│  BISECT: commit a1b2c3 broke test suite          │
│  diff: +removed important function               │
│                                                  │
│  H=Self-heal  S=Skip                             │
│                                                  │
└──────────────────────────────────────────────────┘
```

**Keys:**
- `H` = self-heal → re-run heal loop for selected task
- `S` = skip → mark `StatusSkipped`
- Auto-transition to Ship after all tasks pass or are skipped

**Tests:** 8 tests covering: render verify view, pass/fail checklist, self-heal action, skip action, bisect result display, unrecoverable task badge, auto-transition to Ship, selected task highlighting.

---

### 15. Ship Screen (`internal/tui/ship.go`)

```go
type ShipModel struct {
    theme    theme.Theme
    summary  ShipSummary
    width    int
    height   int
}

type ShipSummary struct {
    TaskDone    int
    TaskTotal   int
    TaskFailed  int
    TaskSkipped int
    Commits     []git.CommitInfo
    Duration    time.Duration
    SessionID   string
}

func NewShipModel(summary ShipSummary, t theme.Theme) *ShipModel
func (m *ShipModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg)
func (m *PlanModel) View() string  // note: returns string, not AppMsg for most cases
```

**Visual layout:**
```
┌────────────────── Ship ──────────────────────────┐
│                                                  │
│  ✓ Session complete!                             │
│                                                  │
│  Session: a1b2c3d4                               │
│  Duration: 12m 34s                               │
│                                                  │
│  Tasks: 6 done, 1 failed, 1 skipped              │
│                                                  │
│  Commits:                                        │
│  a1b2c34 feat(task 1): Create go.mod             │
│  b2c3d45 feat(task 2): Add main.go               │
│  c3d4e56 feat(task 3): Add tests                 │
│  d4e5f67 chore(ship): complete session a1b2c3d4  │
│                                                  │
│  [O] Open in browser  [N] New session  [R] REPL  │
└──────────────────────────────────────────────────┘
```

**Keys:**
- `O` = open in browser (detect dev server URL via `xdg-open`/`open`/`start`)
- `N` = new session → create new session, transition to Initialize
- `R` = return to REPL

**Tests:** 6 tests covering: render ship view, summary display, commit log, new session action, return to REPL action, open in browser.

---

### 16. Integration Test: Full Workflow

Create `internal/workflow/integration_test.go`:

**Test scenario:**
1. Create temp directory, initialize git repo
2. Create mock LLM provider with scripted responses for each phase:
   - Initialize: returns project type detection
   - Discuss: returns 2 questions
   - Plan: returns JSON task list (3 tasks)
   - Execute: returns tool calls (FileWrite for each task), no errors
   - Verify: all files exist, syntax OK
   - Ship: session complete
3. Run full workflow: Initialize → Discuss → Plan → Execute → Verify → Ship
4. Assert:
   - `planning/PROJECT.md` written with goal and Q&A
   - `planning/TASKS.md` written with 3 tasks
   - `planning/STATE.md` shows "ship" phase
   - `checkpoint.json` has 2 checkpoints (last 2 retained)
   - Git repo has 4+ commits (3 task commits + 1 ship commit)
   - Session archived to `archived/`
   - Ledger entry appended to `LEDGER.md`

**Mock provider:**
```go
type MockProvider struct {
    responses map[string]string  // phase → response content
    callCount int
}

func (m *MockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
    // Return iterator that yields pre-scripted response
}
```

**Tests:** 3 integration tests:
1. Happy path: full workflow succeeds
2. Plan retry: LLM returns invalid JSON twice, succeeds on third try
3. Verify failure + self-heal: task verification fails, self-heal fixes it

---

## Absolute Rules

1. **No CGO** — All packages must compile with `CGO_ENABLED=0`. Git operations use `exec.Command`, not C libraries.
2. **Bubble Tea single-threaded** — All TUI state mutations in `Update()`. No goroutine mutations.
3. **Atomic writes** — All file writes use temp file + `os.Rename()`.
4. **Context pruning** — Each phase builds its own `ChatRequest` with only relevant context. Do NOT carry full conversation history.
5. **V1 sequential execution** — Tasks run one at a time. No concurrency in task runner.
6. **No AskUserQuestion tool** — V1 uses native TUI text input for Discuss phase questions.
7. **No telemetry** — Zero phone-home, analytics, or external calls beyond provider APIs.
8. **Existing tests pass** — Do not modify any files from Phases 0-5 except to add new screen types to `internal/tui/types.go`.
9. **Error handling** — Return typed errors from `internal/errors/`, not `fmt.Errorf` strings.
10. **Self-heal limit** — Max 2 heal attempts per task. After 2 failures, mark `StatusUnrecoverable`.

## Deliverables

- `internal/git/` — git operations wrapper (16 tests)
- `pkg/taskrunner/` — task scheduler with topological sort (14 tests)
- `internal/workflow/engine.go` — workflow state machine (8 tests)
- `internal/workflow/initialize.go` — Initialize phase (10 tests)
- `internal/workflow/discuss.go` — Discuss phase (12 tests)
- `internal/workflow/plan.go` — Plan phase with schema validation (16 tests)
- `internal/workflow/execute.go` — Execute phase with tool dispatch (18 tests)
- `internal/workflow/verify.go` — Verify phase with self-heal (14 tests)
- `internal/workflow/ship.go` — Ship phase with ledger update (10 tests)
- `pkg/bisect/` — git bisect wrapper (10 tests)
- `internal/tui/plan.go` — Plan screen (8 tests)
- `internal/tui/execute.go` — Execute screen (10 tests)
- `internal/tui/verify.go` — Verify screen (8 tests)
- `internal/tui/ship.go` — Ship screen (6 tests)
- `internal/tui/types.go` — Updated with 4 new screen types
- `internal/workflow/integration_test.go` — Full workflow integration (3 tests)
- **163 new tests minimum**
- All existing 352+ tests still pass
- `go vet ./...` clean
- `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` succeeds

## File Creation Order

1. `internal/git/` — git operations (foundation for everything else)
2. `pkg/taskrunner/` — task scheduler (needed by Execute phase)
3. `pkg/bisect/` — git bisect wrapper (needed by Verify phase)
4. `internal/workflow/engine.go` — workflow engine core
5. `internal/workflow/initialize.go` — Initialize phase
6. `internal/workflow/discuss.go` — Discuss phase
7. `internal/workflow/plan.go` — Plan phase
8. `internal/workflow/execute.go` — Execute phase
9. `internal/workflow/verify.go` — Verify phase
10. `internal/workflow/ship.go` — Ship phase
11. `internal/tui/types.go` — Add 4 new screen types
12. `internal/tui/plan.go` — Plan screen
13. `internal/tui/execute.go` — Execute screen
14. `internal/tui/verify.go` — Verify screen
15. `internal/tui/ship.go` — Ship screen
16. `internal/workflow/integration_test.go` — Integration tests

Run `go mod tidy` after adding any new dependencies.

After implementation, run:
```
go mod tidy
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
go vet ./...
go test -race -cover ./...
```
