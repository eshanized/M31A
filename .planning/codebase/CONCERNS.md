# Concerns & Technical Debt — M31A

> Mapped: 2026-07-09

## Known Bugs (Documented)

Several bugs are documented with BUG-NN markers in source. These are known edge cases:

| ID | Location | Issue |
|----|----------|-------|
| BUG-01 | internal/git/git.go:451 | Channel ordering issue in git operations |
| BUG-04 | internal/workflow/classify_test.go:12 | Multiple framework indicators confuse classification |
| BUG-05 | internal/workflow/classify_test.go:42 | Multiple lock files need tiebreaker logic |
| BUG-06 | internal/tools/concurrency.go:48 | sync.Map type assertions; short verb-led goals |
| BUG-07 | internal/tools/concurrency.go:48 | sync.Map type assertions |
| BUG-10 | internal/workflow/ship.go:118 | Working tree clean after AddAll edge case |
| BUG-12 | internal/workflow/engine.go:725 | Phase oscillation prevention (one-cycle limit) |
| BUG-15 | internal/workflow/ship.go:347 | Heuristic vs numstat-based classification |
| BUG-17 | internal/provider/cache.go:47 | Cache concurrency edge case |
| BUG-18 | internal/config/loader.go:989 | Silent message loss |
| BUG-19 | internal/tools/concurrency.go:48 | sync.Map type assertions |
| BUG-29 | internal/tokens/estimator.go:320 | Token estimation may over-allow past context window |

## Code Size

- ~134K lines of Go across cmd/, internal/, pkg/
- Largest: internal/tui/ (158 files), internal/workflow/ (91 files), internal/tools/ (79 files)
- Onboarding overhead is non-trivial

## Coverage Gaps

- Targets: 75% overall, 90% for pkg/taskrunner, pkg/bisect, pkg/rollback
- TUI layer (internal/tui/) has limited test coverage due to screen interaction complexity
- No e2e tests for the TUI (only binary-level e2e tests exist)

## Dependency Concerns

- github.com/golang-jwt/jwt/v5 is commented out in go.mod (future JWT support?)
- github.com/godbus/dbus/v5 ties Linux keychain to D-Bus (fails in minimal containers)
- No indirect dependency audit tooling beyond govulncheck

## Area-Specific Concerns

### TUI (internal/tui/)
- 33 screens through single routing system -- complexity grows linearly
- Custom layout solver in layout/ (14 files) adds maintenance burden
- Sidebar model (sidebar_model.go) is large and handles many concerns
- Streaming performance requires 10fps render rate with debounce

### Tools (internal/tools/)
- Bash tool has significant platform-specific code (unix + windows variants)
- Edit tool uses 7-strategy cascade -- complex behavior hard to trace
- Permission system has persistent state management complexity
- 79 files is large for a single package; may need splitting

### Provider Layer
- Three provider implementations with similar boilerplate
- SSE parsing is custom (no standard library) -- edge cases with streaming
- Model capability detection is dynamic but may not cover all models
- Cache invalidation strategy is TTL-only (no event-driven refresh)

### Engine (internal/workflow/)
- 91 files with complex phase interactions
- State machine can have edge cases in transition boundaries
- Self-heal retry (2 retries) may not recover from all failure modes
- Plan chunking adds complexity for edge cases (very small/large plans)

### Config (internal/config/)
- TOML loading + project context detection + merging is layered complexity
- Hot-reload via fsnotify adds race condition surface
- Some config overrides may not be wired to all consumers

### Code Intelligence (internal/codeintel/)
- Only 4 languages (Go, TypeScript, Python, Rust) -- not extensible without parser work
- Tree-sitter dependency (gotreesitter) may have version compatibility issues
- Relevance scoring is heuristic-based; may not work well for all codebase shapes

## Security Observations

- Strong SSRF/DNS rebinding protection on webfetch/websearch
- Permission system is modal-based with configurable defaults
- Keychain abstraction is solid (3 platforms)
- Command blocklist is defense-in-depth but could be bypassed with creative syntax
- No subprocess sandboxing (no container, no seccomp) -- Bash tool runs with user privileges
- OS keychain is D-Bus dependent on Linux (no KDE wallet / GNOME keyring direct support)

## Performance Concerns

- TUI rendering at 10fps may flicker on slow terminals
- SSE streaming parsing can be a bottleneck for fast models
- Code intelligence parsing may be slow for large codebases (>100K files)
- Token estimation via tiktoken-go may have accuracy issues with non-standard models
- Session persistence (JSON files) may not scale to hundreds of sessions
