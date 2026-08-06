---
phase: 06-ecosystem
plan: 06
subsystem: contribution-workflow
tags: [github-templates, codeowners, labeler, pr-checks, contributing, release, semantic-versioning]
key-files:
  created:
    - .github/ISSUE_TEMPLATE/bug_report.yml
    - .github/ISSUE_TEMPLATE/feature_request.yml
    - .github/ISSUE_TEMPLATE/extension_proposal.yml
    - .github/PULL_REQUEST_TEMPLATE.md
    - .github/CODEOWNERS
    - .github/labeler.yml
    - .github/workflows/pr-checks.yml
    - docs/contributing.md
    - docs/release-checklist.md
    - docs/semantic-versioning.md
tech-stack:
  patterns:
    - GitHub Actions PR validation
    - CODEOWNERS for required reviews
    - Labeler for auto-labeling
    - Dependabot for dependency updates
key-decisions:
  - D-06: Full process docs + automation (issue triage, PR templates, CODEOWNERS, labeler, PR checks)
  - 3 issue templates: bug, feature, extension
  - PR template with conventional commits checklist
  - CODEOWNERS per subsystem
  - Labeler by file paths
  - PR checks: lint, test, build, security, optional benchmark
  - Semantic versioning: MAJOR for breaking API, MINOR for features, PATCH for fixes
  - Extension API versioning independent of M31A version
  - Deprecation policy: 2 minor versions notice
requirements-completed:
  - D-06
duration: 45 min
completed: "2026-08-06T14:30:00Z"
---

# Phase 06 Plan 06: Contribution Workflow Automation — Summary

**One-liner:** Implemented complete contribution workflow automation with GitHub templates, CODEOWNERS, labeler, PR checks workflow, and comprehensive contribution documentation.

## Accomplishments

### 1. Issue Templates (3 structured forms)

**.github/ISSUE_TEMPLATE/bug_report.yml:**
- Description, steps to reproduce, expected/actual behavior
- Environment fields (OS, Go version, M31A version, config)
- Logs/output field with shell rendering
- Auto-adds "bug" label

**.github/ISSUE_TEMPLATE/feature_request.yml:**
- Problem statement, proposed solution, alternatives considered
- Use cases, breaking changes dropdown
- Extension-related flag
- Auto-adds "enhancement" label

**.github/ISSUE_TEMPLATE/extension_proposal.yml** (new for ecosystem):
- Extension type dropdown (tool/provider/hook)
- Name, description, use case
- Config example (JSON)
- Distribution plan (GitHub Releases, Homebrew, Scoop)
- Maintenance commitment
- Auto-adds "extension" label

### 2. PR Template (.github/PULL_REQUEST_TEMPLATE.md)
- Description, related issue
- Type: feat/fix/docs/test/refactor/chore
- Testing section with commands run
- Comprehensive checklist (make check, tests, docs, changelog, breaking changes, conventional commits)

### 3. CODEOWNERS (.github/CODEOWNERS)
Per-subsystem required reviewers:
- Global: @eshanized @maintainer-team
- Engine: /internal/engine/ @engine-maintainers
- Tools: /internal/tools/ @tools-maintainers
- Providers: /internal/integrations/provider/ @provider-maintainers
- TUI: /internal/ui/tui/ @tui-maintainers
- Extensions: /pkg/extensions/ @extensions-maintainers
- Extension Docs: /docs/extensions/ @docs-maintainers
- CI/Infra: /.github/ @infra-maintainers
- Config: /internal/core/config/ @config-maintainers
- Docs: /docs/ @docs-maintainers

### 4. Labeler (.github/labeler.yml)
Auto-labels PRs by file paths:
- bug, enhancement, extension, engine, tools, providers, tui
- docs, ci, config, testing, security, breaking-change

### 5. PR Checks Workflow (.github/workflows/pr-checks.yml)
- Triggers: pull_request, pull_request_target
- Concurrency: cancel in-progress for same PR
- Jobs:
  - `lint`: gofmt + golangci-lint (5 min timeout)
  - `test`: go test -race (20 min), go vet
  - `build`: cross-platform matrix (6 targets)
  - `security`: govulncheck
  - `benchmark`: optional, triggered by "benchmark" label

### 6. Contribution Documentation

**docs/contributing.md** — Complete guide:
- Project philosophy (from ROADMAP)
- Ways to contribute (code, docs, extensions, testing, issues)
- Development setup (Go 1.25+, CGO_ENABLED=0, make commands)
- Workflow: fork → branch → commit (conventional) → PR → review → merge
- Code style: gofmt, imports, no emojis, error wrapping, doc comments
- Testing: make test, make test-fast, race detector, integration tests
- Commit messages: conventional commits format
- PR process: template, checks, review, approval, squash merge
- Issue triage: labels, priority, assignment
- Extension contribution: authoring guide link, review process
- Release process: versioning, changelog, GoReleaser
- Code of Conduct reference

**docs/release-checklist.md** — Pre-release verification (12+ items):
- All CI checks pass (lint, test, security, build, benchmark)
- Benchmark CI no regressions
- Nightly workflow passed
- Changelog generated from conventional commits
- Version bumped per semantic versioning
- GoReleaser dry-run succeeds
- Cross-platform builds verified (6 targets)
- Extension compat tests pass
- Documentation updated
- Migration guide for breaking changes

**docs/semantic-versioning.md** — Versioning rules:
- MAJOR: Breaking API changes (pkg/extensions, CLI, config schema)
- MINOR: New features, extension points, non-breaking additions
- PATCH: Bug fixes, docs, CI, internal refactors
- Pre-releases: -alpha, -beta, -rc suffixes
- Extension API versioning independent of M31A version
- Deprecation policy: 2 minor versions notice
- Decision flowchart and examples

## Verification

- All template files exist and are valid YAML/Markdown
- CODEOWNERS syntax valid
- Labeler config valid
- PR checks workflow YAML valid
- Documentation links work and reads well

## Deviations from Plan

None — plan executed exactly as written.

## Impact

New contributors can now:
- Submit quality bug reports, feature requests, extension proposals
- Follow clear PR process with automated checks
- Get routed to correct reviewers via CODEOWNERS
- Understand versioning and release process
- Build extensions with comprehensive documentation