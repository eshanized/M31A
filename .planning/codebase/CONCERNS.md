# Codebase Concerns

**Analysis Date:** 2026-06-12
**Last Updated:** 2026-06-12 (resolution audit)

## Resolved Issues

The following issues were identified in the original audit and have since been resolved:

### Tech Debt (Resolved)

- ~~**Config Merge Logic Bug**~~ — Fixed: `loader.go` now uses `meta.Keys()` to build the `defined` set (line 154).
- ~~**Token Estimator Data Race**~~ — Fixed: `emaFactor` protected by `sync.Mutex` in both `Estimate()` and `Calibrate()`.
- ~~**Provider Client Code Duplication**~~ — Fixed: `BaseClient` extracted to `internal/provider/base_client.go`; OpenRouter and Zen embed it and override only provider-specific methods.
- ~~**Workflow Engine Parameters**~~ — Fixed: `EngineOptions` struct added alongside `NewEngineFromOptions()`.

### Known Bugs (Resolved)

- ~~**Config Merge Inverts Key Tracking**~~ — Fixed: uses `meta.Keys()` correctly.
- ~~**Bash Security Blacklist Trivially Bypassable**~~ — Fixed: blacklist removed entirely; permission system (risk levels, ask/allow/deny rules) is the security control.
- ~~**Zen Client Missing Tools Serialization**~~ — Fixed: `provider.BuildChatBody()` in `common.go` serializes tools; both clients use it.
- ~~**Dispatcher Unknown Tool Check Order**~~ — Fixed: tool lookup (line 140-147) runs before JSON parsing (line 156).

### Security (Resolved)

- ~~**API Key Exposure**~~ — Fixed: `BaseClient.APIKey()` returns masked version (`****xxxx`).
- ~~**Bash Blacklist False Security**~~ — Fixed: blacklist removed.
- ~~**No Rate Limiting on Tool Execution**~~ — Fixed: token bucket rate limiter (10/sec, burst 5) in `Dispatcher`.
- ~~**WebFetch SSRF Protection Incomplete**~~ — Fixed: `isPrivateIP()` uses `net.IP.IsPrivate()` (Go 1.17+) plus loopback, link-local, unspecified, and cloud metadata checks.

### Performance (Resolved)

- ~~**HTML Processing O(n*m)**~~ — Fixed: `replaceInlineTag` and `convertLinks` now maintain a parallel lowercased copy instead of recomputing `strings.ToLower` on the full HTML each iteration.
- ~~**Config File Polling**~~ — Fixed: `WatchConfig` uses `fsnotify` for event-driven file watching with debouncing; falls back to polling if fsnotify is unavailable.
- ~~**Session Manager Cache TOCTOU**~~ — Fixed: `refreshMu` with double-checked locking serializes cache refreshes.
- ~~**SkipDirsMap Allocation**~~ — Fixed: computed once via `sync.Once`; returns a copy for safety.

### Fragile Areas (Resolved)

- ~~**Workflow Engine Bubble Tea Leak**~~ — Fixed: `MsgEmitter` interface uses `any` type; engine imports only domain types, not bubbletea.
- ~~**Provider Registry Health Check Gap**~~ — Fixed: `FindFallbackProvider` calls `RollbackActive()` if health check fails after `TrySetActive`.
- ~~**Session Schema No Versioning**~~ — Fixed: `SchemaVersion int` field with `CurrentSchemaVersion = 1` in `session.go`.

### Scaling Limits (Resolved)

- ~~**Workflow Engine Parameters**~~ — Fixed: `EngineOptions` struct available via `NewEngineFromOptions()`.

---

## Remaining Concerns

### Tech Debt

**Workflow Engine Too Many Responsibilities:**
- Issue: Engine struct manages session tracking, provider management, git operations, tool dispatch, token estimation, streaming, self-healing, checkpoint management, and budget tracking.
- Files: `internal/workflow/engine.go`, `internal/workflow/execute.go`
- Impact: Difficult to test, maintain, or extend individual concerns
- Mitigating factor: `EngineOptions` struct and `MsgEmitter` interface reduce coupling
- Remaining work: Extract `PromptManager`, `LLMClient`, `HealManager`, `BudgetTracker` into separate types

**TUI AppState God Object:**
- Issue: `internal/tui/app_state.go` carries 50+ fields. `Update()` is a large switch with many message type cases.
- Files: `internal/tui/app_state.go`, `internal/tui/app_update.go`
- Impact: Adding new screen requires touching multiple switch statements
- Fix approach: Introduce `ScreenManager` for sub-model lifecycle, extract `PermissionController` and `WorkflowController`

**Session Manager Monolith:**
- Issue: `pkg/session/manager.go` (842 lines) handles many concerns
- Files: `pkg/session/manager.go`
- Impact: Single responsibility violation; difficult to test or extend
- Mitigating factor: `ManagerOpts` struct, `refreshMu` for cache concurrency
- Fix approach: Split into `SessionRepository`, `SessionCache`, `SessionExporter`, `ModelHistory`

### Security Considerations

**Environment Variable Injection:**
- Risk: `LoadDotEnv()` reads `.env` from current working directory and sets environment variables
- Files: `internal/config/loader.go`
- Current mitigation: Does not override existing vars; skips world-writable `.env` files; caps line length at 4096
- Recommendations: Only load `.env` from trusted paths or require user confirmation

### Performance Bottlenecks

**Config Merge Reflection:**
- Problem: Config merging uses `reflect.ValueOf` with hand-written `toTOMLKey()` for PascalCase to snake_case conversion
- Files: `internal/config/loader.go`
- Cause: Reflection-based approach is fragile and non-type-safe
- Improvement path: Use typed merge approach or leverage TOML library's `MetaData`

### Fragile Areas

**TUI Screen Lifecycle Management:**
- Files: `internal/tui/app_update.go`
- Why fragile: Adding new screen requires multiple manual steps across switch statements
- Safe modification: Define `ScreenModel` interface with `Init/Update/View/SetDimensions/SetTheme` methods; use `map[Screen]ScreenModel` registry
- Test coverage: 0% for entire TUI package

**Dispatcher Permission Logic Mixing:**
- Files: `internal/tools/dispatcher.go`
- Why fragile: `Execute()` handles rate limiting, tool lookup, JSON parsing, input normalization, permission checking, and tool execution
- Safe modification: Extract `PermissionEvaluator`, make `Execute()` thin orchestrator
- Test coverage: ~43% for tools package

### Dependencies at Risk

**tiktoken-go:**
- Risk: Unmaintained since 2024; new tokenizers may not be recognized
- Impact: Silent fallback to rune counting for token estimation
- Migration plan: Monitor for maintained forks; consider alternative tokenizer

**BurntSushi/toml v1:**
- Risk: In maintenance mode; v2 has different API
- Impact: Configuration parsing may miss improvements or fixes
- Migration plan: Plan migration to v2 or alternative config format

**Bubble Tea Framework:**
- Risk: TUI tightly coupled to Bubble Tea; workflow engine now decoupled via `MsgEmitter`
- Impact: TUI changes require Bubble Tea knowledge
- Migration plan: Define `ScreenModel` interface for screen lifecycle

### Missing Critical Features

**Integration Tests for Full Workflow:**
- Problem: No tests exercise Initialize → Discuss → Plan → Execute → Verify → Ship pipeline
- Blocks: Cannot catch cross-package regressions

**Fuzzing Tests:**
- Problem: No fuzzing tests despite parsing untrusted input (LLM responses, user config, JSON tool calls)
- Blocks: JSON parsing edge cases, SSE stream malformed input, config file parsing bugs

**Race Detector in CI:**
- Problem: No evidence CI runs with `-race` flag despite concurrent code
- Blocks: Data races go undetected

**API Documentation:**
- Problem: Public packages lack comprehensive documentation
- Blocks: Developer adoption and contribution

### Test Coverage Gaps

**TUI Package (0% coverage):**
- What's not tested: All TUI files
- Files: `internal/tui/` (entire directory)
- Risk: Largest package untested
- Priority: Critical

**Keychain Package (low coverage):**
- What's not tested: `Get/Set/Delete` with D-Bus and `pass` CLI backends
- Files: `pkg/keychain/`
- Risk: Secret storage mechanism untested; platform-specific code unverified
- Priority: High

**Git Package (~31% coverage):**
- What's not tested: `Run`, `CommitStaged`, `Log`, `Diff`, `StatusPorcelain`, `RevParse`
- Files: `internal/git/git.go`
- Priority: High

**Tools Package (~43% coverage):**
- What's not tested: `Edit`, `WebFetch`, `FileDelete`, `FileList`, `FileMove`, `Todo`, `Question`
- Files: `internal/tools/`
- Priority: High

**Workflow Package (~65% coverage):**
- What's not tested: `HealTask`, `DiscussState`, `SetRefinementFeedback`, plan parsing
- Files: `internal/workflow/`
- Priority: Medium

**Session Package (~65% coverage):**
- What's not tested: Recent models, plans, demos, rename, export, filter operations
- Files: `pkg/session/`
- Priority: Medium

---

*Concerns audit: 2026-06-12*
*Resolution audit: 2026-06-12*
