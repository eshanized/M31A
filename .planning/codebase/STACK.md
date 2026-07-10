# STACK.md — Technology Stack

> Last updated: 2026-07-10

## Language and Runtime

| Property | Value |
|---|---|
| Language | Go |
| Version | 1.25.0 (`go.mod` line 3) |
| Module path | `github.com/eshanized/M31A` |
| CGO | **Disabled** (`CGO_ENABLED=0` — hard constraint) |
| Binary | Fully static, single-file |
| Entry point | `cmd/m31a/main.go` |

The `CGO_ENABLED=0` constraint is enforced in the `Makefile` (line 13: `GOFLAGS := CGO_ENABLED=0`) and `.goreleaser.yaml` (line 7). Any dependency requiring CGO breaks the build. This is a deliberate design choice for portability and single-binary distribution.

## Build System

### Makefile (`Makefile`)

The Makefile is the primary build orchestrator with 322 lines covering:

**Primary Targets:**
| Target | Purpose |
|---|---|
| `make build` | Optimized binary for current platform (`-trimpath`, `-s -w` strip) |
| `make debug` | Debug build with symbols (`-gcflags "all=-N -l"`) |
| `make dev` | Build and run immediately |
| `make run-latest` | Build from latest git tag and run |

**Testing Targets:**
| Target | Purpose |
|---|---|
| `make test` | Race-detector + coverage (`-race -cover -coverprofile`) |
| `make test-fast` | Tests without race detector |
| `make test-verbose` | Verbose test output |
| `make test-specific TEST=TestFoo` | Single test execution |
| `make bench` / `make bench-verbose` | Benchmarks with memory stats |
| `make cover` | HTML coverage report generation |

**Code Quality Targets:**
| Target | Purpose |
|---|---|
| `make check` | Canonical full pipeline: `fmt -> tidy -> vet -> lint -> test` |
| `make lint` | golangci-lint with 5m timeout |
| `make lint-fix` | golangci-lint with auto-fix |
| `make vet` | `go vet ./...` |
| `make fmt` | `gofmt` + `goimports` |
| `make tidy` | `go mod tidy` |

**Cross-Compilation:**
| Target | Platforms |
|---|---|
| `make cross` | All 6 platform combinations |
| `build-linux-amd64` | Linux x86_64 |
| `build-linux-arm64` | Linux ARM64 |
| `build-darwin-amd64` | macOS Intel |
| `build-darwin-arm64` | macOS Apple Silicon |
| `build-windows-amd64` | Windows x86_64 |
| `build-windows-arm64` | Windows ARM64 (excluded from goreleaser) |

**Utilities:**
- `make install` — `go install` to GOBIN
- `make install-{platform}` — Platform-specific install to `$INSTALL_PREFIX/bin` (default: `/usr/local`)
- `make clean` / `make nuke` — Artifact cleanup (nuke clears Go caches too)
- `make release` / `make release-dry` — goreleaser snapshot builds
- `make validate-release` — Full release validation via `scripts/validate-release.sh`
- `make version` — Print version/commit/date/Go info
- `make size` — Show binary size after build
- `make deps` / `make deps-verify` — Dependency management

**Build Flags (linker-injected):**
```go
// cmd/m31a/main.go lines 34-39
var (
    Version   = "dev"
    Commit    = "unknown"
    Date      = "unknown"
    GoVersion = "unknown"
)
```

Injected via `-ldflags "-s -w -X main.Version=... -X main.Commit=... -X main.Date=... -X main.GoVersion=..."` (Makefile line 12).

### goreleaser (`.goreleaser.yaml`)

Version 2 goreleaser config for release automation:

- **Platforms:** linux/darwin/windows x amd64/arm64 (windows/arm64 excluded)
- **Build flags:** Same as Makefile (`CGO_ENABLED=0`, `-trimpath`, `-s -w`)
- **Package formats:** deb, rpm, apk, archlinux (via nfpms)
- **Scoop bucket:** Windows package manager support
- **Checksums:** SHA256
- **Changelog:** Auto-generated, excludes docs/test/chore/ci/build commits
- **Release mode:** Draft with auto-prerelease detection

### Install Script (`install.sh`)

Bash installer for one-line installation:
```bash
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/install.sh | bash
```
Supports `--version` and `--bin-dir` flags. Auto-detects OS/arch.

### Scoop Manifest (`m31a.json`)

Windows Scoop package manager manifest for v1.6.1 with SHA256 hash verification.

## Key Dependencies

### Direct Dependencies (`go.mod` lines 5-26)

| Dependency | Version | Purpose |
|---|---|---|
| `charmbracelet/bubbletea` | v1.3.0 | TUI framework (Elm architecture) |
| `charmbracelet/lipgloss` | v1.1.0 | TUI styling/layout |
| `charmbracelet/bubbles` | v0.20.0 | TUI components (inputs, lists, etc.) |
| `charmbracelet/glamour` | v0.6.0 | Markdown rendering in terminal |
| `BurntSushi/toml` | v1.6.0 | TOML config file parsing |
| `bmatcuk/doublestar/v4` | v4.10.0 | Glob pattern matching |
| `godbus/dbus/v5` | v5.2.2 | Linux D-Bus for keychain (Secret Service API) |
| `mattn/go-runewidth` | v0.0.19 | Unicode width calculation for TUI |
| `pkoukk/tiktoken-go` | v0.1.8 | Token counting (tiktoken compatible) |
| `golang.org/x/sync` | v0.21.0 | `singleflight` for model cache dedup |
| `alecthomas/chroma` | v0.10.0 | Syntax highlighting |
| `atotto/clipboard` | v0.1.4 | Clipboard access |
| `fsnotify/fsnotify` | v1.10.1 | Filesystem event watching (config hot-reload) |
| `odvcencio/gotreesitter` | v0.20.5 | Tree-sitter integration for code intelligence |

### Notable Indirect Dependencies

| Dependency | Purpose |
|---|---|
| `microcosm-cc/bluemonday` | HTML sanitization |
| `yuin/goldmark` | Markdown parsing |
| `olekukonko/tablewriter` | Table rendering |
| `muesli/termenv` | Terminal environment detection |
| `google/uuid` | UUID generation |
| `golang.org/x/net` v0.56.0 | Network utilities |
| `golang.org/x/sys` v0.46.0 | System call wrappers |

### Dependency Architecture Rules

- `pkg/` must NOT import `internal/` — enforced by Go module system
- `internal/types` is the shared type vocabulary across all layers
- No vendor directory used (modules only)

## Configuration

### Config File Format: TOML

Primary config path: `~/.m31a/config.toml` (overridable via `M31A_CONFIG` env var).

Config struct defined in `internal/config/types.go`:
```go
type Config struct {
    Provider          ProviderConfig
    Model             ModelConfig
    UI                UIConfig
    Permissions       PermissionsConfig
    Features          FeaturesConfig
    Ledger            LedgerConfig
    Tools             ToolsConfig
    Agents            AgentsConfig
    Git               GitConfig
    Verify            VerifyConfig
    Compaction        CompactionConfig
    Instructions      InstructionsConfig
    Skills            SkillsConfig
    ModelCapabilities ModelCapabilitiesConfig
    Prompts           PromptConfig
    Narrative         NarrativeConfig
    Templates         TemplateConfig
}
```

### Config Loading (`internal/config/loader.go`)

- Uses `BurntSushi/toml` for parsing
- Supports project-level config merge (`internal/config/merge.go`)
- Hot-reload via `fsnotify` watcher (`ConfigWatchInterval = 5s`)
- Discovery walks up to 3 parent directories for project configs
- Default config returned if file is missing (not an error)
- API key resolution from keychain on load

### Environment Variables (`.env.example`)

```
OPENROUTER_API_KEY=     # OpenRouter API
ZEN_API_KEY=            # Zen API
NVIDIA_API_KEY=         # NVIDIA NIM API
```

Loaded via `config.LoadDotEnv()` at startup before logger init. `.env` files are gitignored.

### Prompt System (`internal/config/types.go` lines 41-70)

4-level prompt override chain:
1. Config override (`prompts.overrides[name]`)
2. Project-level (`.m31a/prompts/<name>.md`)
3. Global (`~/.m31a/prompts/<name>.md`)
4. Embedded default (`prompts/<name>.md` via `go:embed`)

## Dev Tooling

### Linting (`.golangci.yml`)

golangci-lint v2 configuration with 5 linters:

| Linter | Purpose |
|---|---|
| `govet` | Go vet checks (with shadow detection enabled) |
| `staticcheck` | Static analysis |
| `errcheck` | Unchecked error returns |
| `ineffassign` | Ineffective assignments |
| `unused` | Unused code detection |

**Config details:**
- Timeout: 5 minutes
- `errcheck`: type assertions and blank checks disabled
- Test files excluded from `errcheck` and `unused`
- Shadow detection enabled for `govet`

### Formatting

- `gofmt` — standard Go formatting (must be clean)
- `goimports` — import grouping (stdlib / third-party / project)

### Coverage Targets

- **75%** overall minimum
- **90%** for critical packages: `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

### Scripts (`scripts/`)

- `validate-release.sh` — Full release validation suite
- `verify_v1.sh` — V1 verification script

## Binary Targets

| Target | Output | Description |
|---|---|---|
| Default build | `m31a` | Optimized, stripped binary for current OS/arch |
| Debug build | `m31a-debug` | Debug symbols retained |
| Cross-compilation | `dist/m31a-{os}-{arch}` | Platform-specific binaries |

All binaries are statically linked (no CGO), using `-trimpath` for reproducible builds.

## Conventional Commits

Commit message format: `type: description`

Recognized types: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`

goreleaser changelog filters exclude: `docs:`, `test:`, `chore:`, `ci:`, `build:`, merge commits.
