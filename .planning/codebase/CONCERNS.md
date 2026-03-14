# Codebase Concerns

**Analysis Date:** 2026-06-12

## Tech Debt

**Config Merge Logic Bug:**
- Issue: `internal/config/loader.go:145-147` uses `meta.Undecoded()` which returns keys NOT decoded into struct fields, the opposite of "explicitly defined keys". This means bool config fields set to `false` in project config are silently ignored when default is `true`.
- Files: `internal/config/loader.go`
- Impact: Project-level config overrides for bool fields (e.g., `compact_mode`, `auto_fallback`) fail silently
- Fix approach: Use `meta.Keys()` instead of `meta.Undecoded()` to track decoded keys

**Token Estimator Data Race:**
- Issue: `internal/tokens/estimator.go:66-101` - `emaFactor` field is read in `Estimate()` and written in `Calibrate()` without synchronization. Concurrent calls create a data race.
- Files: `internal/tokens/estimator.go`
- Impact: Concurrent token estimation and calibration causes data corruption
- Fix approach: Add `sync.Mutex` or `sync/atomic` for `emaFactor` field

**Provider Client Code Duplication:**
- Issue: OpenRouter and Zen clients share ~80% identical code across ~330 lines each. Methods like `New()`, `FetchModels()`, `ChatCompletionStream()`, `HealthCheck()` are structurally identical.
- Files: `internal/provider/openrouter/client.go`, `internal/provider/zen/client.go`
- Impact: Bug fixes must be applied twice; new features require duplicate implementation
- Fix approach: Create `baseClient` struct with shared methods, providers embed and override only provider-specific logic

**Workflow Engine Too Many Responsibilities:**
- Issue: `internal/workflow/engine.go:63-93` - Engine struct manages 18+ concerns including session tracking, provider management, git operations, tool dispatch, token estimation, streaming, self-healing, checkpoint management, and budget tracking. `NewEngine()` takes 10 parameters.
- Files: `internal/workflow/engine.go`, `internal/workflow/execute.go`
- Impact: Difficult to test, maintain, or extend individual concerns
- Fix approach: Extract `PromptManager`, `LLMClient`, `HealManager`, `BudgetTracker`; use `EngineOptions` struct

**TUI AppState God Object:**
- Issue: `internal/tui/app_state.go:49-180` carries 50+ fields including 26 sub-model pointers. `Update()` is a 650-line switch with 40+ message type cases.
- Files: `internal/tui/app_state.go`, `internal/tui/app_update.go`
- Impact: Adding new screen requires touching 5+ switch statements; extreme brittleness
- Fix approach: Introduce `ScreenManager` for sub-model lifecycle, extract `PermissionController` and `WorkflowController`

**Session Manager Monolith:**
- Issue: `pkg/session/manager.go` (842 lines) handles session CRUD, listing, archiving, forking, model management, cleanup, export, filtering, checkpoints, tasks, plans, and project state.
- Files: `pkg/session/manager.go`
- Impact: Single responsibility violation; difficult to test or extend
- Fix approach: Split into `SessionRepository`, `SessionCache`, `SessionExporter`, `ModelHistory`

## Known Bugs

**Config Merge Inverts Key Tracking:**
- Symptoms: Bool config fields set to `false` in `m31a.toml` are silently ignored when default is `true`
- Files: `internal/config/loader.go:145-147`
- Trigger: User sets `compact_mode = false` in project config while default is `true`
- Workaround: None - requires code fix

**Bash Security Blacklist Trivially Bypassable:**
- Symptoms: Simple substring matching allows bypass via flag splitting, variable expansion, or command substitution
- Files: `internal/tools/bash.go:29-59`
- Trigger: Commands like `rm -r -f /` or `bash <(curl url)` pass blacklist
- Workaround: Permission system is actual security control, not blacklist

**Zen Client Missing Tools Serialization:**
- Symptoms: `TestChatCompletionStream_WithTools` fails - tools not included in request body
- Files: `internal/provider/zen/client.go:155+`
- Trigger: Using Zen gateway with tool-enabled models
- Workaround: Use OpenRouter provider for tool use

**Dispatcher Unknown Tool Check Order:**
- Symptoms: `TestDispatcher_UnknownTool` fails - JSON unmarshal runs before tool lookup
- Files: `internal/tools/dispatcher.go:132-138`
- Trigger: LLM generates tool call with no parameters for unknown tool
- Workaround: None - error message hides real problem

## Security Considerations

**API Key Exposure:**
- Risk: Public `APIKey()` method in both provider clients returns raw API key string. Logging with `%+v` formatting exposes keys in log files.
- Files: `internal/provider/openrouter/client.go:94-96`, `internal/provider/zen/client.go:88-90`
- Current mitigation: None
- Recommendations: Remove `APIKey()` or return masked version (e.g., `sk-...xxxx`)

**Environment Variable Injection:**
- Risk: `loadDotEnv()` reads `.env` from current working directory and sets environment variables. Malicious `.env` could inject `M31A_OPENROUTER_API_KEY` or other vars.
- Files: `internal/config/loader.go:673-705`
- Current mitigation: Does not override existing vars
- Recommendations: Only load `.env` from trusted paths or require user confirmation

**Bash Blacklist False Security:**
- Risk: Substring matching provides minimal real protection but creates false sense of security
- Files: `internal/tools/bash.go:29-59`
- Current mitigation: Permission system (risk levels, ask/allow/deny rules)
- Recommendations: Remove blacklist or document it as minimal guard, not security boundary

**No Rate Limiting on Tool Execution:**
- Risk: Malicious or buggy LLM could generate thousands of tool calls per second
- Files: `internal/tools/dispatcher.go`
- Current mitigation: Token bucket rate limiter exists (10/sec with burst of 5)
- Recommendations: Verify rate limiter is effective; add monitoring

**WebFetch SSRF Protection Incomplete:**
- Risk: Private IPv4 ranges blocked but IPv6 link-local and IPv4-mapped IPv6 may bypass checks
- Files: `internal/tools/webfetch.go:80-86`
- Current mitigation: `IsLinkLocalUnicast()` check covers 169.254.0.0/16
- Recommendations: Use `net.IP.IsPrivate()` (Go 1.17+) for comprehensive checks

## Performance Bottlenecks

**Config Merge Reflection:**
- Problem: `internal/config/loader.go:196-287` uses `reflect.ValueOf` for config merging with hand-written `toTOMLKey()` for PascalCase → snake_case conversion
- Files: `internal/config/loader.go`
- Cause: Reflection-based approach is fragile and non-type-safe
- Improvement path: Use typed merge approach or leverage TOML library's `MetaData`

**HTML Processing O(n*m):**
- Problem: `internal/tools/webfetch.go:488-516` - `replaceBlockTag` re-lowercases entire HTML on each iteration
- Files: `internal/tools/webfetch.go`
- Cause: Each tag replacement re-computes `strings.ToLower` on entire HTML body (up to 5MB)
- Improvement path: Track offsets in lowercased copy separately or use single-pass approach

**Config File Polling:**
- Problem: `internal/config/loader.go:642-671` uses 5-second polling via `os.Stat` instead of file system events
- Files: `internal/config/loader.go`
- Cause:浪费 I/O and up to 5-second latency
- Improvement path: Use `fsnotify` for event-driven file watching

**Session Manager Cache TOCTOU:**
- Problem: `pkg/session/manager.go:310-317` - Multiple goroutines can simultaneously miss cache and perform filesystem walk
- Files: `pkg/session/manager.go`
- Cause: Redundant I/O on cache miss
- Improvement path: Use `singleflight.Group` to deduplicate concurrent walks

**SkipDirsMap Allocation:**
- Problem: `internal/types/constants.go:104-110` creates new map allocation on every call
- Files: `internal/types/constants.go`
- Cause: `SkipDirs` is immutable but map is recreated each time
- Improvement path: Compute once at init time and cache

## Fragile Areas

**TUI Screen Lifecycle Management:**
- Files: `internal/tui/app_update.go:1146-1281`, `app_update.go:673-800`
- Why fragile: Adding new screen requires 8 manual steps across multiple switch statements. Missing any causes silent bugs (nil pointer, unresponsive screen, wrong size).
- Safe modification: Define `ScreenModel` interface with `Init/Update/View/SetDimensions/SetTheme` methods; use `map[Screen]ScreenModel` registry
- Test coverage: 0% for entire TUI package

**Workflow Engine Bubble Tea Leak:**
- Files: `internal/workflow/engine.go:23`, `internal/workflow/engine_messages.go:8`
- Why fragile: Business logic layer directly imports `bubbletea` and uses `tea.Msg`. Cannot be tested or used outside Bubble Tea context.
- Safe modification: Define domain-specific `Event` interface; TUI layer adapts to `tea.Msg`
- Test coverage: 64.5% for workflow package

**Dispatcher Permission Logic Mixing:**
- Files: `internal/tools/dispatcher.go:121-231`
- Why fragile: `Execute()` handles rate limiting, tool lookup, JSON parsing, input normalization, permission checking (5 branching paths), interactive/non-interactive mode detection, and tool execution in 110 lines
- Safe modification: Extract `PermissionEvaluator`, `RateLimiter`, make `Execute()` thin orchestrator
- Test coverage: 43.1% for tools package

**Provider Registry Health Check Gap:**
- Files: `internal/provider/fallback.go:30-40`
- Why fragile: `TrySetActive` atomically sets provider active, health check runs after switch. If health check fails, provider remains active but unhealthy.
- Safe modification: Check health before `TrySetActive` or add rollback mechanism
- Test coverage: 53.8% for provider package

**Session Schema No Versioning:**
- Files: `pkg/session/session.go`, `pkg/session/manager.go`
- Why fragile: Sessions serialized via `json.Marshal(session)` with no version field. Adding/removing fields breaks backward compatibility with no migration path.
- Safe modification: Add `SchemaVersion int` field; create `migrateSession(data []byte, version int)` function
- Test coverage: 64.7% for session package

## Scaling Limits

**TUI Package Size:**
- Current capacity: 20,684 LOC across 84 files with 839 functions
- Limit: Weighted statement coverage stuck at 26.6% due to zero test coverage
- Scaling path: Introduce TUI test harness; focus on pure render/state functions first

**Workflow Engine Parameters:**
- Current capacity: 10 positional parameters in `NewEngine()`
- Limit: Adding new parameters requires changing all call sites
- Scaling path: Use `EngineOptions` struct with functional options pattern

**Config Merge Complexity:**
- Current capacity: Reflection-based merge with 40+ UI config fields
- Limit: New field types require adding switch cases; `toTOMLKey()` doesn't handle edge cases
- Scaling path: Use typed merge or library like `github.com/imdario/mergo`

## Dependencies at Risk

**tiktoken-go:**
- Risk: Unmaintained since 2024; new tokenizers (e.g., o200k_base for GPT-4o) may not be recognized
- Impact: Silent fallback to rune counting for token estimation
- Migration plan: Monitor for maintained forks; consider alternative tokenizer

**BurntSushi/toml v1:**
- Risk: In maintenance mode; v2 has different API
- Impact: Configuration parsing may miss improvements or fixes
- Migration plan: Plan migration to v2 or alternative config format

**Bubble Tea Framework:**
- Risk: Tightly coupled; workflow engine imports directly
- Impact: Cannot test workflow without UI framework; framework changes affect business logic
- Migration plan: Define domain events; use adapter pattern

## Missing Critical Features

**Integration Tests for Full Workflow:**
- Problem: No tests exercise Initialize → Discuss → Plan → Execute → Verify → Ship pipeline
- Blocks: Cannot catch cross-package regressions; confidence in releases

**Fuzzing Tests:**
- Problem: No fuzzing tests despite parsing untrusted input (LLM responses, user config, JSON tool calls)
- Blocks: JSON parsing edge cases, SSE stream malformed input, config file parsing bugs

**Race Detector in CI:**
- Problem: No evidence CI runs with `-race` flag despite concurrent code (SSE parsing, streaming, permission channels)
- Blocks: Data races go undetected

**API Documentation:**
- Problem: Public packages (`pkg/arbitrage`, `pkg/autodream`, `pkg/bisect`, `pkg/ledger`, `pkg/rollback`, `pkg/session`, `pkg/taskrunner`) have no comprehensive documentation
- Blocks: Developer adoption and contribution

## Test Coverage Gaps

**TUI Package (0% coverage):**
- What's not tested: 20,684 LOC across 84 files with 839 functions
- Files: `internal/tui/` (entire directory)
- Risk: Largest package untested; weighted coverage stuck at 26.6%
- Priority: Critical - blocks coverage improvement

**Keychain Package (2.1% coverage):**
- What's not tested: `Get/Set/Delete` with D-Bus and `pass` CLI backends
- Files: `pkg/keychain/keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go`
- Risk: Secret storage mechanism untested; platform-specific code unverified
- Priority: High - security-critical functionality

**Git Package (30.8% coverage):**
- What's not tested: `Run`, `CommitStaged`, `Log`, `Diff`, `StatusPorcelain`, `RevParse`, etc.
- Files: `internal/git/git.go`
- Risk: Version control operations untested
- Priority: High - core workflow dependency

**Tools Package (43.1% coverage):**
- What's not tested: `Edit`, `WebFetch`, `FileDelete`, `FileList`, `FileMove`, `Todo`, `Question`
- Files: `internal/tools/`
- Risk: Tool execution untested; security controls unverified
- Priority: High - core functionality

**Workflow Package (64.5% coverage):**
- What's not tested: `HealTask`, `DiscussState`, `SetRefinementFeedback`, plan parsing functions
- Files: `internal/workflow/`
- Risk: Business logic untested; state management unverified
- Priority: Medium - core workflow logic

**Session Package (64.7% coverage):**
- What's not tested: Recent models, plans, demos, rename, export, filter operations
- Files: `pkg/session/`
- Risk: Data persistence untested
- Priority: Medium - data integrity risk

---

*Concerns audit: 2026-06-12*
