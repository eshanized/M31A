# Phase 7 Implementation Prompts

Each prompt below is self-contained and designed to be given to an AI coding agent. Prompts include context about existing types, interfaces, and patterns the new code must integrate with.

---

## P7.6 — Slash Command System

**File:** `internal/tui/commands.go` + `internal/tui/commands_test.go`

**Prompt:**

Create a slash command parsing and dispatch system for the M31A TUI. The system should parse user input starting with `/` and route to command handlers.

### Context

- The TUI is built with `charmbracelet/bubbletea`
- `AppState` in `internal/tui/app.go` is the main model with fields: `registry *provider.Registry`, `activeProvider string`, `activeModel *types.ModelInfo`, `sessionManager *session.Manager`, `config *config.Config`, `dispatcher *tools.Dispatcher`
- `types.WorkflowPhase` is a string enum with values: `PhaseIdle`, `PhaseInitialize`, `PhaseDiscuss`, `PhasePlan`, `PhaseExecute`, `PhaseVerify`, `PhaseShip`
- `pkg/session/checkpoint.go` has `SaveCheckpoint()`, `LoadCheckpoints()`, `LatestCheckpoint()` functions
- `internal/config/types.go` has `Config` struct with `Provider`, `Model`, `UI`, `Permissions`, `Features`, `Ledger`, `Ghost` sub-configs

### Types to define

```go
// CommandResult represents the outcome of a command execution.
type CommandResult struct {
    Success  bool
    Message  string          // User-facing status message
    Screen   *Screen         // Optional screen transition
    SessionID *string        // Optional new session ID
    Config   *config.Config  // Optional config update
}

// CommandHandler is a function that processes a command with args.
type CommandHandler func(args []string, ctx CommandContext) CommandResult

// CommandContext provides environment for command execution.
type CommandContext struct {
    Registry       *provider.Registry
    SessionManager *session.Manager
    SessionID      string
    Config         *config.Config
    Dispatcher     *tools.Dispatcher
    Git            *git.Git
}
```

### Commands to implement

| Command | Args | Description |
|---------|------|-------------|
| `/help` | none | List all available commands with descriptions |
| `/clear` | none | Clear current conversation context (reset messages) |
| `/status` | none | Show current session info: phase, model, provider, message count, token usage |
| `/model` | [id] | Show current model, or switch to model by ID (validates via registry) |
| `/provider` | [name] | Show current provider, or switch provider (uses registry.SetActive) |
| `/reset` | none | Reset to first-run screen, discarding current session |
| `/quit` | none | Exit the application |
| `/undo` | none | Restore the latest checkpoint (use session.LoadCheckpoints + reset to that phase) |
| `/compress` | none | Trigger context consolidation (stub for now — log warning if over threshold) |
| `/ledger` | none | Show recent session entries from LEDGER.md |
| `/rollback` | [hash] | Show commit chain (last 10) or reset to specific commit |
| `/sessions` | none | List recent sessions from session manager |
| `/goal` | [text] | Set or show the current session goal |
| `/phase` | [name] | Show or manually transition to a workflow phase |
| `/config` | [key] [value] | Show or set a config value |
| `/models` | none | List all cached models |

### Implementation requirements

1. **ParseCommand(input string) (name string, args []string, ok bool)** — extracts command name and args from input. Returns ok=false if input doesn't start with `/`.
2. **CommandRegistry struct** — maps command names to handlers. Methods: `Register(name, handler)`, `Get(name) (handler, bool)`, `List() []string`, `Execute(input, ctx) (CommandResult, bool)`.
3. **DefaultCommands() *CommandRegistry** — factory that registers all 16 commands above.
4. Each handler should validate args count and return appropriate error messages.
5. Use `fmt.Sprintf` for formatted output in `CommandResult.Message`.
6. For `/undo`: load latest checkpoint, return message with phase/timestamp info. If no checkpoint, return error message.
7. For `/compress`: check token usage from context, if over `types.AutoDreamThreshold` (0.60), return warning message.
8. For `/model`: if no args, show current model from context. If args, validate via `ctx.Registry.Get()` and show model details.
9. For `/provider`: if no args, show current. If args, call `ctx.Registry.SetActive(name)` and return success/error.
10. For `/rollback`: use `git.Log(false, "")` to get last 10 commits, format as list. If hash provided, validate it exists.

### Tests to write

- `TestParseCommand` — various inputs: `/help`, `/model gpt-4`, `/config key value`, `not a command`
- `TestCommandRegistry` — register, get, list, execute
- `TestHelpCommand` — returns list of commands
- `TestStatusCommand` — returns session info
- `TestModelCommand` — show current, switch model
- `TestProviderCommand` — show current, switch provider
- `TestUndoCommand` — with and without checkpoints
- `TestCompressCommand` — under and over threshold
- `TestRollbackCommand` — list commits, validate hash
- `TestInvalidCommands` — unknown command, wrong arg count

### Key patterns

- Commands are pure functions — no direct AppState mutation. Return `CommandResult` with desired state changes.
- The TUI's `Update()` method handles applying `CommandResult` (screen transitions, config saves, etc.).
- Use `strings.Fields()` for arg splitting, `strings.TrimPrefix(input, "/")` for command name.
- Error messages should be user-friendly: `"unknown command: /foo. Type /help for available commands."`

---

## P7.4 — Commit Rollback Chain

**File:** `pkg/rollback/rollback.go` + `pkg/rollback/rollback_test.go`

**Prompt:**

Create a git rollback chain package that allows browsing recent commits and resetting to any point in history.

### Context

- `internal/git/git.go` provides `Git` struct with methods: `Log(oneline bool, since string) ([]CommitInfo, error)`, `ResetSoft(commit string) error`, `ResetHard(commit string) error`, `StashPush(message string) error`, `HeadHash() (string, error)`, `Diff(ref1, ref2) (string, error)`, `Status() (string, error)`
- `internal/git/git.go` `CommitInfo` struct has: `Hash`, `ShortHash`, `Author`, `Message`, `Timestamp`
- `pkg/session/checkpoint.go` has checkpoint lifecycle that may need to be preserved during rollback

### Types to define

```go
// RollbackEntry represents a commit in the rollback chain.
type RollbackEntry struct {
    CommitInfo git.CommitInfo
    Diff       string            // Diff from this commit to HEAD
    IsCurrent  bool              // Whether this is the current HEAD
    HasCheckpoint bool           // Whether a checkpoint exists after this commit
}

// RollbackResult holds the outcome of a rollback operation.
type RollbackResult struct {
    Success      bool
    PreviousHead string
    NewHead      string
    ChangesStashed bool
    Message      string
}

// Rollback wraps git operations for rollback.
type Rollback struct {
    git    *git.Git
}
```

### Methods to implement

1. **New(git *git.Git) *Rollback** — constructor.

2. **Chain(limit int) ([]RollbackEntry, error)** — returns the last `limit` commits as rollback entries. For each commit, compute the diff to HEAD. Mark the current HEAD entry. Check if any checkpoint exists after each commit by loading checkpoints and comparing timestamps.

3. **CurrentHead() (string, error)** — shortcut to get current HEAD hash.

4. **Preview(hash string) (string, error)** — returns the diff between the given commit and HEAD. This is what would be lost if rolling back.

5. **SoftReset(hash string) (*RollbackResult, error)** — performs a `git reset --soft` to the given commit. Changes after the commit remain staged. Stashes any uncommitted changes first if `git Status()` shows dirty state. Returns result with previous/new HEAD.

6. **HardReset(hash string) (*RollbackResult, error)** — performs a `git reset --hard` to the given commit. All changes after the commit are discarded. Returns result.

7. **SafeReset(hash string) (*RollbackResult, error)** — hybrid approach: stashes current changes, does `reset --hard`, then pops the stash. This preserves changes as stashed entries rather than losing them.

8. **HasUncommittedChanges() (bool, error)** — checks `git Status()` for any modified/untracked files.

### Implementation requirements

1. Chain() should limit to 20 commits by default, accept a custom limit.
2. Chain() entries should be ordered newest-first (HEAD at index 0).
3. Preview() should return human-readable diff, capped at 50,000 chars (use `types.BashOutputLimit`).
4. All reset methods should stash first if there are uncommitted changes (to prevent data loss).
5. RollbackResult.Message should be user-friendly: `"Rolled back from abc123 to def456. 3 commits undone. Changes stashed."`
6. Use `git.Log(false, "")` to get full commit history. Take the first `limit` entries.
7. For diff computation: `git.Diff(entry.Hash, "HEAD")`.
8. For checkpoint comparison: load all checkpoints, compare `Checkpoint.Timestamp` with `CommitInfo.Timestamp`.

### Tests to write

Use a temporary git repo for all tests (similar to `internal/git/git_test.go` pattern).

- `TestChain` — create 5 commits, verify chain returns 5 entries in correct order
- `TestChain_Limit` — create 10 commits, request limit 3, verify 3 returned
- `TestPreview` — create 2 commits, preview first, verify diff contains expected changes
- `TestSoftReset` — create 3 commits, soft reset to first, verify HEAD changed, changes staged
- `TestHardReset` — create 3 commits, hard reset to first, verify HEAD changed, changes discarded
- `TestSafeReset` — make uncommitted changes, safe reset to prior commit, verify changes preserved in stash
- `TestHasUncommittedChanges` — clean repo returns false, create file returns true
- `TestChain_Empty` — repo with no commits returns empty chain
- `TestPreview_InvalidHash` — non-existent hash returns error
- `TestSoftReset_CurrentHead` — reset to current head is no-op, returns success

---

## P7.2 — Cost-Aware Model Arbitrage

**File:** `pkg/arbitrage/arbitrage.go` + `pkg/arbitrage/arbitrage_test.go`

**Prompt:**

Create a model arbitrage package that estimates task complexity, compares model costs across providers, and recommends the optimal model based on cost/quality tradeoffs.

### Context

- `types.ModelInfo` struct has: `ID`, `Provider`, `Name`, `Description`, `ContextLength`, `Pricing` (map or struct with input/output/token costs), `Architecture`, `TopProvider`, `Capabilities CapFlags`
- `internal/provider/interface.go` has `LLMProvider.EstimateCost(modelID string, usage types.Usage) float64`
- `internal/config/types.go` has `ModelConfig.AutoArbitrage` (bool) and `ModelConfig.ArbitrageThreshold` (float64)
- `types.Task` has `Description`, `Action`, `Files`, `Dependencies` — these indicate complexity
- `types.AutoDreamThreshold = 0.60` is the context usage threshold for compression

### Types to define

```go
// ComplexityLevel represents estimated task complexity.
type ComplexityLevel string

const (
    ComplexitySimple  ComplexityLevel = "simple"
    ComplexityModerate ComplexityLevel = "moderate"
    ComplexityComplex  ComplexityLevel = "complex"
)

// CostEstimate holds pricing for a model operation.
type CostEstimate struct {
    ModelID       string
    Provider      string
    InputCost     float64  // Cost for input tokens
    OutputCost    float64  // Cost for output tokens
    TotalCost     float64  // Combined cost
    Currency      string   // "USD"
    InputTokens   int      // Estimated input tokens
    OutputTokens  int      // Estimated output tokens
}

// ArbitrageRecommendation holds the result of model selection.
type ArbitrageRecommendation struct {
    RecommendedModel  CostEstimate
    Alternatives      []CostEstimate  // Next cheapest options
    Complexity        ComplexityLevel
    Savings           float64         // Savings vs most expensive option
    Reason            string          // Why this model was chosen
}

// Scorer estimates task complexity.
type Scorer struct {
    // thresholds for complexity classification
    SimpleThreshold  int  // max tokens for "simple"
    ComplexThreshold int  // min tokens for "complex"
}
```

### Functions to implement

1. **NewScorer(simpleThreshold, complexThreshold int) *Scorer** — constructor. Defaults: simple=2000, complex=8000.

2. **Score(task types.Task) (ComplexityLevel, int)** — estimates complexity based on:
   - Number of files involved
   - Number of dependencies
   - Description length (word count)
   - Action keywords (keywords like "refactor", "implement", "design" → higher complexity)
   - Returns complexity level and estimated token count

3. **EstimateTokens(complexity ComplexityLevel, task types.Task) (input int, output int)** — estimates token counts:
   - Simple: input 1000-3000, output 500-1500
   - Moderate: input 3000-8000, output 1500-4000
   - Complex: input 8000-20000, output 4000-10000
   - Adjust based on file count (add 500 tokens per file)

4. **CompareModels(models []types.ModelInfo, inputTokens, outputTokens int) []CostEstimate** — for each model, compute cost estimate using the model's pricing data. Sort by total cost ascending.

5. **Recommend(models []types.ModelInfo, task types.Task, threshold float64) (*ArbitrageRecommendation, error)** — main arbitrage function:
   - Score the task complexity
   - Estimate token counts
   - Compare all models
   - If cheapest model's capabilities match the complexity level, recommend it
   - If threshold is set (e.g., 0.05 = 5%), allow spending up to threshold% more for a better model
   - Return recommendation with reasoning

6. **ShouldArbitrage(currentCost, alternativeCost, threshold float64) bool** — returns true if the cost difference exceeds the threshold percentage.

### Implementation requirements

1. Complexity scoring keywords:
   - Simple: "fix", "add", "update", "change", "rename"
   - Moderate: "implement", "create", "refactor", "restructure"
   - Complex: "design", "architect", "migrate", "rewrite", "system"

2. Token estimation should consider: system prompt (~500 tokens), task context (~200 tokens per file), conversation history (variable).

3. `CompareModels` should handle models with missing pricing data gracefully (skip or use $0.00 estimate).

4. `Recommend` should prefer models with matching capabilities:
   - Complex tasks → prefer models with larger context windows
   - Simple tasks → prefer cheapest model

5. Cost calculation: `inputTokens * inputPricePerToken + outputTokens * outputPricePerToken`.

6. The `threshold` parameter means: "accept a model up to threshold% more expensive if it has better capabilities for this task."

### Tests to write

- `TestScore_Simple` — "fix typo in README" → ComplexitySimple
- `TestScore_Moderate` — "implement user authentication" → ComplexityModerate
- `TestScore_Complex` — "design and implement microservice architecture" → ComplexityComplex
- `TestScore_Files` — task with 5 files → higher complexity
- `TestScore_Dependencies` — task with 3 deps → moderate complexity boost
- `TestEstimateTokens` — verify ranges for each complexity level
- `TestCompareModels` — 3 models with different pricing, verify sorted order
- `TestCompareModels_MissingPricing` — model with no pricing handled gracefully
- `TestRecommend_SimpleTask` — recommends cheapest model for simple task
- `TestRecommend_ComplexTask` — recommends capable model for complex task
- `TestRecommend_WithThreshold` — threshold allows spending more for better model
- `TestShouldArbitrage` — various threshold comparisons
- `TestArbitrage_NoModels` — returns error with no models
- `TestArbitrage_SingleModel` — returns the only option

---

## P7.5 — AutoDream Context Consolidation

**File:** `pkg/autodream/autodream.go` + `pkg/autodream/autodream_test.go`

**Prompt:**

Create an AutoDream package that consolidates conversation context when token usage exceeds a threshold, generating a compressed summary that preserves critical information.

### Context

- `types.Message` has `Role`, `Content`, `Segments []MessageSegment`, `ToolCalls []ToolCall`, `Usage *Usage`, `CreatedAt`
- `types.Usage` typically has `PromptTokens`, `CompletionTokens`, `TotalTokens`
- `types.AutoDreamThreshold = 0.60` (60% context usage triggers warning)
- `types.ContextWarningThreshold = 0.80` (80% is critical)
- `types.ModelInfo` has `ContextLength` (max tokens for the model)
- `pkg/session/` manages session lifecycle with `Session.Messages []types.Message`

### Types to define

```go
// ConsolidationResult holds the outcome of context consolidation.
type ConsolidationResult struct {
    OriginalMessages int      // Number of messages before consolidation
    ConsolidatedMessages int  // Number of messages after
    TokensSaved      int      // Estimated tokens saved
    Summary          string   // Generated summary text
    Preserved        []int    // Indices of messages that were preserved (not consolidated)
}

// ContextInfo describes current context usage.
type ContextInfo struct {
    TotalTokens    int
    UsedTokens     int
    UsagePercent   float64
    ContextLength  int
    MessageCount   int
}

// Consolidator performs context consolidation.
type Consolidator struct {
    threshold   float64  // Trigger threshold (default 0.60)
    warningThreshold float64  // Warning threshold (default 0.80)
}
```

### Methods to implement

1. **NewConsolidator(threshold, warningThreshold float64) *Consolidator** — constructor. Defaults: 0.60, 0.80.

2. **ContextInfo(messages []types.Message, contextLength int) ContextInfo** — computes current context usage:
   - Sum all `message.Usage.TotalTokens` (or estimate if Usage is nil)
   - Calculate usage percentage
   - Return message count

3. **ShouldConsolidate(messages []types.Message, contextLength int) bool** — returns true if usage exceeds threshold.

4. **Consolidate(messages []types.Message, contextLength int) (*ConsolidationResult, error)** — main consolidation logic:
   - Identify messages to consolidate (oldest first, skipping system messages and recent messages)
   - Generate a summary of consolidated messages
   - Preserve the last 5 messages (recent context)
   - Preserve all system messages
   - Replace consolidated messages with a single summary message
   - Return result with token savings

5. **GenerateSummary(messages []types.Message) string** — creates a human-readable summary of the given messages:
   - Group by role (user/assistant)
   - Extract key actions from tool calls
   - Summarize file changes
   - Format as: "User asked to X. Assistant did Y, Z. Files modified: a.go, b.go."

6. **WarningMessage(usagePercent float64) string** — returns appropriate warning based on usage level:
   - 60-80%: "Context usage at {pct}%. Consider using /compress to consolidate."
   - 80%+: "Context usage critical at {pct}%. Auto-compressing."

### Implementation requirements

1. Consolidation should preserve:
   - System messages (role == "system")
   - Last 5 messages (most recent context)
   - Messages with tool calls that resulted in file modifications
   - The initial goal/context message

2. Summary generation should be concise — max 500 tokens worth of text.

3. Token estimation for messages without Usage data: count words × 1.3 (rough token approximation).

4. The consolidated summary message should have role "assistant" and content prefixed with "[AutoDream Context Summary]".

5. ShouldConsolidate uses `threshold` from config (default 0.60).

6. ContextInfo should handle empty message lists gracefully (0 tokens, 0%).

### Tests to write

- `TestContextInfo` — various message counts, verify token sums
- `TestContextInfo_Empty` — empty messages returns 0%
- `TestShouldConsolidate` — under threshold false, over true
- `TestConsolidate` — 20 messages, verify consolidation preserves system + last 5
- `TestConsolidate_FewMessages` — under threshold, no consolidation needed
- `TestConsolidate_PreservesToolCalls` — messages with file modification tool calls preserved
- `TestGenerateSummary` — verify summary format
- `TestGenerate Summary_Empty` — empty returns ""
- `TestWarningMessage` — various usage levels
- `TestConsolidate_TokenEstimation` — messages without Usage data estimated correctly

---

## P7.3 — Cross-Session Learning Ledger Viewer

**File:** `pkg/ledger/ledger.go` + `pkg/ledger/ledger_test.go`

**Prompt:**

Create a ledger package that parses, filters, and aggregates session history from the LEDGER.md file, providing statistics and browsing capabilities.

### Context

- LEDGER.md is stored at `~/.m31a/LEDGER.md`
- Format per entry (written by `engine.go:appendLedgerEntry`):
```
## Session <id> — <date>
- Model: <model_id>
- Provider: <provider_name>
- Tasks: <done>/<total>
- Duration: <duration_string>
- Goal: <goal_text>
```
- `internal/config/types.go` has `LedgerConfig` struct
- `pkg/session/manager.go` has `ListSessions()` returning `[]SessionInfo`

### Types to define

```go
// LedgerEntry represents a parsed session entry from the ledger.
type LedgerEntry struct {
    SessionID string
    Date      time.Time
    Model     string
    Provider  string
    TasksDone int
    TasksTotal int
    Duration  time.Duration
    Goal      string
    Raw       string  // Original markdown
}

// LedgerStats aggregates statistics across ledger entries.
type LedgerStats struct {
    TotalSessions    int
    TotalTasksDone   int
    TotalTasks       int
    AvgDuration      time.Duration
    SuccessRate      float64  // tasks_done / tasks_total
    TopModels        map[string]int  // model → session count
    TopProviders     map[string]int  // provider → session count
    DateRange        LedgerDateRange
}

// LedgerDateRange holds the earliest and latest dates.
type LedgerDateRange struct {
    Earliest time.Time
    Latest   time.Time
}

// LedgerFilter provides filtering options for ledger queries.
type LedgerFilter struct {
    Provider    string
    Model       string
    DateFrom    time.Time
    DateTo      time.Time
    Limit       int
    Offset      int
}

// Ledger reads and queries the session ledger.
type Ledger struct {
    path string  // Path to LEDGER.md
}
```

### Methods to implement

1. **New(path string) *Ledger** — constructor. If path is empty, uses `~/.m31a/LEDGER.md`.

2. **Entries() ([]LedgerEntry, error)** — reads and parses the entire ledger file. Returns entries in reverse chronological order (newest first).

3. **EntriesFiltered(filter LedgerFilter) ([]LedgerEntry, error)** — returns entries matching the filter:
   - Provider: exact match on provider name
   - Model: substring match on model ID
   - DateFrom/DateTo: date range
   - Limit: max entries to return
   - Offset: skip first N entries

4. **Stats() (*LedgerStats, error)** — computes aggregate statistics across all entries:
   - Total sessions, tasks done, total tasks
   - Average duration
   - Success rate (tasks_done / tasks_total)
   - Top models (map of model ID → count, sorted by count)
   - Top providers (map of provider name → count)
   - Date range (earliest to latest entry)

5. **StatsFiltered(filter LedgerFilter) (*LedgerStats, error)** — statistics for entries matching the filter.

6. **RecentSessions(limit int) ([]LedgerEntry, error)** — shortcut for last N entries (equivalent to EntriesFiltered with Limit).

7. **Exists() bool** — returns true if the ledger file exists.

8. **Append(entry LedgerEntry) error** — appends a new entry to the ledger file. Creates file if it doesn't exist. Uses atomic write (temp file + rename).

### Implementation requirements

1. Parsing: split file by `## Session` headers, parse each block's key-value lines.
2. Date parsing: use `time.Parse("2006-01-02", dateStr)`.
3. Duration parsing: use `time.ParseDuration(durationStr)` — handles "1h30m", "45s", etc.
4. Handle malformed entries gracefully — skip and continue, don't fail the entire parse.
5. Entries should be trimmed of whitespace. Empty lines between entries are normal.
6. Atomic write for Append: write to temp file in same directory, then rename.
7. Stats should handle empty ledgers gracefully (0 values, no panics).

### Tests to write

Create a temporary ledger file for testing.

- `TestEntries` — parse 5 entries, verify all fields
- `TestEntries_Empty` — empty file returns empty slice
- `TestEntries_Malformed` — file with some bad entries, verify only valid ones returned
- `TestEntriesFiltered_Provider` — filter by provider
- `TestEntriesFiltered_Model` — filter by model substring
- `TestEntriesFiltered_DateRange` — filter by date range
- `TestEntriesFiltered_Limit` — limit results
- `TestEntriesFiltered_Offset` — skip first N
- `TestStats` — compute stats for 5 entries
- `Stats_Empty` — stats for empty ledger returns zero values
- `TestStatsFiltered` — stats for filtered entries
- `TestAppend` — append entry, verify it appears in Entries()
- `TestAppend_CreatesFile` — append to non-existent file creates it
- `TestExists` — file exists true, doesn't exist false
- `TestRecentSessions` — get last 3 entries

---

## P7.1 — Model Selector UI

**File:** `internal/tui/modelselector.go` + `internal/tui/modelselector_test.go`

**Prompt:**

Create a Model Selector TUI screen that allows users to browse, search, and select AI models across all registered providers.

### Context

- `types.ModelInfo` has: `ID`, `Provider`, `Name`, `Description`, `ContextLength`, `Pricing`, `Architecture`, `TopProvider`, `Capabilities CapFlags`
- `provider.Registry` has methods: `List() []string`, `Get(name) (LLMProvider, error)`, `ActiveProvider() LLMProvider`
- `LLMProvider` has: `FetchModels(ctx) ([]types.ModelInfo, error)`, `GetModel(id) (*types.ModelInfo, error)`, `Name() string`, `EstimateCost(modelID, usage) float64`
- `internal/tui/types.go` has `ScreenModelSelector` enum value already defined
- Bubble Tea pattern: model has `Init() tea.Cmd`, `Update(msg tea.Msg) (tea.Model, tea.Cmd)`, `View() string`
- Use `charmbracelet/lipgloss` for styling, `charmbracelet/bubbles/list` for list component

### Model struct to define

```go
type ModelSelectorModel struct {
    registry       *provider.Registry
    models         []types.ModelInfo
    list           list.Model
    filtered       []types.ModelInfo
    searchInput    textinput.Model
    width, height  int
    loading        bool
    errMsg         string
    selectedModel  *types.ModelInfo
    theme          theme.Theme
    providerFilter string  // Empty = all providers
    viewMode       int     // 0=list, 1=grid
}
```

### Messages to define

```go
type ModelsLoadedMsg struct {
    Models []types.ModelInfo
    Provider string
}

type ModelsLoadErrMsg struct {
    Error string
}
```

### Methods to implement

1. **NewModelSelector(registry *provider.Registry, theme theme.Theme) *ModelSelectorModel** — constructor. Initialize list with delegate, search input, and key bindings.

2. **Init() tea.Cmd** — triggers model loading. Returns a command that calls `registry.ActiveProvider().FetchModels()` for the active provider, then all other providers.

3. **Update(msg tea.Msg) (tea.Model, tea.Cmd)** — handle:
   - `tea.KeyMsg`: Enter (select model), Esc (cancel), `/` (focus search), `1`/`2` (switch view mode), `p`/`P` (filter by provider), `j/k` or up/down (navigate), `g/G` (go to top/bottom)
   - `ModelsLoadedMsg`: populate list
   - `ModelsLoadErrMsg`: show error
   - `textinput.Msg`: update search input, filter list
   - Search should filter on Model ID, Name, Description, and Provider

4. **View() string** — render:
   - Header: "Select Model" with provider filter indicator
   - Main area: list of models (name, provider, context length, estimated cost)
   - Search bar (visible when search is focused)
   - Footer: key bindings help
   - Loading spinner when fetching
   - Error display when models fail to load

5. **SelectedModel() *types.ModelInfo** — returns the currently selected model (for use after screen dismiss).

6. **loadModelsCmd() tea.Cmd** — returns a tea.Cmd that fetches models from all providers and sends `ModelsLoadedMsg`.

### List delegate

Create a custom `list.DefaultDelegate` that renders each model as:
```
[model_name]  [provider]  context: [N]  cost: $[X]/1M tokens
  [description truncated to 80 chars]
```

Use lipgloss colors: provider name in different color per provider (OpenRouter=purple, Zen=blue).

### Implementation requirements

1. The list should support up to 200 models without performance issues.
2. Search should be case-insensitive and match on ID, name, description, and provider.
3. Provider filter: pressing `p` cycles through registered providers. Show "All" when no filter.
4. Loading state: show spinner with "Loading models from {provider}..."
5. Error state: show error message with "r" to retry option.
6. View modes: list (default) shows compact info, grid shows more details.
7. Selection: Enter on a model sets `selectedModel` and signals screen transition.
8. Cancel: Esc returns to previous screen without selection.
9. The screen should auto-fetch models on init, showing loading state during fetch.
10. Cost display: use `provider.EstimateCost()` with a standard usage (e.g., 100K input, 50K output tokens) for comparison.

### Tests to write

- `TestModelSelector_Init` — verify loading state on init
- `TestModelSelector_ModelsLoaded` — verify list populated after load msg
- `TestModelSelector_Search` — search filters list
- `TestModelSelector_ProviderFilter` — provider filter works
- `TestModelSelector_SelectModel` — Enter selects model
- `TestModelSelector_Cancel` — Esc cancels
- `TestModelSelector_ErrorState` — error message displayed
- `TestModelSelector_View` — verify View() output structure
- `TestModelSelector_KeyBindings` — all key bindings functional

---

## P7.7 — Settings Screen Inline Editing + Model/Ledger Tabs

**File:** Update `internal/tui/settings.go` + update `internal/tui/settings_test.go`

**Prompt:**

Update the existing Settings screen to support inline editing of config values and add two new tabs: Model and Ledger.

### Context

- Current `SettingsModel` in `internal/tui/settings.go` has 4 tabs: General (0), Provider (1), Permissions (2), Features (3)
- `SettingsModel` has fields: `config`, `theme`, `activeTab`, `width`, `height`, `keychain`, `storeInKC`, `dirty`, `statusMsg`, `errMsg`
- Tab cycling uses `tab`/`shift+tab` with `m.activeTab = (m.activeTab + 1) % 4`
- `Save()` writes config to disk via `config.Save(m.config, m.configPath)`
- `config.Config` has sub-configs: `Provider`, `Model`, `UI`, `Permissions`, `Features`, `Ledger`, `Ghost`
- `pkg/arbitrage/` has arbitrage types and functions (if implemented)
- `pkg/ledger/` has ledger types and functions (if implemented)

### Changes required

1. **Add 2 new tabs**: Model (4), Ledger (5). Update `tabLabels` to 6 entries. Update tab cycling modulo from 4 to 6.

2. **Add inline editing state**:
```go
editingField string     // Which field is being edited (e.g., "provider.apiKey", "ui.maxIterations")
editInput    textinput.Model
editValue    string     // Current value being edited
```

3. **General tab edits**: Allow editing `UI.MaxIterations`, `UI.ShowTokenUsage`, `UI.ShowCostEstimate`.
   - Navigate with `j/k` or up/down within the tab
   - Press `e` to edit the focused field
   - Use `textinput.Model` for text entry
   - Press `Enter` to confirm, `Esc` to cancel

4. **Provider tab edits**: Allow editing `Provider.DefaultProvider`, API key (stored in keychain).
   - API key field should be masked (show `****`)
   - Use keychain for storage (existing `storeInKC` logic)

5. **Permissions tab edits**: Allow editing `Permissions.DefaultMode`, adding/removing permission rules.
   - Navigate rules with `j/k`
   - Press `a` to add rule, `d` to delete focused rule

6. **Features tab edits**: Allow toggling feature flags on/off.
   - Press `space` or `Enter` to toggle boolean flags
   - Flags: `AutodreamEnabled`, `SubagentEnabled`, `AutoBackup`, `ResumeOnStartup`, `AutoArbitrage`

7. **New Model tab**: Show current model, list available models, set default model.
   - Use `registry.GetModel()` and `registry.ActiveProvider().FetchModels()`
   - Display: model name, provider, context length, pricing
   - Press `Enter` on a model to set as default
   - Show cost comparison if arbitrage is enabled

8. **New Ledger tab**: Show recent session history and statistics.
   - Use `pkg/ledger.Ledger.RecentSessions(10)` for last 10 entries
   - Use `pkg/ledger.Ledger.Stats()` for aggregate stats
   - Display as a table: Date | Model | Provider | Tasks | Duration | Goal
   - Press `Enter` on an entry to see details

### Key bindings to add

- `e` — edit focused field (in tabs with editable fields)
- `space` — toggle boolean flag (Features tab)
- `a` — add new item (Permissions rules)
- `d` — delete focused item (Permissions rules)
- `j/k` or `up/down` — navigate within tab content

### Implementation requirements

1. The edit input should appear inline, replacing the current value display.
2. On save (`Enter` at top level), all pending edits should be applied and config written to disk.
3. Invalid input should show an error message without applying the change.
4. The Model and Ledger tabs should show loading state while fetching data.
5. Use existing lipgloss styling for consistency.
6. The `dirty` flag should be set when any field is modified.
7. Config changes should be applied in `Update()` and saved in `Save()`.

### Tests to update/add

- `TestSettingsTabs` — verify 6 tabs exist and cycle correctly
- `TestSettings_EditGeneral` — edit MaxIterations, verify config updated
- `TestSettings_EditProvider` — edit API key, verify keychain used
- `TestSettings_ToggleFeature` — toggle AutodreamEnabled, verify config
- `TestSettings_AddPermissionRule` — add rule, verify config
- `TestSettings_DeletePermissionRule` — delete rule, verify config
- `TestSettings_ModelTab` — model tab loads and displays models
- `TestSettings_SelectModel` — select model as default
- `TestSettings_LedgerTab` — ledger tab shows recent sessions
- `TestSettings_SaveWithEdits` — save applies all pending edits

---

## Gap Fix — FallbackEvent TUI Emission

**File:** Update `internal/tui/app.go` + `internal/tui/types.go`

**Prompt:**

Add TUI support for provider fallback events. When the active provider fails and the system falls back to an alternative, the TUI should display a notification.

### Context

- `internal/provider/fallback.go` has `FallbackEvent` struct:
```go
type FallbackEvent struct {
    OriginalProvider string
    FallbackProvider string
    Reason           string
    Timestamp        time.Time
}
```
- `FindFallbackProvider(registry, exclude string) *FallbackEvent` returns a fallback event when a fallback provider is found
- `AppState` in `internal/tui/app.go` handles `AppMsg` with `Health *HealthUpdateMsg` and `Provider *ProviderSwitchMsg`

### Changes required

1. **In `internal/tui/types.go`**: Add `Fallback *FallbackEvent` field to `AppMsg`.

2. **In `internal/tui/app.go`**: Add a case in `Update()` for handling `AppMsg.Fallback`:
   - Show a notification banner: "Provider failed: {reason}. Falling back to {fallback}."
   - Update `m.activeProvider` to the fallback provider
   - Set `m.registry` to use the fallback provider

3. **Add notification state to AppState**:
```go
fallbackNotification *FallbackNotification
```
```go
type FallbackNotification struct {
    Event     provider.FallbackEvent
    Dismissed bool
    ShownAt   time.Time
}
```

4. **Render the notification** in `View()` — show at the top of the screen when present, with dismiss key (`x`).

5. **Key binding**: `x` dismisses the fallback notification.

### Tests

- `TestFallbackNotification` — notification displayed when fallback event received
- `TestFallbackNotification_Dismiss` — notification dismissed with x key
- `TestFallbackNotification_ProviderSwitch` — active provider updated

---

## Gap Fix — Thinking Block T Key Binding

**File:** Update `internal/tui/app.go` or the REPL model

**Prompt:**

Add a `T` key binding to toggle thinking block visibility in the REPL.

### Context

- `internal/tui/components/thinking.go` has `ThinkingBlock` component with `Toggle()`, `SetVisible(bool)`, `Visible() bool`, `View() string` methods
- The thinking block is displayed in the REPL when the model returns thinking tokens
- Currently there is no key binding to toggle visibility

### Changes required

1. In the REPL model's `Update()`, add a case for `t` or `T` key that calls `m.thinkingBlock.Toggle()`.
2. Update the footer help text to include "T: toggle thinking".
3. Persist the visibility state so it's remembered across messages.

### Tests

- `TestThinkingBlockToggle` — T key toggles visibility
- `TestThinkingBlock_Visible` — thinking shown when visible
- `TestThinkingBlock_Hidden` — thinking hidden when toggled off

---

## Gap Fix — Background Model Cache Refresh

**File:** Update `internal/provider/cache.go` or `internal/tui/health.go`

**Prompt:**

Add a background ticker that automatically refreshes the model cache when it expires.

### Context

- `internal/provider/cache.go` has `ModelCache` with `ttl` (normal expiry) and `staleTTL` (hard expiry)
- `ModelCache.IsExpired()` returns true when `time.Since(m.fetched) > m.ttl`
- `ModelCache.IsStale()` returns true when `time.Since(m.fetched) > m.staleTTL`
- `internal/tui/health.go` has `HealthCheckTicker` pattern using `time.Ticker`

### Changes required

1. Add `ModelCacheRefreshTicker` struct similar to `HealthCheckTicker`:
```go
type ModelCacheRefreshTicker struct {
    ticker  *time.Ticker
    cache   *ModelCache
    provider LLMProvider
    ctx     context.Context
}
```

2. Methods: `NewModelCacheRefreshTicker(cache, provider, interval)`, `Start() tea.Cmd`, `Stop()`, `OnTick() tea.Msg`.

3. Define `ModelCacheRefreshMsg` type.

4. In `AppState.Update()`, handle `ModelCacheRefreshMsg`: call `provider.FetchModels()`, update cache, emit notification if models changed.

5. Start the ticker in `AppState.Init()` alongside `HealthCheckTicker`.

6. Interval: use `types.ModelCacheTTL` (5 minutes) for normal refresh.

### Tests

- `TestModelCacheRefresh` — cache refreshed after TTL
- `TestModelCacheRefresh_SkippedIfNotExpired` — no refresh if cache still valid
- `TestModelCacheRefresh_FetchError` — error handled gracefully

---

## How to Use These Prompts

1. **Independent prompts** (can be done in parallel): P7.6 (commands), P7.4 (rollback), P7.2 (arbitrage), P7.5 (autodream), P7.3 (ledger), Gap fixes
2. **Dependent prompts** (require previous work):
   - P7.1 (model selector) depends on P7.2 (arbitrage) for cost display
   - P7.7 (settings update) depends on P7.1 (model selector) and P7.3 (ledger) for the new tabs

### Recommended implementation order:

1. P7.6 (commands) + P7.4 (rollback) + P7.2 (arbitrage) — parallel, no dependencies
2. P7.5 (autodream) + P7.3 (ledger) — parallel, depend on commands
3. Gap fixes (fallback, thinking toggle, cache refresh) — parallel
4. P7.1 (model selector) — depends on arbitrage
5. P7.7 (settings update) — depends on model selector + ledger

### After all implementations:

1. Run `go test -race -cover ./...` — all tests should pass
2. Run `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` — build should succeed
3. Run `go vet ./...` — no issues
4. Wire new screens into `app.go` Update/View switch statements
5. Add Screen constants to `internal/tui/types.go` for any new screens
