# Phase 1: Repo Reorganization — Research

**Phase:** 1  
**Date:** 2026-07-21  
**Status:** Complete

---

## Current Package Inventory

| Package | Files | Target Layer |
|---------|-------|--------------|
| `internal/tui/` | 356 | `ui/` |
| `internal/tools/` | 108 | `tools/` |
| `internal/workflow/` | 95 | `engine/` |
| `internal/provider/` | 33 | `integrations/` |
| `internal/session/` | 16 | `engine/` |
| `internal/narrative/` | 16 | `engine/` |
| `internal/config/` | 13 | `core/` |
| `internal/codeintel/` | 13 | `integrations/` |
| `internal/testutil/` | 11 | `tests/testutil/` (root) |
| `internal/keychain/` | 7 | `integrations/` |
| `internal/types/` | 6 | `core/` |
| `internal/fileutil/` | 6 | `infrastructure/` |
| `internal/bisect/` | 6 | `engine/` |
| `internal/decision/` | 5 | `engine/` |
| `internal/context/` | 5 | `integrations/` |
| `internal/compaction/` | 5 | `engine/` |
| `internal/taskrunner/` | 4 | `engine/` |
| `internal/skills/` | 4 | `integrations/` |
| `internal/rollback/` | 4 | `engine/` |
| `internal/ledger/` | 4 | `integrations/` |
| `internal/shell/` | 3 | `integrations/` |
| `internal/retry/` | 2 | `infrastructure/` |
| `internal/errors/` | 2 | `core/` |
| `internal/history/` | 2 | `integrations/` |
| `internal/logging/` | 2 | `integrations/` |
| `internal/metrics/` | 3 | `integrations/` |
| `internal/autodream/` | 3 | `integrations/` |
| `internal/arbitrage/` | 3 | `integrations/` |
| `internal/coordinator/` | 2 | `engine/` |
| `internal/log/` | 3 | `integrations/` |
| `internal/tokens/` | 3 | `engine/` |
| `internal/wiring/` | 1 | (delete — integration tests) |

**Total Go files:** ~880 (source + tests)

---

## Target 6-Layer Structure

```
internal/
├── core/                    # Shared vocabulary — NO internal imports
│   ├── types/               # WorkflowPhase, Message, Tool, Task, ModelInfo, etc.
│   ├── errors/              # Sentinel errors + ProviderError, ToolError, ConfigError
│   ├── config/              # TOML config, 4-level prompt override, hot-reload
│   └── constants.go         # (from types/constants.go)
│
├── engine/                  # Workflow orchestration
│   ├── workflow/            # 7-phase engine, state machine, prompts, templates
│   ├── taskrunner/          # Kahn's algorithm task scheduler
│   ├── bisect/              # Git bisect wrapper
│   ├── rollback/            # Git-based rollback
│   ├── session/             # Session persistence, checkpoints
│   ├── compaction/          # LLM-based session summarization
│   ├── narrative/           # Event-to-progress transformer
│   ├── decision/            # Decision receipt ring buffer
│   ├── tokens/              # Token estimation with EMA calibration
│   └── coordinator/         # Per-session concurrency control
│
├── ui/                      # Bubble Tea TUI
│   ├── tui/                 # Main app, screens, components, layout, theme
│   │   ├── screens/         # 30+ screen models
│   │   ├── components/      # 56 reusable UI components
│   │   ├── layout/          # Box, constraints, page, stack, solver
│   │   ├── streaming/       # SSE streaming renderer
│   │   ├── theme/           # Lipgloss theme system
│   │   ├── commands/        # Slash command definitions
│   │   ├── a11y/            # Accessibility
│   │   └── tuitypes/        # TUI message types
│   └── narrative/           # (keep in engine — it's a transformer, not UI)
│
├── integrations/            # External system adapters
│   ├── provider/            # LLM provider abstraction + 3 implementations
│   ├── git/                 # Git operations wrapper
│   ├── keychain/            # OS keychain (Linux/macOS/Windows)
│   ├── shell/               # Platform-aware shell execution
│   ├── context/             # Dynamic context sources (git, env, datetime)
│   ├── history/             # Frecent prompt history
│   ├── ledger/              # LEDGER.md append-only session records
│   ├── metrics/             # Session metrics collector
│   ├── logging/             # Audit logging with secret redaction
│   ├── log/                 # Structured logging with slog
│   ├── autodream/           # AutoDream context compression
│   ├── arbitrage/           # Model-cost optimizer
│   ├── codeintel/           # 4-language code intelligence
│   └── skills/              # Skill discovery and registration
│
├── tools/                   # 18 built-in tools + dispatcher
│   ├── dispatcher.go        # Central executor
│   ├── defaults.go          # Tool registration
│   ├── interface.go         # Tool interface
│   ├── fileops/             # File operations
│   ├── exec/                # Bash, DevServer
│   ├── search/              # Glob, Grep, WebSearch, WebFetch
│   ├── ai/                  # AskUserQuestion
│   └── subagent/            # Parallel sub-agent manager
│
└── infrastructure/          # Cross-cutting utilities
    ├── fileutil/            # Atomic write, file locking
    ├── retry/               # Exponential backoff retry
    └── testutil/            # (Moves to tests/testutil/ at root)
```

---

## Import Dependency Graph

```
cmd/m31a → core/config, core/types, engine/workflow, ui/tui, ui/tui/theme,
           integrations/provider, integrations/git, integrations/keychain,
           integrations/ledger, integrations/log, tools, tools/subagent,
           engine/session, engine/rollback, engine/tokens, integrations/autodream

ui/tui → engine/workflow, integrations/provider, tools, engine/session,
         core/types, core/config, integrations/git, integrations/ledger,
         engine/rollback, integrations/autodream, engine/decision,
         engine/narrative, integrations/metrics, integrations/arbitrage,
         tools/subagent, integrations/keychain, integrations/history

engine/workflow → integrations/provider, tools, engine/session, core/types,
                  core/config, integrations/git, integrations/ledger,
                  engine/decision, engine/tokens, integrations/codeintel,
                  engine/compaction, integrations/context, integrations/metrics,
                  infrastructure/retry, engine/taskrunner

integrations/provider → core/types, core/errors
tools → core/types, core/config, core/errors, integrations/metrics,
        tools/fileops, tools/exec, tools/search, tools/ai, tools/subagent
engine/session → core/types, engine/coordinator, core/errors
core/types → (no internal imports — leaf package)
```

**Dependency direction (must be maintained):**
```
core ← engine ← ui
core ← integrations ← ui
core ← tools ← ui
infrastructure ← (everything)
```

---

## Files Requiring Import Path Updates

**Estimated:** 483 Go source files

| Source Layer | Files | Import Updates Needed |
|--------------|-------|----------------------|
| `cmd/m31a/` | 3 | ~15 imports to update |
| `internal/tui/` | 356 | ~200+ files with internal imports |
| `internal/workflow/` | 95 | ~80+ files with internal imports |
| `internal/tools/` | 108 | ~60+ files with internal imports |
| `internal/provider/` | 33 | ~20+ files with internal imports |
| Other packages | ~85 | ~50+ files with internal imports |

**Import path transformation:**
```
Old: github.com/eshanized/M31A/internal/types
New: github.com/eshanized/M31A/internal/core/types

Old: github.com/eshanized/M31A/internal/workflow
New: github.com/eshanized/M31A/internal/engine/workflow

Old: github.com/eshanized/M31A/internal/tui
New: github.com/eshanized/M31A/internal/ui/tui

Old: github.com/eshanized/M31A/internal/provider
New: github.com/eshanized/M31A/internal/integrations/provider

Old: github.com/eshanized/M31A/internal/tools
New: github.com/eshanized/M31A/internal/tools (unchanged)

Old: github.com/eshanized/M31A/internal/fileutil
New: github.com/eshanized/M31A/internal/infrastructure/fileutil
```

---

## Risk Analysis

### HIGH RISK
1. **Import path breakage** — Single missed import causes compile failure. Mitigation: Automated sed replacement + compile check after each wave.
2. **go:embed paths** — `internal/workflow/prompts/` and `internal/workflow/templates/` use `//go:embed`. Moving these breaks embed directives. Mitigation: Update embed paths or keep prompts/templates in place.
3. **Build tag conflicts** — Platform-specific files (keychain, shell, fileutil) use build tags. Mitigation: Preserve build tags, verify cross-compilation.

### MEDIUM RISK
4. **Test file movement** — Test files reference package-internal unexported symbols. Mitigation: Move tests with their packages.
5. **CI pipeline paths** — `.github/workflows/ci.yml` references Makefile targets. Mitigation: Makefile targets stay at root.
6. **goreleaser paths** — `.goreleaser.yaml` references `cmd/m31a/main.go`. Mitigation: Entry point path unchanged.

### LOW RISK
7. **Documentation links** — Internal docs reference file paths. Mitigation: Update docs after reorganization.
8. **Wiki content** — `M31A.wiki/` references code paths. Mitigation: Wiki is separate, update as needed.

---

## Recommended Execution Order

### Wave 1: Foundation (No Import Changes)
1. Create target directory structure (`internal/core/`, `internal/engine/`, `internal/ui/`, `internal/integrations/`, `internal/infrastructure/`)
2. Move `internal/types/` → `internal/core/types/` (leaf package, no internal imports)
3. Move `internal/errors/` → `internal/core/errors/`
4. Move `internal/config/` → `internal/core/config/`
5. Move `internal/fileutil/` → `internal/infrastructure/fileutil/`
6. Move `internal/retry/` → `internal/infrastructure/retry/`
7. **Compile check** — Only core and infrastructure imports break here

### Wave 2: Engine Layer
1. Move `internal/workflow/` → `internal/engine/workflow/` (update go:embed paths)
2. Move `internal/taskrunner/` → `internal/engine/taskrunner/`
3. Move `internal/bisect/` → `internal/engine/bisect/`
4. Move `internal/rollback/` → `internal/engine/rollback/`
5. Move `internal/session/` → `internal/engine/session/`
6. Move `internal/compaction/` → `internal/engine/compaction/`
7. Move `internal/narrative/` → `internal/engine/narrative/`
8. Move `internal/decision/` → `internal/engine/decision/`
9. Move `internal/tokens/` → `internal/engine/tokens/`
10. Move `internal/coordinator/` → `internal/engine/coordinator/`
11. **Compile check** — Engine layer imports break

### Wave 3: Integrations Layer
1. Move `internal/provider/` → `internal/integrations/provider/`
2. Move `internal/git/` → `internal/integrations/git/`
3. Move `internal/keychain/` → `internal/integrations/keychain/`
4. Move `internal/shell/` → `internal/integrations/shell/`
5. Move `internal/context/` → `internal/integrations/context/`
6. Move `internal/history/` → `internal/integrations/history/`
7. Move `internal/ledger/` → `internal/integrations/ledger/`
8. Move `internal/metrics/` → `internal/integrations/metrics/`
9. Move `internal/logging/` → `internal/integrations/logging/`
10. Move `internal/log/` → `internal/integrations/log/`
11. Move `internal/autodream/` → `internal/integrations/autodream/`
12. Move `internal/arbitrage/` → `internal/integrations/arbitrage/`
13. Move `internal/codeintel/` → `internal/integrations/codeintel/`
14. Move `internal/skills/` → `internal/integrations/skills/`
15. **Compile check** — Integrations layer imports break

### Wave 4: UI Layer
1. Move `internal/tui/` → `internal/ui/tui/`
2. **Compile check** — UI layer imports break

### Wave 5: Root Cleanup
1. Move `e2e_test.go` → `tests/e2e/e2e_test.go`
2. Move `internal/testutil/` → `tests/testutil/`
3. Move `.env.test` → `tests/.env.test`
4. Move audit/planning docs to `docs/archive/`
5. Delete backup files (`.bak`, `.bak2`, `.patch`)
6. Update `.gitignore`
7. **Final compile + test check**

---

## Specific File Moves

### Core Layer
```
internal/types/*        → internal/core/types/
internal/errors/*       → internal/core/errors/
internal/config/*       → internal/core/config/
```

### Engine Layer
```
internal/workflow/*     → internal/engine/workflow/
internal/taskrunner/*   → internal/engine/taskrunner/
internal/bisect/*       → internal/engine/bisect/
internal/rollback/*     → internal/engine/rollback/
internal/session/*      → internal/engine/session/
internal/compaction/*   → internal/engine/compaction/
internal/narrative/*    → internal/engine/narrative/
internal/decision/*     → internal/engine/decision/
internal/tokens/*       → internal/engine/tokens/
internal/coordinator/*  → internal/engine/coordinator/
```

### UI Layer
```
internal/tui/*          → internal/ui/tui/
```

### Integrations Layer
```
internal/provider/*     → internal/integrations/provider/
internal/git/*          → internal/integrations/git/
internal/keychain/*     → internal/integrations/keychain/
internal/shell/*        → internal/integrations/shell/
internal/context/*      → internal/integrations/context/
internal/history/*      → internal/integrations/history/
internal/ledger/*       → internal/integrations/ledger/
internal/metrics/*      → internal/integrations/metrics/
internal/logging/*      → internal/integrations/logging/
internal/log/*          → internal/integrations/log/
internal/autodream/*    → internal/integrations/autodream/
internal/arbitrage/*    → internal/integrations/arbitrage/
internal/codeintel/*    → internal/integrations/codeintel/
internal/skills/*       → internal/integrations/skills/
```

### Infrastructure Layer
```
internal/fileutil/*     → internal/infrastructure/fileutil/
internal/retry/*        → internal/infrastructure/retry/
```

### Test Consolidation
```
e2e_test.go             → tests/e2e/e2e_test.go
internal/testutil/*     → tests/testutil/
.env.test               → tests/.env.test
```

---

## Go Module Path Implications

**Module path stays the same:** `github.com/eshanized/M31A`

**No module rename needed** — only internal package paths change.

**Import path update pattern:**
```
sed -i 's|github.com/eshanized/M31A/internal/types|github.com/eshanized/M31A/internal/core/types|g'
sed -i 's|github.com/eshanized/M31A/internal/errors|github.com/eshanized/M31A/internal/core/errors|g'
sed -i 's|github.com/eshanized/M31A/internal/config|github.com/eshanized/M31A/internal/core/config|g'
sed -i 's|github.com/eshanized/M31A/internal/workflow|github.com/eshanized/M31A/internal/engine/workflow|g'
sed -i 's|github.com/eshanized/M31A/internal/taskrunner|github.com/eshanized/M31A/internal/engine/taskrunner|g'
sed -i 's|github.com/eshanized/M31A/internal/bisect|github.com/eshanized/M31A/internal/engine/bisect|g'
sed -i 's|github.com/eshanized/M31A/internal/rollback|github.com/eshanized/M31A/internal/engine/rollback|g'
sed -i 's|github.com/eshanized/M31A/internal/session|github.com/eshanized/M31A/internal/engine/session|g'
sed -i 's|github.com/eshanized/M31A/internal/compaction|github.com/eshanized/M31A/internal/engine/compaction|g'
sed -i 's|github.com/eshanized/M31A/internal/narrative|github.com/eshanized/M31A/internal/engine/narrative|g'
sed -i 's|github.com/eshanized/M31A/internal/decision|github.com/eshanized/M31A/internal/engine/decision|g'
sed -i 's|github.com/eshanized/M31A/internal/tokens|github.com/eshanized/M31A/internal/engine/tokens|g'
sed -i 's|github.com/eshanized/M31A/internal/coordinator|github.com/eshanized/M31A/internal/engine/coordinator|g'
sed -i 's|github.com/eshanized/M31A/internal/provider|github.com/eshanized/M31A/internal/integrations/provider|g'
sed -i 's|github.com/eshanized/M31A/internal/git|github.com/eshanized/M31A/internal/integrations/git|g'
sed -i 's|github.com/eshanized/M31A/internal/keychain|github.com/eshanized/M31A/internal/integrations/keychain|g'
sed -i 's|github.com/eshanized/M31A/internal/shell|github.com/eshanized/M31A/internal/integrations/shell|g'
sed -i 's|github.com/eshanized/M31A/internal/context|github.com/eshanized/M31A/internal/integrations/context|g'
sed -i 's|github.com/eshanized/M31A/internal/history|github.com/eshanized/M31A/internal/integrations/history|g'
sed -i 's|github.com/eshanized/M31A/internal/ledger|github.com/eshanized/M31A/internal/integrations/ledger|g'
sed -i 's|github.com/eshanized/M31A/internal/metrics|github.com/eshanized/M31A/internal/integrations/metrics|g'
sed -i 's|github.com/eshanized/M31A/internal/logging|github.com/eshanized/M31A/internal/integrations/logging|g'
sed -i 's|github.com/eshanized/M31A/internal/log|github.com/eshanized/M31A/internal/integrations/log|g'
sed -i 's|github.com/eshanized/M31A/internal/autodream|github.com/eshanized/M31A/internal/integrations/autodream|g'
sed -i 's|github.com/eshanized/M31A/internal/arbitrage|github.com/eshanized/M31A/internal/integrations/arbitrage|g'
sed -i 's|github.com/eshanized/M31A/internal/codeintel|github.com/eshanized/M31A/internal/integrations/codeintel|g'
sed -i 's|github.com/eshanized/M31A/internal/skills|github.com/eshanized/M31A/internal/integrations/skills|g'
sed -i 's|github.com/eshanized/M31A/internal/fileutil|github.com/eshanized/M31A/internal/infrastructure/fileutil|g'
sed -i 's|github.com/eshanized/M31A/internal/retry|github.com/eshanized/M31A/internal/infrastructure/retry|g'
sed -i 's|github.com/eshanized/M31A/internal/testutil|github.com/eshanized/M31A/tests/testutil|g'
```

---

## Test File Movement Strategy

**Rule:** Move test files with their packages. Test files reference unexported symbols, so they must stay in the same package.

**Exception:** `internal/wiring/regression_test.go` — integration test that imports multiple packages. Move to `tests/integration/` or delete if obsolete.

**Test infrastructure:**
```
internal/testutil/*     → tests/testutil/
e2e_test.go             → tests/e2e/e2e_test.go
.env.test               → tests/.env.test
```

**After movement:** Update test import paths using same sed patterns.

---

## go:embed Path Updates

**Files using go:embed:**
- `internal/workflow/prompts/loader.go` — embeds `prompts/*.md` and `prompts/models/*.txt`
- `internal/workflow/templates/` — embedded project templates

**After move to `internal/engine/workflow/`:**
- Update embed paths to relative location: `//go:embed prompts/*.md`
- Verify embed directives still resolve correctly

---

## Verification Checklist

After each wave:
1. `go build ./...` — compile check
2. `go vet ./...` — static analysis
3. `make lint` — golangci-lint

After all waves:
1. `go build ./...` — full compile
2. `go test ./...` — all tests pass
3. `make test` — race-enabled tests with coverage
4. `make lint` — full lint pass
5. Cross-compile check: `make cross` or `GOOS=linux GOARCH=arm64 go build ./cmd/m31a`

---

*Research complete: 2026-07-21*