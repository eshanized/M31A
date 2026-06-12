## Summary

A short (1-3 sentence) description of what this PR does and why.

Fixes # (issue number, if applicable)

## Type of change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Refactor (no behavior change)
- [ ] Docs only
- [ ] Tests only

## Changes

- Bullet-pointed list of the main changes
- One concern per bullet — keep it scannable

## Architecture compliance

- [ ] This PR respects the package dependency graph in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [ ] No new V1.1 features introduced (ghost, PiP, subagents, deferred tools)
- [ ] No new tools beyond the V1 set (Bash, FileRead, FileWrite, Glob, Grep) without discussion
- [ ] No telemetry, analytics, or phone-home behavior added
- [ ] `CGO_ENABLED=0` still produces a clean build

## Checklist

- [ ] `gofmt -w .` is clean
- [ ] `golangci-lint run ./...` passes
- [ ] `go test -race ./...` passes
- [ ] New code meets the 75% coverage threshold (90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`)
- [ ] Exported functions and types have doc comments
- [ ] Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`)
- [ ] Docs updated (`docs/`, `CONTRIBUTING.md`, or inline) where behavior changed
- [ ] One concern per PR — this PR does not mix unrelated changes

## Screenshots / recordings

If the PR changes TUI behavior, attach a screenshot or screen recording here.

## Testing

Describe how you tested this change. Include any manual REPL commands, workflow runs, or edge cases you exercised.

```
$ m31a
> /workflow ...
```
