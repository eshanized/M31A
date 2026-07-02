# AGENTS.md — M31A

## Quick commands

```bash
make build          # optimized binary (CGO_ENABLED=0, static)
make test           # race-enabled tests with coverage
make lint           # golangci-lint (govet, staticcheck, errcheck, ineffassign, unused)
make check          # fmt → tidy → vet → test in sequence
make test-fast      # tests without race detector
make test-specific TEST=TestFoo   # run one test
make cover          # generate HTML coverage report
make debug          # build with debug symbols (no strip)
make clean          # remove build artifacts
```

**Build requirements:** Go 1.25+, `CGO_ENABLED=0` (hard constraint — binary must be static).

## Build + verify order

```
make check          # canonical full check (fmt → tidy → vet → test)
make lint           # standalone lint (golangci-lint, 5m timeout)
make test           # standalone tests
```

For a quick smoke test: `make test-fast`.

## Coverage targets

- **75%** overall
- **90%** for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

## Architecture (what matters for code changes)

- **Entry point:** `cmd/m31a/main.go` — flag parsing, config load, provider registration, TUI construction
- **TUI is Bubble Tea (Elm architecture).** All state mutations go through `Update()` only. Never mutate `AppState` from a goroutine. Use `tea.Cmd` / `tea.Msg`.
- **Workflow engine:** `internal/workflow/engine.go` — seven phases (Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship)
- **Provider layer:** `internal/provider/` — three providers (OpenRouter, Zen, Nvidia), never direct Anthropic/OpenAI. Models discovered dynamically from APIs.
- **Tools:** 18 built-in in `internal/tools/`, registered in `defaults.go`. Dispatcher in `dispatcher.go` handles permissions, rate limiting, concurrency.
- **Dependency rule:** `pkg/` must NOT import `internal/`. Enforced by Go module system.
- **`internal/types`** is the shared type vocabulary across all layers.

## Code style

- All code must be `gofmt`-clean
- Imports: group stdlib / third-party / project, use `goimports`
- **No emojis** in code or docs
- Return errors, never panic. Wrap with `fmt.Errorf("%w", err)`
- Exported functions/types need doc comments

## Conventional commits

`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`

## Lint config

Enabled linters: govet (with shadow), staticcheck, errcheck, ineffassign, unused.
Test files are excluded from `errcheck` and `unused`.

## E2E tests

`e2e_test.go` compiles and runs the binary. Real API tests (`TestBinary_Prompt_*RealAPI`) require env vars (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`) — they skip when unset.

## Cross-compilation

Targets: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/amd64 (windows/arm64 excluded).
Use `make cross` or goreleaser.

## Key gotchas

- `go.mod` says `go 1.25.0` — verify this matches your Go version
- No CGO anywhere — if any dependency requires it, the build breaks
- Bubble Tea is strictly single-threaded — use channels, never shared mutable state from goroutines
- Provider model lists are dynamic — never hardcode model names
- API keys go through OS keychain (`pkg/keychain/`) — never written to disk in plaintext
- `.env` files are gitignored except `.env.example`
