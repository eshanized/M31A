# Phase 9: Architecture Upgrade & Directory Restructuring - Research

**Researched:** 2026-07-16
**Domain:** Go module restructuring, package decomposition, interface-driven architecture
**Confidence:** HIGH (codebase analysis) / MEDIUM (pattern recommendations from Go community)

## Summary

Phase 9 restructures the M31A Go codebase: (1) move `pkg/` contents into `internal/` since there are no external consumers, (2) delete `internal/types/` alias layer and import `pkg/types/` directly, (3) split TUI into screen sub-packages, (4) group tools by domain, (5) decompose the workflow engine with extracted interfaces. The key insight from codebase analysis is that **132 source files** import `internal/types/` while only **22 files** import `pkg/types/` directly. The `internal/types/` alias layer exists solely to break circular imports between `internal/provider` and `internal/tools`. After restructuring, the canonical import will be `pkg/types/` everywhere, eliminating one layer of indirection.

The codebase has 172 TUI `.go` files, 88 tools `.go` files, and 94 workflow `.go` files. The three largest files are `internal/workflow/engine.go` (1707 lines), `internal/tui/app.go` (777 lines), and `internal/tools/dispatcher.go` (492 lines). Screen-specific models already exist as individual files (`home_model.go`, `repl_model.go`, `settings_model.go`, etc.) but live flat in `internal/tui/` — they need to move to `internal/tui/screens/<name>/`.

**Primary recommendation:** Execute restructuring in 4 atomic plans: (1) type layering cleanup, (2) package moves (pkg/ -> internal/, TUI screen splits, tool domain groups), (3) interface extraction + constructor injection, (4) verification. Each plan must be independently compilable and testable.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Conservative splitting -- only split where there's a clear domain boundary. Do not restructure healthy code.
- **D-02:** `internal/tui/` -> split screen-specific code into `internal/tui/screens/<name>/` (e.g., `screens/home/`, `screens/repl/`, `screens/settings/`). Keep shared concerns (routing, theme, layout) in `tui/` root.
- **D-03:** `internal/workflow/` -> extract shared concerns into sub-packages: `workflow/engine/` for core orchestration, `workflow/phases/` for phase runners, `workflow/streaming/` for LLM streaming. Keep phase files named as-is.
- **D-04:** `internal/tools/` -> group tools by domain: `tools/fileops/` (read/write/edit/delete/move), `tools/exec/` (bash/devserver), `tools/search/` (glob/grep/webfetch/websearch), `tools/ai/` (agent/question). Keep dispatcher at `tools/` root.
- **D-05:** Delete `internal/types/` entirely. All internal packages import `pkg/types/` directly.
- **D-06:** Audit `pkg/` for types only used internally and move them into `internal/`. Keep `pkg/` lean.
- **D-07:** Domain-specific types live near their domain: TUI messages in `tui/tuitypes/`, workflow messages stay with workflow, tool types stay with tools.
- **D-08:** Move all `pkg/` contents into `internal/`. M31A has no external importers.
- **D-09:** Migration strategy: move `pkg/` as a subtree. Keep package names the same to minimize import churn.
- **D-10:** Keep each package as a separate unit even if small. Clear boundaries over consolidation.
- **D-11:** Extract a clean `WorkflowEngine` interface that TUI imports. Implementation behind the interface.
- **D-12:** Group TUI handler files by domain: `handlers/workflow.go`, `handlers/config.go`, `handlers/navigation.go`.
- **D-13:** Define clear interfaces at package boundaries (WorkflowRunner, ToolExecutor, ProviderRegistry).
- **D-14:** Use constructor injection for wiring. No singletons or package-level state.

### the agent's Discretion
- File naming conventions within new sub-packages
- Import ordering in reorganized files
- Whether to do the migration in one commit or incremental atomic commits
- Exact placement of borderline files

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| NFR-4 | Maintainability: 75% coverage, gofmt-clean, golangci-lint, conventional commits | Interface extraction reduces coupling; sub-packages improve discoverability; test coverage targets maintained by keeping package boundaries clear |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Type definitions | `pkg/types/` (moved to `internal/types/`) | — | Canonical types shared across all layers |
| TUI screen rendering | `internal/tui/screens/<name>/` | `internal/tui/` (routing) | Each screen owns its model+view+update |
| TUI shared concerns | `internal/tui/` (root) | `internal/tui/components/` | Routing, theme, layout stay at root |
| Workflow orchestration | `internal/workflow/engine/` | `internal/workflow/phases/` | Engine is the central coordinator |
| Phase execution | `internal/workflow/phases/` | `internal/workflow/engine/` | Each phase is a focused implementation |
| Tool execution | `internal/tools/dispatcher.go` | `internal/tools/<domain>/` | Dispatcher orchestrates, domain packages implement |
| Tool implementations | `internal/tools/<domain>/` | `internal/tools/` (root) | Domain grouping for discoverability |
| Provider abstraction | `internal/provider/` (unchanged) | — | Already well-structured with interface |
| Session management | `internal/session/` (moved from `pkg/`) | — | No external consumers |

## Standard Stack

### Core (No New Dependencies Required)

This phase restructures existing code -- it does not add new libraries. All tools used are Go standard tooling:

| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| `goimports` | latest | Auto-fix import paths after moves | Official Go tool, handles bulk import rewriting |
| `golangci-lint` | v2 (configured) | Verify no lint regressions after restructure | Already in project, catches unused imports, shadow vars |
| `go vet` | stdlib | Verify no circular imports or type errors | Built into Go toolchain |
| `go build ./...` | stdlib | Compile check after each plan | Fastest feedback loop |

### Supporting

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `gopls rename` | Rename packages/types across codebase | Only if renaming types during cleanup |
| `go test ./...` | Verify all tests pass after each change | Every plan completion |
| `go list -deps` | Verify import graph is clean | Final verification plan |

**No package installations needed.** This phase only restructures existing code.

## Package Legitimacy Audit

> **Not applicable** -- this phase installs no external packages. All changes are structural refactoring of existing code.

## Architecture Patterns

### System Architecture Diagram (Post-Restructuring)

```text
cmd/m31a/main.go
    |
    v
internal/
    |-- config/           (unchanged)
    |-- log/              (unchanged)
    |-- errors/           (unchanged)
    |-- fileutil/         (unchanged)
    |-- git/              (unchanged)
    |-- tokens/           (unchanged)
    |-- context/          (unchanged)
    |-- decision/         (unchanged)
    |-- codeintel/        (unchanged)
    |
    |-- types/            (moved from pkg/types/, canonical definitions)
    |-- session/          (moved from pkg/session/)
    |-- keychain/         (moved from pkg/keychain/)
    |-- metrics/          (moved from pkg/metrics/)
    |-- compaction/       (moved from pkg/compaction/)
    |-- ledger/           (moved from pkg/ledger/)
    |-- retry/            (moved from pkg/retry/)
    |-- rollback/         (moved from pkg/rollback/)
    |-- taskrunner/       (moved from pkg/taskrunner/)
    |-- bisect/           (moved from pkg/bisect/)
    |-- arbitrage/        (moved from pkg/arbitrage/)
    |-- autodream/        (moved from pkg/autodream/)
    |-- coordinator/      (moved from pkg/coordinator/)
    |-- history/          (moved from pkg/history/)
    |-- narrative/        (moved from pkg/narrative/)
    |-- skills/           (moved from pkg/skills/)
    |-- errors_pub/       (moved from pkg/errors/)
    |
    |-- provider/         (unchanged, well-structured)
    |
    |-- tools/
    |   |-- dispatcher.go (stays, orchestrator)
    |   |-- defaults.go   (stays, registration)
    |   |-- permissions.go (stays)
    |   |-- interface.go  (stays)
    |   |-- fileops/      (NEW: file read/write/edit/delete/move)
    |   |-- exec/         (NEW: bash, devserver)
    |   |-- search/       (NEW: glob, grep, webfetch, websearch)
    |   |-- ai/           (NEW: agent, question)
    |   |-- subagent/     (stays)
    |   +-- ...support files stay at root
    |
    |-- workflow/
    |   |-- engine/       (NEW: core engine orchestration)
    |   |-- phases/       (NEW: phase runners)
    |   |-- streaming/    (NEW: LLM streaming logic)
    |   |-- ...support files stay at workflow root
    |
    +-- tui/
        |-- app.go, app_update.go, app_view.go (stays)
        |-- app_routing.go (stays, routing logic)
        |-- router.go (stays)
        |-- screens/     (NEW: screen sub-packages)
        |   |-- home/
        |   |-- repl/
        |   |-- settings/
        |   |-- first_run/
        |   |-- plan/
        |   |-- execute/
        |   |-- verify/
        |   |-- ship/
        |   |-- ...etc
        |-- components/  (stays, reusable)
        |-- commands/    (stays)
        |-- layout/      (stays)
        |-- streaming/   (stays)
        |-- theme/       (stays)
        |-- tuitypes/    (stays)
        +-- handlers/    (NEW: grouped handler files)
```

### Pattern 1: Type Alias Removal (internal/types/ -> pkg/types/)

**What:** Delete `internal/types/` entirely. All 132 files that import `internal/types` change to import `pkg/types` directly.

**When to use:** When `internal/types/` only contains aliases to `pkg/types/` (verified -- this is the case).

**Migration approach:**
```go
// BEFORE (132 files):
import m31types "github.com/eshanized/M31A/internal/types"
// Usage: m31types.WorkflowPhase, m31types.ToolCall, etc.

// AFTER (132 files):
import m31types "github.com/eshanized/M31A/pkg/types"
// Usage: m31types.WorkflowPhase, m31types.ToolCall, etc. (unchanged)
```

**Key insight:** Since all aliases use `= pkgTypes.XXX` syntax, the types are identical. Changing the import path is a pure rename -- no code logic changes. `goimports` handles this automatically.

**Risk:** If any file imports BOTH `internal/types` and `pkg/types`, there will be a conflict. Must audit for this pattern first. Current analysis shows only `internal/types/types.go` itself and `internal/types/constants.go`/`git.go`/`plan.go`/`toolcall.go` import `pkg/types` -- these files are deleted entirely.

### Pattern 2: Screen Sub-Package Extraction (TUI)

**What:** Move screen-specific model/view/update files from `internal/tui/` into `internal/tui/screens/<name>/`.

**When to use:** When a screen has its own model struct, View() method, and Update() handler -- which all 34 screens do.

**Example for Home screen:**
```go
// internal/tui/screens/home/home.go
package home

import (
    "github.com/charmbracelet/bubbletea"
    "github.com/eshanized/M31A/internal/tui/tuitypes"
    // ... other imports
)

type Model struct {
    // Home screen state
    logo        string
    prompt      string
    suggestions []string
    tips        []string
    width       int
    height      int
}

func New() *Model {
    return &Model{}
}

func (m *Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    // Home-specific update logic
    return m, nil
}

func (m *Model) View() string {
    // Home-specific view
    return ""
}
```

**Routing stays in `tui/` root:**
```go
// internal/tui/app_routing.go (updated)
func (m *AppState) initScreenUpdaters() {
    m.screenUpdaters[tuitypes.ScreenHome] = func(msg tea.Msg) tea.Cmd {
        if m.homeModel == nil {
            m.homeModel = home.New()
        }
        result, cmd := m.homeModel.Update(msg)
        if result != nil {
            m.homeModel = result.(*home.Model)
        }
        return cmd
    }
    // ... other screens
}
```

**Key constraint:** Bubble Tea's Elm architecture requires that `Update()` returns `(tea.Model, tea.Cmd)`. Screen sub-packages must return concrete types so `tui/` root can type-assert. Use `tea.Model` interface for the public API, concrete type internally.

### Pattern 3: Tool Domain Grouping

**What:** Group tool implementations by domain while keeping dispatcher at root.

**When to use:** When tools share dependencies or conceptual grouping (file operations, execution, search, AI).

**File-to-domain mapping (verified from codebase):**

| File | Domain | Sub-package |
|------|--------|-------------|
| `fileread.go`, `filewrite.go`, `filelist.go`, `filedelete.go`, `filemove.go`, `edit.go` | fileops | `tools/fileops/` |
| `bash.go`, `bash_unix.go`, `bash_windows.go`, `bash_sandbox_*.go`, `devserver.go` | exec | `tools/exec/` |
| `glob.go`, `grep.go`, `webfetch.go`, `websearch.go` | search | `tools/search/` |
| `agent.go`, `question.go`, `memory.go` | ai | `tools/ai/` |
| `codemap.go`, `codecomplexity.go` | analysis | Keep at root (only 2 files) |
| `git.go` | vcs | Keep at root (only 1 file) |
| `todo.go`, `todoread.go` | workflow | Keep at root (only 2 files) |
| `dispatcher.go`, `defaults.go`, `permissions.go`, `interface.go`, `constants.go` | core | Keep at root |
| `output_store.go`, `concurrency.go`, `dns_cache.go`, `metrics.go` | infra | Keep at root |

**Interface pattern for domain packages:**
```go
// internal/tools/fileops/fileops.go
package fileops

import (
    "github.com/eshanized/M31A/internal/types"
    "github.com/eshanized/M31A/internal/config"
)

// FileRead implements types.Tool for reading files.
type FileRead struct {
    workDir string
}

func NewFileRead(workDir string) *FileRead {
    return &FileRead{workDir: workDir}
}

func (f *FileRead) Name() string        { return "FileRead" }
func (f *FileRead) Description() string  { return "Read file contents" }
func (f *FileRead) RiskLevel() types.RiskLevel { return types.RiskSafe }
func (f *FileRead) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    // implementation
}
```

**Registration stays centralized in `defaults.go`:**
```go
// internal/tools/defaults.go (updated)
import (
    "github.com/eshanized/M31A/internal/tools/fileops"
    "github.com/eshanized/M31A/internal/tools/exec"
    "github.com/eshanized/M31A/internal/tools/search"
    "github.com/eshanized/M31A/internal/tools/ai"
)

func DefaultDispatcher(...) (*Dispatcher, error) {
    d := NewDispatcher(cfg)
    // Register fileops tools
    d.Register(fileops.NewFileRead(workDir))
    d.Register(fileops.NewFileWrite(workDir, backupDir))
    // Register exec tools
    d.Register(exec.NewBash(workDir))
    // ... etc
}
```

### Pattern 4: Workflow Engine Decomposition

**What:** Split `engine.go` (1707 lines) into focused sub-packages while preserving the state machine.

**Approach (conservative -- per D-01):**
- Keep `engine.go` as the orchestrator but extract into `workflow/engine/` sub-package
- Move phase runners into `workflow/phases/` (each phase file stays as-is, just under new path)
- Extract LLM streaming logic into `workflow/streaming/`
- Keep `state_machine.go`, `phase_coordinator.go`, `context_builder.go`, `prompt_builder.go` at `workflow/` root

**Wire-up pattern:**
```go
// internal/workflow/engine/engine.go
package engine

import (
    "github.com/eshanized/M31A/internal/workflow/phases"
    "github.com/eshanized/M31A/internal/workflow/streaming"
)

type Engine struct {
    // Core state
    state *WorkflowState
    sm    *StateMachine
    
    // Delegates
    phaseRunner phases.Runner
    streamer    *streaming.Streamer
}

func New(opts EngineOptions) *Engine {
    return &Engine{
        state:       NewWorkflowState(opts),
        sm:          NewStateMachine(),
        phaseRunner: phases.NewRunner(opts),
        streamer:    streaming.NewStreamer(opts.Provider),
    }
}
```

### Anti-Patterns to Avoid

- **Moving files without updating all imports:** Every file move requires updating ALL importers. Use `goimports` after each move. Verify with `go build ./...`.
- **Breaking the Elm architecture:** Screen sub-packages must NOT mutate shared state from goroutines. All mutations go through `Update()` returning `tea.Cmd`.
- **Creating import cycles:** `tools/fileops/` must NOT import `tools/dispatcher.go`. The dispatcher imports domain packages, not the reverse.
- **Over-splitting:** D-01 says "conservative splitting." Only create sub-packages where there are 3+ related files. Two-file domains stay at root.
- **Losing type identity:** `types.Tool` interface must remain consistent across all packages. Do not create local `Tool` interfaces in sub-packages.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Import path rewriting | Manual find-replace | `goimports -w ./...` | Handles edge cases, grouped imports, aliased imports |
| Circular import detection | Custom lint script | `go build ./...` + `go vet` | Go compiler detects cycles at build time |
| Package dependency graph | Manual tracking | `go list -deps ./internal/...` | Standard Go tooling, accurate |
| Type identity verification | Custom tests | `go build ./...` compiles | If types are wrong, build fails |

**Key insight:** Go's compiler is the best restructuring validator. After every move, run `go build ./...`. If it compiles, the import graph is clean. No custom tooling needed.

## Common Pitfalls

### Pitfall 1: Dual Import Conflict
**What goes wrong:** A file imports both `internal/types` and `pkg/types` after partial migration, causing ambiguous type references.
**Why it happens:** Some files may already import `pkg/types` directly (22 files do) while also using `internal/types` aliases.
**How to avoid:** Before migrating, grep for files that import BOTH packages. If found, remove the `internal/types` import from those files first, then delete `internal/types/` entirely.
**Warning signs:** `go build` errors about "ambiguous selector" or "multiple packages named types"

### Pitfall 2: go:embed Path Breakage
**What goes wrong:** `//go:embed templates/...` directives reference paths relative to the source file. Moving `engine.go` into `engine/` sub-package breaks embed paths.
**Why it happens:** `go:embed` paths are relative to the file containing the directive, not the module root.
**How to avoid:** Keep `go:embed` directives at `workflow/` root, or move the embedded files with the code. Verify with `go build` after any move involving embedded resources.
**Warning signs:** `go build` errors about "pattern contains no files" or embed directives not resolving

### Pitfall 3: Constructor Injection Cascade
**What goes wrong:** Adding constructor parameters to fix package-level state requires changing every call site, which may span multiple packages.
**Why it happens:** Removing singletons means every consumer must receive dependencies through constructors.
**How to avoid:** Phase the DI introduction: first extract interfaces, then add constructors to new sub-packages only. Don't refactor existing constructors in the same plan.
**Warning signs:** `go build` errors about "not enough arguments in call to"

### Pitfall 4: Test Import Path Breakage
**What goes wrong:** Test files import the old package paths and break after moves.
**Why it happens:** Tests often import the same packages as production code.
**How to avoid:** Run `go test ./...` after every move. `goimports` handles test files too.
**Warning signs:** Test files fail to compile after package moves

### Pitfall 5: Circular Import After Tool Domain Split
**What goes wrong:** `tools/fileops/` imports `tools/dispatcher.go` for `OutputStore`, creating a cycle.
**Why it happens:** Tool implementations currently access dispatcher internals directly.
**How to avoid:** Define shared interfaces in `tools/interface.go`. Dispatcher passes dependencies through method parameters, not package-level imports. Domain packages depend only on `types/` and `config/`.
**Warning signs:** `go build` error: "import cycle not allowed"

## Code Examples

### Moving pkg/ to internal/ (Mechanical)

```go
// BEFORE:
import "github.com/eshanized/M31A/pkg/session"

// AFTER:
import "github.com/eshanized/M31A/internal/session"

// Run this across all files:
// goimports -w ./...
```

### Screen Sub-Package with Interface

```go
// internal/tui/screens/repl/repl.go
package repl

import (
    tea "github.com/charmbracelet/bubbletea"
    "github.com/eshanized/M31A/internal/tui/tuitypes"
    m31types "github.com/eshanized/M31A/pkg/types"
)

// Model is the REPL screen state.
type Model struct {
    messages   []m31types.Message
    input      string
    streaming  bool
    width      int
    height     int
    workflow   tuitypes.WorkflowRunner // interface, not concrete
}

// New creates a REPL model with injected dependencies.
func New(wf tuitypes.WorkflowRunner) *Model {
    return &Model{workflow: wf}
}

func (m *Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.KeyMsg:
        // handle input
    case m31types.StreamChunkMsg:
        // handle streaming
    }
    return m, nil
}

func (m *Model) View() string {
    // render REPL
    return ""
}
```

### Interface Extraction at Package Boundary

```go
// internal/tui/tuitypes/interfaces.go (existing, updated)

// WorkflowRunner abstracts workflow engine operations for TUI.
type WorkflowRunner interface {
    RunPhase(ctx context.Context, phase m31types.WorkflowPhase, goal string) error
    CurrentPhase() m31types.WorkflowPhase
    IsRunning() bool
    Cancel()
}

// ToolExecutor abstracts tool execution for permission requests.
type ToolExecutor interface {
    Execute(ctx context.Context, call m31types.ToolCall) (m31types.ToolResult, error)
    RequestPermission(call m31types.ToolCall) bool
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `internal/types/` aliases | Direct `pkg/types/` imports | Phase 9 | 132 files change import path |
| Flat `internal/tui/` (172 files) | `tui/screens/<name>/` sub-packages | Phase 9 | ~60 screen files move |
| Flat `internal/tools/` (88 files) | `tools/<domain>/` sub-packages | Phase 9 | ~20 tool files move |
| `pkg/` for all shared code | `internal/` (no external consumers) | Phase 9 | ~17 packages move |

**No deprecated patterns in use** -- the current codebase follows standard Go conventions. This phase is purely structural reorganization.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Go standard testing + race detector |
| Config file | none -- uses `go test ./...` |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| NFR-4 | All tests pass after restructure | integration | `go test ./... -count=1` | Yes (existing tests) |
| NFR-4 | No circular imports | build | `go build ./...` | N/A (compile check) |
| NFR-4 | gofmt-clean | lint | `gofmt -l ./...` | N/A (format check) |
| NFR-4 | golangci-lint clean | lint | `make lint` | Yes (.golangci.yml) |

### Sampling Rate
- **Per task commit:** `go build ./...` + `go test ./... -count=1 -short`
- **Per wave merge:** `make check` (fmt -> tidy -> vet -> lint -> test)
- **Phase gate:** `make check` + `go build ./...` + manual verification of import graph

### Wave 0 Gaps
- None -- existing test infrastructure covers all phase requirements
- All 180+ test files will be verified after each restructuring step

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Not applicable -- no auth changes |
| V3 Session Management | no | Not applicable -- session code moves, no logic changes |
| V4 Access Control | no | Not applicable -- permission system unchanged |
| V5 Input Validation | no | Not applicable -- validation logic moves, not changes |
| V6 Cryptography | no | Not applicable -- no crypto changes |

### Known Threat Patterns for Restructuring

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Import cycle allowing privilege escalation | Tampering | Go compiler rejects circular imports at build time |
| Package-level state leaking between goroutines | Information Disclosure | Constructor injection eliminates singletons |

**Security impact:** LOW. This phase restructures code paths without changing runtime behavior. All security controls (permissions, keychain, sandbox) remain functionally identical.

## Risk Assessment

### High-Risk Areas

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Breaking all imports (132 files) | HIGH (intentional) | HIGH | `goimports -w ./...` handles bulk rewriting. Verify with `go build ./...` |
| go:embed path breakage in workflow | MEDIUM | HIGH | Keep embed directives at workflow root. Test with `go build` after moves |
| Circular imports after tool domain split | MEDIUM | HIGH | Domain packages import only `types/` and `config/`. Dispatcher imports domains |
| Screen model interface mismatch | LOW | MEDIUM | Use `tea.Model` interface for public API. Type-assert in routing |
| Test file import breakage | HIGH (expected) | LOW | `goimports` handles test files. `go test ./...` catches any misses |

### Verification Strategy

**Atomic verification:** Each plan must be independently compilable and testable:

1. **Plan 1 (Type cleanup):** After deleting `internal/types/`, run `go build ./...` and `go test ./...`
2. **Plan 2 (Package moves):** After each package move, run `go build ./...`. After all moves, run `make check`
3. **Plan 3 (Interface extraction):** After each interface extraction, run `go build ./...` and `go test ./...`
4. **Plan 4 (Final verification):** Full `make check` + import graph audit

**Import graph audit command:**
```bash
# Verify no cycles
go build ./...

# Verify all packages compile
go list ./internal/... | wc -l

# Verify test coverage maintained
go test ./... -coverprofile=coverage.out
go tool cover -func=coverage.out | tail -1
```

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | No external consumers of `pkg/` -- safe to move to `internal/` | pkg/ -> internal/ | HIGH -- would break external importers. User confirmed no external consumers in D-08 |
| A2 | `internal/types/` only contains aliases to `pkg/types/` | Type cleanup | LOW -- verified by reading all 6 files in `internal/types/` |
| A3 | All 34 screens have separate model/view/update files | Screen decomposition | MEDIUM -- verified by file listing. Some screens may share models |
| A4 | `goimports` can handle all import path rewrites automatically | Tool usage | LOW -- `goimports` is standard for this. Manual fixes may be needed for edge cases |

**If this table is empty:** All claims in this research were verified or cited -- no user confirmation needed.

## Open Questions

1. **Workflow engine decomposition depth**
   - What we know: `engine.go` is 1707 lines with phase runners, streaming, and orchestration
   - What's unclear: Whether splitting into `workflow/engine/`, `workflow/phases/`, `workflow/streaming/` creates too many small packages vs. the benefit
   - Recommendation: Start with extracting phase runners only (conservative per D-01). Keep streaming logic in engine unless it exceeds 500 lines after extraction

2. **Screen sub-package granularity**
   - What we know: 34 screens exist, some very small (e.g., `confirm_quit`)
   - What's unclear: Whether tiny screens warrant their own sub-package
   - Recommendation: Only create sub-packages for screens with 3+ files or 200+ lines. Smaller screens stay in `tui/` root

3. **Handler file grouping**
   - What we know: D-12 says group `handlers/workflow.go`, `handlers/config.go`, `handlers/navigation.go`
   - What's unclear: Whether creating a `handlers/` sub-package adds value vs. keeping files in `tui/` root with naming convention
   - Recommendation: Use `app_handlers_workflow.go` naming in `tui/` root (current pattern). Sub-package adds import overhead without clear benefit for internal TUI files

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go | Build/test | ✓ | 1.26.5 | — |
| goimports | Import rewriting | ✓ (via `make fmt`) | latest | Manual import fixes |
| golangci-lint | Lint verification | ✓ (via `make lint`) | v2 | — |
| git | Version control | ✓ | — | — |

**Missing dependencies with no fallback:** None.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: all files read from `internal/`, `pkg/`, `cmd/m31A/`
- `internal/types/types.go` -- verified all 147 lines are aliases to `pkg/types/`
- `internal/workflow/engine.go` -- verified 1707 lines, embedded resources, import structure
- `internal/tui/app_routing.go` -- verified routing pattern for screen decomposition
- `internal/tools/defaults.go` -- verified registration pattern for domain grouping

### Secondary (MEDIUM confidence)
- Go standard library documentation on module system, `internal/` enforcement
- Bubble Tea documentation on Elm architecture and screen decomposition patterns

### Tertiary (LOW confidence)
- None -- all findings verified against actual codebase

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH -- no new dependencies needed, all Go standard tooling
- Architecture: HIGH -- patterns derived from actual codebase structure and existing conventions
- Pitfalls: HIGH -- all pitfalls verified against actual import patterns and file structures

**Research date:** 2026-07-16
**Valid until:** 2026-08-16 (30 days -- stable Go codebase, no fast-moving dependencies)
