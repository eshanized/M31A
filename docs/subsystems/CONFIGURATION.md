# M31A Configuration Reference

This document describes the configuration architecture, precedence hierarchy, schema options, and canonical profiles of the M31 Autonomous (M31A) runtime.

## Configuration Precedence Hierarchy (7 Tiers)

Configuration settings are resolved hierarchically across 7 distinct tiers, where higher tiers override lower tiers (with strict monotonic security constraints):

```
Tier 7: CLI Overrides       (--profile, --timeout, flags)
  ↑
Tier 6: Session Settings    (Active interactive cockpit session)
  ↑
Tier 5: Mission Manifest    (.m31a/mission.toml)
  ↑
Tier 4: Workspace Config    (<workspace>/.m31/config.toml)
  ↑
Tier 3: User Config         (~/.config/m31/config.toml)
  ↑
Tier 2: System Admin Config (/etc/m31/config.toml)
  ↑
Tier 1: Built-in Defaults   (Hardcoded in runtime binary)
```

### Platform-Aware Path Resolution

Configuration files are located according to standard operating system conventions:

| Platform | System Configuration | User Configuration | Workspace Configuration |
|:---|:---|:---|:---|
| **Linux** | `/etc/m31/config.toml` | `~/.config/m31/config.toml` | `<workspace>/.m31/config.toml` |
| **macOS** | `/Library/Application Support/m31/config.toml` | `~/Library/Application Support/m31/config.toml` | `<workspace>/.m31/config.toml` |
| **Windows** | `C:\ProgramData\m31\config.toml` | `%APPDATA%\m31\config.toml` | `<workspace>/.m31/config.toml` |

---

## Monotonic Security Inheritance

M31A enforces **monotonic security inheritance**:
- An overriding configuration tier (e.g. Workspace or Session) may **tighten** security invariants, add mandatory verification checks, or reduce resource budgets.
- An overriding tier **cannot weaken** security vetoes, disable built-in safety rules, expand filesystem boundaries beyond workspace roots, or bypass policy gates.

Attempts to weaken safety invariants fail configuration validation immediately.

---

## The 7 Canonical Configuration Profiles

M31A includes 7 built-in profiles tailored for distinct operational environments:

### 1. `safe`
The most restrictive profile, optimized for untrusted environments and sensitive codebases.
- **Autonomy Mode**: `Safe` (all mutations require human confirmation).
- **Network Access**: Denied completely.
- **Capabilities**: Read-only filesystem operations, repo search, and AST inspection.
- **Tool Permitted**: `read_file`, `list_files`, `glob`, `grep`, `repo_search`, `repo_symbols`.

### 2. `coding`
Optimized for standard software engineering workflows.
- **Autonomy Mode**: `Assisted` (workspace file modifications permitted; git pushes and destructive commands require confirmation).
- **Network Access**: Restricted to package registries and documentation endpoints.
- **Capabilities**: Read/write workspace files, local git operations, test and linter execution.
- **Tools Permitted**: Filesystem suite, git suite (except push), QA runners (`run_tests`, `run_linter`, `run_formatter`).

### 3. `research`
Designed for codebase exploration, documentation synthesis, and architectural discovery.
- **Autonomy Mode**: `Autonomous` (non-mutating).
- **Network Access**: Outbound read-only web search and documentation fetching permitted.
- **Capabilities**: Read-only filesystem, repo intelligence, web search, memory lookup.
- **Tools Permitted**: Read tools, symbol queries, dependency analyzers, web search tools.

### 4. `autonomous`
Fully autonomous mode for end-to-end task execution and automated refactoring.
- **Autonomy Mode**: `Autonomous` (unsupervised execution within hard budget boundaries).
- **Capabilities**: Full workspace toolchain, bounded subprocess execution, differential DAG replanning.
- **Budget Enforced**: 10-dimensional hard constraints.

### 5. `ci`
Tailored for non-interactive continuous integration pipelines.
- **Autonomy Mode**: `Unattended` (fail-closed: any prompt for approval immediately terminates with DENY).
- **Capabilities**: Bounded compilation, test runs, and completion report generation.
- **Output**: Machine-readable JSON scorecards and deterministic exit codes.

### 6. `security_review`
Specialized for vulnerability assessment and security audits.
- **Autonomy Mode**: `Safe` (read-only analysis).
- **Capabilities**: Dependency vulnerability scanners, secret detectors, AST tainted data flow analyzers.
- **Policy Overrides**: Extra scrutiny on credentials, network sockets, and process boundaries.

### 7. `release`
Designed for artifact staging, changelog preparation, and release attribution.
- **Autonomy Mode**: `Assisted` (commit tagging and push requires explicit authorization).
- **Capabilities**: Git attribution trailers, changelog compilation, checksum generation, and release reporting.

---

## Configuration Schema Example

A complete `config.toml` demonstrates the schema structure:

```toml
[runtime]
profile = "coding"
concurrency_limit = 4
workspace_root = "."
telemetry_enabled = true

[budget]
max_wall_clock_seconds = 3600
max_concurrent_agents = 4
max_agent_steps = 100
max_model_calls = 250
max_tokens = 500000
max_cost_usd = 10.00
max_cpu_seconds = 600
max_memory_bytes = 4294967296  # 4 GB
max_artifact_bytes = 104857600  # 100 MB
max_retries = 3

[policy]
default_decision = "ask"
allow_network = false
strict_workspace_root = true

[agents]
planner_model = "claude-3-7-sonnet"
implementer_model = "claude-3-7-sonnet"
verifier_model = "claude-3-5-haiku"
```
