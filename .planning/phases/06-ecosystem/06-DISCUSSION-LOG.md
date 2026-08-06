# Phase 6: Ecosystem - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-06
**Phase:** 6-Ecosystem
**Areas discussed:** Extension Loading Mechanism, Plugin API Surface, Configuration Profiles, Workflow Extensibility, Extension Distribution, Contribution Workflow & Templates, Documentation & Sample Projects, CI/Release/Benchmark Automation

---

## Extension Loading Mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| External CLI tools (stdio/JSON-RPC) | Extensions are separate executables communicating via stdio/JSON-RPC. M31A spawns them as child processes. Language-agnostic, no CGO, strong isolation. | |
| WASM modules (wazero) | Extensions are WASM modules loaded via a WASM runtime (e.g., wazero). Sandboxed, language-agnostic, but adds runtime dependency and complexity. | |
| Config-based external commands | Extensions register via config file pointing to external commands. M31A executes them as subprocesses. Simple, no new dependencies, but less dynamic. | ✓ |
| Let the agent decide | Let the agent research and recommend based on codebase patterns and constraints. | |

**User's choice:** Config-based external commands
**Notes:** Simpler approach, no new runtime dependencies, leverages existing subprocess execution patterns in the codebase.

---

## Plugin API Surface

| Option | Description | Selected |
|--------|-------------|----------|
| New public pkg/extensions package | Create a new `pkg/extensions` package with stable public interfaces. External tools/providers implement these. Internal adapters bridge to existing Dispatcher/Registry. | ✓ |
| JSON-RPC over stdio | Define a JSON-RPC protocol over stdio. Extensions implement the protocol in any language. M31A communicates via subprocess stdio. | |
| Separate Go module for plugin interfaces | Define a minimal Go interface in a separate module (e.g., github.com/eshanized/m31a-ext). External plugins import it. M31A loads via config-based commands. | |
| Let the agent decide | Let the agent research and recommend based on codebase patterns. | |

**User's choice:** New public pkg/extensions package
**Notes:** Keeps everything in Go, leverages existing interface patterns (Tool, LLMProvider), internal adapters bridge to core systems.

---

## Configuration Profiles

| Option | Description | Selected |
|--------|-------------|----------|
| Layered TOML with named profiles | Layered config: global → org → workspace → project → env vars. Each layer overrides previous. Profiles are named config subsets selectable via flag/env. | |
| TOML profiles with inheritance | Keep TOML but add `profiles` section with named configs. Use `extends` for inheritance. Active profile via `--profile` flag or `M31A_PROFILE` env. | |
| Multi-file layered config | Use separate files: `config.toml` (global), `.m31a/workspace.toml` (org/workspace), `m31a.json` (project). Merge at load time with clear precedence. | ✓ |
| Let the agent decide | Let the agent research and recommend based on existing config loader patterns. | |

**User's choice:** Multi-file layered config
**Notes:** Clear separation of concerns, explicit precedence, leverages existing TOML loader with multi-file merge.

---

## Workflow Extensibility

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed phases, extensible tools/providers only | Extensions only provide tools and providers. Workflow phases remain fixed in core. Simplest, maintains reliability. | |
| Phase hooks (pre/post each phase) | Allow extensions to register custom phase handlers that run at specific points (e.g., post-plan, pre-execute). Core phases stay fixed, but hooks enable customization. | ✓ |
| Custom phase registration | Allow extensions to define entirely new phases that can be inserted into the workflow. More flexible but complex phase ordering/dependencies. | |
| Let the agent decide | Let the agent research and recommend based on engine architecture. | |

**User's choice:** Phase hooks (pre/post each phase)
**Notes:** Balances extensibility with stability. Core phases remain predictable, hooks allow customization without phase ordering complexity.

---

## Extension Distribution

| Option | Description | Selected |
|--------|-------------|----------|
| Decentralized (GitHub Releases, package managers) | Extensions are just Go binaries/scripts distributed via GitHub Releases, Homebrew, Scoop, etc. M31A config points to local paths. No central registry needed. | ✓ |
| Curated index in GitHub repo | Maintain a curated index (JSON/YAML) in the M31A repo or a separate repo. Users browse/install via CLI command. Lightweight registry. | |
| Registry service with search/versioning | Build a simple extension registry service (or use GitHub API) with search, versioning, dependency resolution. More features but more infrastructure. | |
| Let the agent decide | Let the agent research and recommend based on project scope. | |

**User's choice:** Decentralized (GitHub Releases, package managers)
**Notes:** Zero infrastructure overhead, aligns with Go ecosystem conventions, users already familiar with Homebrew/Scoop/GitHub Releases.

---

## Contribution Workflow & Templates

| Option | Description | Selected |
|--------|-------------|----------|
| Standard GitHub templates + CONTRIBUTING.md | Standard GitHub PR template, issue templates (bug, feature, question), contributing.md with workflow, conventional commits, DCO sign-off. Follow existing patterns. | |
| Templates + automated enforcement | Add PR checklist, automated checks (lint, test, build), required reviewers, branch protection rules. More enforcement. | |
| Full process docs + automation | Add issue triage labels, milestone planning, release checklist, semantic versioning guidelines. Full process documentation. | ✓ |
| Let the agent decide | Let the agent decide based on project needs. | |

**User's choice:** Full process docs + automation
**Notes:** Complete contribution lifecycle coverage, automated enforcement reduces maintainer burden.

---

## Documentation & Sample Projects

| Option | Description | Selected |
|--------|-------------|----------|
| Extension API docs + authoring guide + samples | API reference for pkg/extensions, extension authoring guide, 2-3 sample extensions (tool, provider, workflow hook), architecture decision records. | |
| Comprehensive docs + tutorial + testing guide | Above + interactive tutorial, migration guide for internal-to-external patterns, extension testing patterns, FAQ. | ✓ |
| Minimal viable docs | Minimal: just API reference and one sample extension. Expand later based on feedback. | |
| Let the agent decide | Let the agent decide based on scope. | |

**User's choice:** Comprehensive docs + tutorial + testing guide
**Notes:** Invests in developer experience upfront to accelerate ecosystem adoption.

---

## CI/Release/Benchmark Automation

| Option | Description | Selected |
|--------|-------------|----------|
| Enhance existing GoReleaser + benchmarks + compat tests | GoReleaser already configured. Add: benchmark CI job with regression detection (benchstat), extension compatibility test matrix, automated changelog from conventional commits, release notes template. | |
| Full automation + nightly + dashboards + dep updates | Above + scheduled nightly builds, performance dashboards, automated dependency updates (Dependabot/Renovate), release candidate promotion workflow. | ✓ |
| Minimal: just benchmark regression detection | Keep current GoReleaser setup, only add benchmark regression detection. Minimal change. | |
| Let the agent decide | Let the agent decide based on current CI state. | |

**User's choice:** Full automation + nightly + dashboards + dep updates
**Notes:** Comprehensive automation reduces manual release overhead, nightly catches regressions early, dashboards provide visibility.

---

## the agent's Discretion

- Agent may design the exact `pkg/extensions` interface signatures
- Agent may choose hook point names and payload structure
- Agent may select specific sample extensions to build
- Agent may design the multi-file config merge precedence rules
- Agent may choose benchmark suite and regression thresholds

---

## Deferred Ideas

None — discussion stayed within phase scope