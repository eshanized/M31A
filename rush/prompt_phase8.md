# Phase 8 — Polish, Testing & v1.0 Release

## Context

M31A V1 implementation is ~95% complete. All core subsystems are implemented and tested:
- Phase 0-7: Provider layer, TUI (10 screens), rendering pipeline, tool system, session state, 6-phase workflow engine, signature features (arbitrage, ledger, rollback, autodream, model selector, settings, slash commands)
- 50 test files across ~22 packages
- CI pipeline configured (lint, test, build matrix, GoReleaser)
- Binary builds statically with `CGO_ENABLED=0`

The remaining work is **Phase 8**: polish, documentation, release pipeline, and final verification. This is primarily non-code work: error handling audit, test coverage verification, documentation writing, and release infrastructure hardening.

## Roadmap Reference

See `adrenaline/ROADMAP.md` Phase 8 (lines 594-637) for the full specification. This prompt implements all P8 tasks.

---

## Task P8.1 — Error Handling Hardening

### Current State
- 14 sentinel errors defined in `internal/errors/errors.go`
- Error types exist: `ErrProviderUnreachable`, `ErrRateLimited`, `ErrInvalidKey`, etc.
- TUI displays errors in some paths but needs audit

### What to Do

1. **Audit every error return path** across all packages. Ensure errors are:
   - Wrapped with context using `fmt.Errorf("operation: %w", err)`
   - Never raw stack traces shown to user
   - Displayed via styled TUI error messages (use `theme.Error` style)

2. **Graceful shutdown on Ctrl+C**:
   - Read `internal/tui/app.go` — verify `tea.WithCatch` or equivalent is used
   - On `tea.InterruptMsg` or signal capture: save current session state (call `sessionMgr.SaveSession()`)
   - Incomplete tool calls should emit `[INTERRUPTED]` status in tool card
   - Read `internal/tools/dispatcher.go` — verify running processes get `SIGINT` forwarded

3. **Offline mode**:
   - If both providers unreachable (health check fails for both), load REPL with history viewer but disable streaming
   - Show banner: "No providers available — offline mode. History is readable but no new messages."
   - User can still use `/sessions`, `/ledger`, `/rollback`, `/status` commands

4. **Verify error paths in workflow engine**:
   - Read `internal/workflow/engine.go` — check all `RunPhase` error returns
   - Each phase should return a structured error that TUI can display
   - Failed phase transitions should not lose session state

### Files to Read First
- `internal/errors/errors.go`
- `internal/tui/app.go` — look for `tea.InterruptMsg` handling
- `internal/tui/repl.go` — error display in streaming
- `internal/tools/dispatcher.go` — error handling in tool execution
- `internal/workflow/engine.go` — error returns from RunPhase
- `internal/provider/fallback.go` — fallback error handling

---

## Task P8.2 — Test Coverage Pass

### Current State
- 50 test files exist
- Coverage target: 75% overall, 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

### What to Do

1. **Run current coverage report**:
   ```bash
   go test -race -cover -coverprofile=coverage.out ./...
   go tool cover -func=coverage.out | grep total
   go tool cover -html=coverage.out -o coverage.html
   ```
   Report the overall coverage percentage and per-package breakdown.

2. **Identify gaps**: Find packages below 75% coverage and below 90% for the three critical packages. Write targeted tests for:
   - Uncovered error paths
   - Edge cases (empty inputs, nil values, boundary conditions)
   - Integration scenarios (session save/load roundtrip, checkpoint restore)

3. **Race detector**: `go test -race ./...` must pass cleanly on all packages. Fix any data races found.

4. **Integration test suite**: The roadmap calls for:
   - Full workflow against mocked provider (Initialize through Ship)
   - Corrupt session recovery test
   - Keychain fallback test
   - Auto-fallback triggering test

   Read `internal/workflow/engine_test.go` — it already has 18+ test functions. Determine if additional integration tests are needed beyond what exists.

5. **Test cleanup**: Remove any dead or skipped tests. Ensure all tests use table-driven patterns where appropriate.

### Verification Commands
```bash
cd /home/snigdha/Desktop/Helix/M31A

go test -race -cover -coverprofile=coverage.out ./...
go tool cover -func=coverage.out | tail -5
go test -race -count=3 ./...  # flaky test detection
```

---

## Task P8.3 — Cross-Platform Verification

### Current State
- CI matrix exists in `.github/workflows/ci.yml`: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
- `CGO_ENABLED=0` enforced in all build steps
- Keychain has platform-specific files: `keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go` (stub)

### What to Do

1. **Windows keychain stub**: `pkg/keychain/keychain_windows.go` returns `ErrNotImplemented`.
   - Add `go-wincred` dependency to `go.mod`
   - Implement `Get()`, `Set()`, `Delete()` using `wincred` package
   - Follow same pattern as Linux/macOS implementations
   - Add Windows-specific tests

2. **Build verification on all platforms**:
   - Trigger CI manually or verify locally that cross-compile works:
   ```bash
   GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build -o m31a-linux-amd64 ./cmd/m31a
   GOOS=linux GOARCH=arm64 CGO_ENABLED=0 go build -o m31a-linux-arm64 ./cmd/m31a
   GOOS=darwin GOARCH=amd64 CGO_ENABLED=0 go build -o m31a-darwin-amd64 ./cmd/m31a
   GOOS=darwin GOARCH=arm64 CGO_ENABLED=0 go build -o m31a-darwin-arm64 ./cmd/m31a
   GOOS=windows GOARCH=amd64 CGO_ENABLED=0 go build -o m31a-windows-amd64.exe ./cmd/m31a
   ```
   All five builds must succeed.

3. **Platform-specific behavior audit**:
   - Bash tool: PTY on Linux/macOS, pipes on Windows — verify fallback path works
   - Keychain: all three backends compile
   - Browser launch: `xdg-open` (Linux), `open` (macOS), `start` (Windows) — verify detection logic exists
   - File paths: forward slashes vs backslashes — verify path handling is platform-safe

### Files to Read
- `pkg/keychain/keychain_windows.go`
- `internal/tools/bash.go` — PTY vs pipe fallback
- `.github/workflows/ci.yml` — build matrix configuration

---

## Task P8.4 — Documentation

### Current State
- `README.md` exists (brief overview)
- `docs/ARCHITECTURE.md` comprehensive (dependency graph, data flows, threading model)
- `docs/INTERFACES.md` exists
- `docs/TYPES.md` exists
- Missing: `CONTRIBUTING.md`, `CHANGELOG.md`, detailed slash command reference, config reference

### What to Create

1. **`README.md` (rewrite)** — Production-quality README with:
   - Hero section: what M31A is, 1-sentence description
   - Quick start (30-second install): `curl -fsSL https://... | bash`
   - First-run walkthrough (screenshot or ASCII art of REPL screen)
   - Six-phase workflow overview
   - All slash commands table (name, description, example)
   - Config reference (all fields with defaults)
   - Key bindings reference (REPL, Plan, Execute, Verify, Ship screens)
   - Project status (v1.0 ready, V1.1 planned)
   - License (MIT)

2. **`CONTRIBUTING.md`** — Developer guide:
   - Development setup (Go version, dependencies, `go mod tidy`)
   - Build commands (`make build`, `make test`, `make lint`)
   - Test instructions (`go test -race ./...`, coverage requirements)
   - PR conventions (commit message format, changelog entries, test requirements)
   - Architecture overview link (`docs/ARCHITECTURE.md`)
   - How to add a new tool, new phase, new TUI screen

3. **`CHANGELOG.md`** — Start with v1.0.0 entry:
   ```
   ## v1.0.0 — YYYY-MM-DD

   ### Features
   - Six-phase workflow: Initialize, Discuss, Plan, Execute, Verify, Ship
   - Dual provider support: OpenRouter + OpenCode Zen
   - 10-screen Bubble Tea TUI with dark/light themes
   - 5 core tools: Bash, FileRead, FileWrite, Glob, Grep
   - Model selector with fuzzy search and cost comparison
   - Auto-fallback on provider failure (429/503)
   - Cross-session learning ledger
   - Commit rollback chain with git bisect integration
   - AutoDream context consolidation
   - Model arbitrage for cost optimization
   - Session persistence and resume
   - OS keychain integration (Linux/macOS/Windows)
   - 16+ slash commands

   ### Technical
   - Static binary (CGO_ENABLED=0) for Linux, macOS, Windows
   - ~24,000 LOC Go with 50+ test files
   - Structured logging (slog) with rotation
   - No telemetry, no analytics, no phone-home
   ```

4. **`docs/SLASH_COMMANDS.md`** — Complete reference for all slash commands:
   - Name, description, arguments, example usage
   - Group by category (session, provider, workflow, config, utility)

5. **`docs/CONFIG.md`** — Configuration reference:
   - All `config.toml` fields with defaults
   - Environment variable overrides
   - Keychain integration details
   - Example config file

6. **`--help` flag**: Ensure `m31a --help` prints all CLI flags, subcommands, and env vars.
   - Read `cmd/m31a/main.go` — add flag parsing if not present
   - Use `flag` package or `cobra` if already in go.mod

### Files to Read
- `README.md` (current)
- `docs/ARCHITECTURE.md`
- `internal/config/types.go` — all config fields
- `internal/tui/commands.go` — all slash commands
- `cmd/m31a/main.go` — CLI flags

---

## Task P8.5 — Release Pipeline

### Current State
- `.github/workflows/ci.yml` has GoReleaser release job
- Build matrix: 5 platforms
- Missing: Homebrew tap, install script, `cosign` signing

### What to Do

1. **GoReleaser config** (`.goreleaser.yaml`):
   - Verify it exists or create it
   - Builds for: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
   - Archives: `.tar.gz` for Unix, `.zip` for Windows
   - Checksums: `checksums.txt` with SHA256
   - Changelog: auto-generated from git commits since last tag
   - Release notes: include feature list from CHANGELOG.md

2. **Homebrew tap formula**:
   - Create `homebrew-m31a` repo structure or add formula to existing tap
   - Template:
   ```ruby
   class M31a < Formula
     desc "Terminal AI coding assistant with six-phase workflow"
     homepage "https://github.com/eshanized/M31A"
     url "https://github.com/eshanized/M31A/releases/download/v1.0.0/m31a_1.0.0_darwin_arm64.tar.gz"
     sha256 "<SHA256>"
     version "1.0.0"

     def install
       bin.install "m31a"
     end

     test do
       system "#{bin}/m31a", "--version"
     end
   end
   ```

3. **Install script**:
   Create `install.sh`:
   ```bash
   #!/usr/bin/env bash
   set -euo pipefail
   # Detect OS and arch
   # Download appropriate binary from GitHub releases
   # Verify checksum
   # Install to /usr/local/bin (or ~/.local/bin)
   # Print success message
   ```

4. **cosign signing** (optional but recommended):
   - Add cosign verification to install script
   - Sign release artifacts in GoReleaser workflow
   - Upload `.sig` and `.pub` files with release

### Files to Read
- `.github/workflows/ci.yml` — current release configuration
- `go.mod` — module path for download URLs

---

## Task P8.6 — v1.0.0 Tag & Release

### What to Do

1. **Acceptance criteria verification** — The roadmap lists 26 acceptance criteria.
   Create a checklist script `scripts/verify_v1.sh` that verifies each one programmatically where possible:
   - Binary builds cleanly
   - All tests pass
   - No race conditions
   - Coverage above threshold
   - All 6 workflow phases complete end-to-end (against mock provider)
   - All 5 tools execute correctly
   - Provider auto-fallback works
   - Session save/resume works
   - All TUI screens render
   - Slash commands parse and route
   - Ledger file grows after Ship
   - Rollback creates backup branch
   - AutoDream triggers on context threshold
   - Model arbitrage suggests cheaper models
   - OS keychain stores/retrieves keys
   - Theme cycles dark/light
   - Permission modal gates dangerous tools
   - Thinking blocks render and collapse
   - Tool cards show correct status
   - Health check updates connection badge
   - First-run screen completes setup
   - Settings screen saves config atomically
   - Resume screen detects corrupted sessions
   - git bisect integration finds offending commit
   - Checkpoint system saves/restores state
   - Graceful shutdown saves session on Ctrl+C

2. **Create v1.0.0 tag**:
   ```bash
   git tag -a v1.0.0 -m "v1.0.0: Initial release"
   git push origin v1.0.0
   ```

3. **GitHub release**:
   ```bash
   gh release create v1.0.0 --generate-notes --draft
   ```

---

## Implementation Order

Execute tasks in this order:

1. **P8.1** — Error handling audit (read all error paths, add graceful shutdown)
2. **P8.2** — Test coverage pass (run coverage, fill gaps, race detector)
3. **P8.3** — Cross-platform (Windows keychain, cross-compile verification)
4. **P8.4** — Documentation (README, CONTRIBUTING, CHANGELOG, SLASH_COMMANDS, CONFIG)
5. **P8.5** — Release pipeline (GoReleaser, Homebrew, install script)
6. **P8.6** — v1.0.0 tag (acceptance criteria verification, tag, release)

---

## Verification Commands

After all tasks are complete:

```bash
cd /home/snigdha/Desktop/Helix/M31A

# 1. Clean build
go mod tidy
CGO_ENABLED=0 go build -o m31a ./cmd/m31a

# 2. Vet
go vet ./...

# 3. Test with race detector and coverage
go test -race -cover -coverprofile=coverage.out ./...
go tool cover -func=coverage.out | tail -5

# 4. Cross-compile all platforms
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a
GOOS=linux GOARCH=arm64 CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a
GOOS=darwin GOARCH=amd64 CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a
GOOS=darwin GOARCH=arm64 CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a
GOOS=windows GOARCH=amd64 CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a

# 5. Verify documentation exists
test -f README.md && echo "README: OK"
test -f CONTRIBUTING.md && echo "CONTRIBUTING: OK"
test -f CHANGELOG.md && echo "CHANGELOG: OK"
test -f docs/SLASH_COMMANDS.md && echo "SLASH_COMMANDS: OK"
test -f docs/CONFIG.md && echo "CONFIG: OK"

# 6. Verify release files
test -f .goreleaser.yaml && echo "GoReleaser: OK"
test -f install.sh && echo "Install script: OK"

# 7. Run acceptance criteria verification script
bash scripts/verify_v1.sh
```

---

## Constraints

- Follow `AGENTS.md` rules strictly
- Bubble Tea is single-threaded — all state mutations through `Update()` only
- No CGO. Binary must be static (`CGO_ENABLED=0`)
- No telemetry. No analytics. No external calls except OpenRouter/Zen
- No V1.1 features (ghost mode, PiP, subagents, deferred tools)
- No hardcoded model lists
- Keep documentation concise but complete — no fluff
- All test changes must pass existing tests (no regressions)

---

## Expected Deliverables

1. **Code changes**:
   - Error handling improvements (wrapped errors, graceful shutdown, offline mode)
   - Windows keychain implementation (`go-wincred`)
   - Test coverage improvements (new test files, gap coverage)
   - `--help` flag implementation (if missing)

2. **New files**:
   - `CONTRIBUTING.md`
   - `CHANGELOG.md`
   - `docs/SLASH_COMMANDS.md`
   - `docs/CONFIG.md`
   - `install.sh`
   - `.goreleaser.yaml` (if missing)
   - `scripts/verify_v1.sh`
   - `coverage.html` (generated artifact)

3. **Updated files**:
   - `README.md` (rewritten)
   - `pkg/keychain/keychain_windows.go` (implemented)
   - `internal/tui/app.go` (graceful shutdown)
   - Various test files (coverage gaps filled)

4. **Release artifacts**:
   - v1.0.0 git tag
   - GitHub release (draft)
   - Homebrew tap formula

5. **Walkthrough document**:
   - `rush/walkthrough_phase8.md` — summary of all changes, coverage report, build verification, documentation index
