# Contributing to M31A

> Welcome! M31A is a terminal-native AI coding agent focused on reliability, predictability, and developer experience. We're glad you're here.

## Project Philosophy

M31A follows these guiding principles (from our ROADMAP):

- **Reliability** — Build a foundation users can trust
- **Predictability** — Deterministic behavior, no surprises
- **Simplicity** — Prefer improving existing experience over expanding functionality
- **Performance** — Make it feel instantaneous
- **Developer Experience** — New contributors can understand and modify the codebase confidently
- **User Experience** — Users always understand what M31A is doing

## Ways to Contribute

- **Code** — Features, bug fixes, refactoring
- **Documentation** — Guides, API docs, tutorials
- **Extensions** — Custom tools, providers, workflow hooks
- **Testing** — Unit, integration, contract tests
- **Issues** — Bug reports, feature requests, extension proposals

## Development Setup

### Prerequisites

- Go 1.25+
- `CGO_ENABLED=0` (required for static builds)
- `golangci-lint` (for linting)
- `goreleaser` (for release builds)

### Quick Start

```bash
# Clone and build
git clone https://github.com/eshanized/M31A
cd M31A
make build

# Run tests
make test

# Run with race detector
make test-fast

# Full check (fmt → tidy → vet → lint → test)
make check
```

## Workflow

1. **Fork** the repository
2. **Branch** from `master`: `git checkout -b feat/my-feature`
3. **Commit** using [Conventional Commits](#commit-messages)
4. **Push** to your fork
5. **Open PR** against `master` using the [PR template](.github/PULL_REQUEST_TEMPLATE.md)
6. **Review** — Address feedback, CI must pass
7. **Merge** — Squash and merge after approval

## Code Style

- **Formatting:** `gofmt` / `goimports` (run `make fmt`)
- **Imports:** stdlib → third-party → project
- **No emojis** in code or docs
- **Error handling:** Wrap with `fmt.Errorf("%w", err)`, return errors never panic
- **Doc comments:** Required for exported functions/types
- **Max line length:** ~120 chars (soft)

## Testing

- **Unit tests:** Pure functions, edge cases
- **Integration tests:** Workflow logic, real LLM providers (mocked)
- **Race detector:** Always run `make test` (includes `-race`)
- **Coverage:** Target 75% overall, 90% for critical packages (`pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`)

```bash
# Fast (no race)
make test-fast

# Full (with race)
make test

# Specific test
make test-specific TEST=TestFoo

# Coverage report
make cover
```

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): concise description

- Detail 1
- Detail 2
```

| Type | Description |
|------|-------------|
| `feat` | New feature, endpoint, component |
| `fix` | Bug fix, error correction |
| `docs` | Documentation only |
| `test` | Test-only changes |
| `refactor` | Code cleanup, no behavior change |
| `chore` | Config, tooling, dependencies |

**Examples:**
```
feat(engine): add phase hook registration
fix(tools): handle EOF in subprocess stdout
docs(extensions): add FAQ for streaming
test(config): add merge precedence tests
```

## PR Process

1. **Template:** Use the [PR template](.github/PULL_REQUEST_TEMPLATE.md)
2. **Checks:** All CI must pass (lint, test, build, security)
3. **Review:** Required reviews from [CODEOWNERS](CODEOWNERS)
4. **Approval:** At least one approval from code owner
5. **Merge:** Squash and merge after approval

## Issue Triage

- **Labels:** bug, enhancement, extension, documentation, good-first-issue
- **Priority:** critical > high > medium > low
- **Assignment:** Maintainers triage and assign

## Extension Contribution

See [Extension Authoring Guide](docs/extensions/authoring-guide.md) and [Extension Proposal Template](.github/ISSUE_TEMPLATE/extension_proposal.yml).

## Release Process

1. Version bump per [Semantic Versioning](docs/semantic-versioning.md)
2. Changelog generated from conventional commits
3. `make release` (GoReleaser handles cross-platform builds)
6. Verify release artifacts on GitHub Releases
7. Announce (changelog, Discord, etc.)

See [Release Checklist](docs/release-checklist.md).

## Code of Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). We follow the Contributor Covenant.