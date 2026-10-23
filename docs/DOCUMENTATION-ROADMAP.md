# M31A Documentation Roadmap

> Status: proposed
>
> This document defines the documentation coverage plan for M31A. It is not a claim that every proposed page already exists or that every runtime capability is production-qualified.

## Objective

Make M31A understandable and operable by three audiences without forcing them to reverse-engineer source code:

1. **Users** installing M31A, configuring a provider, starting sessions and executions, reviewing changes, and recovering from errors.
2. **Contributors** changing the Rust runtime, tests, configuration, security controls, and TUI.
3. **Operators and auditors** evaluating trust boundaries, data handling, platform support, verification evidence, and release integrity.

Documentation must describe implemented behavior, not aspirational behavior. Source code, tests, runtime help output, CI workflows, and release artifacts must be checked before describing behavior as supported.

## Current baseline

The repository already has a root README, `AGENTS.md`, community contribution and security guidance, and subsystem documentation under `docs/`. Existing pages cover getting started, architecture, configuration, CLI, policy, security, tools, autonomy, recovery, testing, plugins, deployment, platform support, release engineering, and an audit/remediation report.

The gap is therefore not simply “no documentation.” The system needs better task-oriented user guidance, broader engineering references, clear source-of-truth rules, consistency checks, and a documented route from first install through advanced operation and contribution.

## Proposed information architecture

### Keep and improve existing pages

| Existing path | Required improvement |
|---|---|
| `README.md` | Product overview, truthful current status, minimal working quickstart, supported-platform caveats, links to task-based docs. Avoid duplicating entire subsystem manuals. |
| `docs/README.md` | Become the documentation home with audience-based paths, searchable page index, task flows, and status/evidence conventions. |
| `docs/getting-started/installation.md` | Exact install/update/verify/uninstall workflows; separate published artifacts from qualified platforms; checksum instructions for each OS. |
| `docs/getting-started/quickstart.md` | End-to-end verified walkthrough from initialization to a safe, inspectable change; expected output and cleanup. |
| `docs/architecture/ARCHITECTURE.md` | Source-verified modules, dependency direction, canonical IDs, execution lifecycle, data flow, and diagrams. Resolve contradictory role counts and capability counts. |
| `docs/architecture/RUNTIME-AUTHORITY.md` | Keep authority ownership and invariants; link to a concise contribution-oriented architecture overview. |
| `docs/architecture/PROMPT-AUTHORITY.md` | Document trust boundaries and actual sanitization/escaping contracts with tests as evidence. |
| `docs/subsystems/CLI.md` | Generate or cross-check the command reference against Clap definitions and actual `--help` output. Explicitly identify hidden/deprecated aliases. |
| `docs/subsystems/CONFIGURATION.md` | Complete, valid example config; exact file paths; precedence; typed settings; secret handling; validation and common failure messages. |
| `docs/subsystems/TOOLS.md` | Derive tool names and schemas from the current registry; document risk, approvals, bounds, and failure semantics. |
| `docs/subsystems/POLICY.md` | Decision table for ALLOW/DENY/ASK/ESCALATE, approval behavior, unattended mode, and immutable safety invariants. |
| `docs/subsystems/SECURITY.md` | Threat model, attacker capabilities, trust boundaries, isolation limits by platform, secret lifecycle, disclosure workflow. Avoid unsupported certification claims. |
| `docs/subsystems/RECOVERY.md` | Startup recovery, pause/resume/cancel, checkpoints, corrupted data, retry limits, recovery procedures. |
| `docs/subsystems/TESTING.md` | Tiered test commands, deterministic vs credential-gated tests, OS-specific tests, acceptance evaluation harness. |
| `docs/PLATFORM-SUPPORT.md` | A single authoritative status table: supported, integrated but not qualified, compile-checked, unsupported; evidence and known limitations per target. |
| `docs/DEPLOYMENT.md` and `docs/RELEASE.md` | Clear division between install/update/channel semantics and maintainer release procedure. |

### New task-oriented user guides

| Proposed path | Content |
|---|---|
| `docs/user-guide/README.md` | User-guide navigation and supported-workflow conventions. |
| `docs/user-guide/first-run.md` | Onboarding, credential setup, workspace initialization, model discovery, first execution. |
| `docs/user-guide/sessions-and-executions.md` | Distinguish sessions, execution runs, tasks, and missions/legacy aliases; lifecycle actions and examples. |
| `docs/user-guide/tui.md` | Actual TUI layout, panels, commands, keyboard shortcuts, approvals, settings, known limitations. Verify every shortcut against code and tests. |
| `docs/user-guide/reviewing-changes.md` | Inspect diffs and artifacts, interpret verification results, understand Git/worktree effects, safely accept or reject changes. |
| `docs/user-guide/model-configuration.md` | Provider support policy, model discovery, selection, metadata trust, credential storage, diagnosing empty/stale catalogs. |
| `docs/user-guide/settings.md` | Settings categories, precedence/provenance, confirmation and persistence behavior, restart-required settings. |
| `docs/user-guide/automation-and-ci.md` | Headless use, JSON output, exit codes, bounded runs, CI integration, non-interactive ASK fail-closed behavior. |

### New engineering references

| Proposed path | Content |
|---|---|
| `docs/architecture/data-model.md` | Canonical runtime entities and relationships; session/execution/task/job/artifact/checkpoint/event IDs; authoritative persistence. |
| `docs/architecture/execution-lifecycle.md` | State transitions from request intake through plan, scheduling, tool invocation, verification, checkpoint and final result. Include failure/cancellation/recovery paths. |
| `docs/architecture/storage-and-migrations.md` | StorageLayout, global vs workspace state, production vs development isolation, database migrations, legacy migration and backup/restore behavior. |
| `docs/architecture/security-boundaries.md` | Trust boundaries and side-effect authority from model output through policy to process/filesystem/network execution. |
| `docs/development/setup.md` | Prerequisites, Rust toolchain, clone/build workflow, environment, database setup, debug workflow. |
| `docs/development/testing.md` | Fast feedback loop, full verification matrix, platform-gated tests, live model tests, evaluation harness, interpreting failures. |
| `docs/development/adding-a-tool.md` | Required traits/registration, schema and permission declarations, policy integration, bounds, errors, telemetry, unit/integration/security tests. |
| `docs/development/adding-an-agent-role.md` | Role registry/profile, permitted capabilities, context contract, limits, handoff semantics, tests, docs updates. |
| `docs/development/changing-configuration.md` | Canonical default/resolver/schema/CLI/TUI wiring, precedence, compatibility, migrations and regression tests. |
| `docs/development/tui-development.md` | TUI state projection, event flow, rendering/input separation, terminal restoration, test fixtures and headless tests. |
| `docs/development/architecture-decision-records.md` | ADR format and location, decision status, alternatives, trade-offs, superseding decisions. |
| `docs/reference/environment-variables.md` | Enumerate supported non-secret environment overrides from implementation; state that production credentials are handled by the credential subsystem. |
| `docs/reference/configuration-schema.md` | Table of every canonical configuration field, type, default source, bounds, effect, precedence, restart needs. Prefer generation from schema where possible. |
| `docs/reference/exit-codes.md` | Stable exit-code meanings and command-specific output/automation implications. |
| `docs/reference/glossary.md` | Canonical definitions for workspace, session, execution run, task, job, mission, profile, capability, tool, policy decision, artifact, checkpoint, verification evidence. |

### Operations and support

| Proposed path | Content |
|---|---|
| `docs/operations/troubleshooting.md` | Symptom → diagnosis → safe remediation for startup, invalid config, credentials, model discovery, Git/worktree, sandbox, database, TUI, update/rollback. |
| `docs/operations/faq.md` | Concise answers to common install, provider, privacy, safety, supported OS and runtime behavior questions. |
| `docs/operations/data-and-privacy.md` | What is persisted locally, where logs/events/artifacts live, redaction boundaries, provider data flow, deletion/retention controls, and what the project cannot guarantee. |
| `docs/operations/backup-and-restore.md` | Identify authoritative stores, safe shutdown/backup procedure, restore compatibility, and integrity checks. |
| `docs/operations/incident-response.md` | Response to suspected credential exposure, unsafe changes, corrupted state, malicious repository content, and disclosure of security issues. |
| `docs/operations/upgrade-migration.md` | Upgrade, channel change, database/config migration, compatibility, rollback, and troubleshooting. |

## Documentation contract

1. **Evidence before claims:** every claim about a command, flag, default, policy stage, tool, role, file path, model, or platform status must be checked against executable source or tests.
2. **One canonical owner:** maintain one authoritative document per subject. Other pages summarize and link; they must not re-define conflicting details.
3. **Status language:** distinguish implemented, tested, CI-verified, release-qualified, and aspirational capabilities.
4. **Runnable examples:** commands and configurations must be checked in CI or a docs-contract test where feasible. Mark destructive actions and external side effects clearly.
5. **Safe by default:** never place real tokens, personal paths, or live secrets in examples. Use deliberately invalid example credentials only when illustrating validation.
6. **Version-aware:** specify version applicability for breaking changes and channel semantics. Keep release filenames/version numbers out of generic installation instructions when links can resolve to the latest release.
7. **No fictional evidence:** do not claim tests passed, security certification, signature verification, OS support, or provider support unless the relevant evidence exists.
8. **Accessible navigation:** every new page links to its parent index; every index link resolves; use descriptive page titles and short task-focused sections.

## Implementation order

### P0 — Correctness and navigation
- Audit all existing documentation against current implementation.
- Resolve contradictions in agent-role/tool/capability counts, execution terminology, configuration paths, storage paths, model provider policy, and platform qualification.
- Update root README and docs index to establish the canonical navigation.
- Add links to existing pages and remove stale commands or version-specific examples.
- Add a documentation consistency check for links, required files, commands/examples that can be tested, and source-verified tables.

### P1 — User success path
- Publish first-run, session/execution lifecycle, TUI, reviewing changes, model configuration, settings and automation/CI guides.
- Add troubleshooting covering the most common setup/run/recovery errors.
- Ensure a new user can get from a clean clone/install to a safely reviewed result without relying on undocumented assumptions.

### P2 — Contributor enablement
- Document data model, execution lifecycle, storage/migrations and security boundaries.
- Add focused contribution guides for tools, agent roles, config and TUI.
- Document local test workflows and platform/credential-gated test classes.
- Establish an ADR convention and update process for architecture changes.

### P3 — Operational maturity
- Add privacy/data lifecycle, backup/restore, upgrade/migration and incident response guides.
- Generate the CLI/config reference from executable metadata when feasible.
- Track docs drift as CI failures for broken links, stale command examples, missing index entries and contradictory canonical values.

## Definition of done

- The README directs users to one clear documentation landing page.
- A clean-install quickstart has been executed against the current default branch.
- Every documented CLI command and flag has been checked against current CLI definitions or help output.
- Agent/tool/capability counts and platform statuses match their authoritative registries and support evidence.
- Configuration paths, precedence, secret handling and state locations agree across all docs.
- All Markdown links resolve; every page is indexed; examples contain no real credentials.
- Documentation-related tests pass, and any tests requiring external credentials or a native platform are explicitly classified.
- The documentation explains what M31A does **not** guarantee, especially where binaries exist but runtime qualification or provider-backed verification is incomplete.
