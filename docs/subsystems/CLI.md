# M31A Command Line Interface (CLI) Reference

This document provides a comprehensive operational reference for the `m31a` command-line binary.

## Global Synopsis

```bash
m31a [OPTIONS] [COMMAND]
```

### Global Options

- `-c, --config <PATH>`: Custom path to runtime configuration file.
- `-p, --profile <NAME>`: Override active profile (`safe`, `coding`, `research`, `autonomous`, `ci`, `security_review`, `release`).
- `--model <MODEL>`: Override active model name for this invocation.
- `--autonomy <AUTONOMY>`: Override autonomy level/mode for this invocation.
- `-o, --output <FORMAT>`: Output format: `text` (default), `json`, `stream-json`.
- `-q, --quiet`: Suppress non-essential informational output.
- `-h, --help`: Print help information.
- `-V, --version`: Print version information.

---

## Exit Codes

M31A uses standardized UNIX exit codes to indicate execution results:

| Exit Code | Meaning | Description |
|:---:|:---|:---|
| **0** | Success | Command or mission completed successfully with all verification gates passed. |
| **1** | Verification Failure | Mission completed work but failed quality/verification checks (tests, linter). |
| **2** | Policy Violation | Action was blocked fail-closed by the security policy engine. |
| **3** | Budget Exhaustion | Mission exceeded one or more 10-dimensional hard resource bounds. |
| **4** | Crash / Infrastructure | Unexpected runtime crash, database corruption, or system failure. |
| **5** | Configuration Error | Invalid configuration schema, missing files, or monotonic violation. |

---

## Subcommand Reference

### 0. `session`
Interactive developer session operations.

- **`session new`**: Start a new interactive developer session (launches REPL).
- **`session list`**: List recorded sessions with status and workspace.
- **`session show <SESSION_ID>`**: Display session details and conversation history.
- **`session resume <SESSION_ID>`**: Resume an existing session from persisted state.

### 1. `mission`
Manage and execute autonomous missions.

- **`mission run <PROMPT>`**: Start a new autonomous mission.
  - `--profile <NAME>`: Profile override for this mission.
  - `--wait-for-approval`: Wait for operator approval when policy triggers an `Ask` decision.
- **`mission list`**: List all recorded missions.
  - `--all`: Include finished and cancelled missions.
- **`mission show <MISSION_ID>`**: Show detailed status, task DAG, and budget usage of a mission.
- **`mission pause <MISSION_ID>`**: Pause an in-progress mission at the next safe checkpoint.
- **`mission cancel <MISSION_ID>`**: Abort an in-progress mission and terminate all child processes.
- **`mission resume <MISSION_ID>`**: Resume an interrupted or crashed mission from its latest checkpoint.
- **`mission fork <MISSION_ID>`**: Fork a mission into a fresh branch/mission with an updated goal.

### 2. `task`
Inspect individual tasks within a mission DAG.

- **`task list <MISSION_ID>`**: List all tasks associated with a mission.
- **`task show <TASK_ID>`**: Display execution details, assigned agent, and verification evidence for a task.

### 3. `agent`
Inspect agent pool status and available roles.

- **`agent list`**: List active and idle agents in the runtime pool.

  The 8 canonical agent roles are: `planner`, `researcher`, `architect`, `implementer`, `reviewer`, `verifier`, `diagnostician`, `integrator`.

### 4. `capability`
Inspect registered runtime capabilities and providers.

- **`capability list`**: List all 15 core capability families and their operational availability.

### 5. `policy`
Evaluate security policy decisions.

- **`policy check <TOOL>`**: Dry-run evaluate an action against active policy layers.
  - `-m, --mission-id <ID>`: Optional mission ID context for evaluation.

### 6. `checkpoint`
Manage atomic two-phase mission checkpoints.

- **`checkpoint list <MISSION_ID>`**: List checkpoints created during a mission.
- **`checkpoint restore <CHECKPOINT_ID>`**: Revert workspace and runtime state to a specific checkpoint.

### 7. `artifact`
Inspect immutable content-addressed artifacts.

- **`artifact list <MISSION_ID>`**: List all artifacts produced by a mission.
- **`artifact show <ARTIFACT_ID>`**: Print metadata, SHA-256 digest, size, and content preview for an artifact.

### 8. `doctor`
Run environmental diagnostics and prerequisite health probes.

- **`doctor`**: Validates Git configuration, Cgroup v2 availability, POSIX rlimits support, compiler availability, and SQLite storage integrity.
  - `--json`: Emit machine-readable JSON health report.

### 9. `config`
Validate and inspect runtime configuration.

- **`config validate`**: Validate configuration syntax and monotonic inheritance rules.
  - `--path <PATH>`: Path to configuration file (defaults to active workspace config).
- **`config get <KEY>`**: Retrieve active configuration value by dotted key path.
- **`config set <KEY> <VALUE>`**: Set configuration value in user or workspace config file.
- **`config sources`**: Display ordered configuration source precedence and active files.
- **`config explain <KEY>`**: Display resolution provenance, tier, and override chain for a configuration key.

### 10. `telemetry`
Inspect execution traces, spans, and metrics.

- **`telemetry inspect <MISSION_ID>`**: Inspect telemetry records for a mission.
  - `--summary`: Display high-level mission duration, tokens, and cost.
  - `--spans`: Display hierarchical execution spans.
  - `--metrics`: Print sampled metric timeseries.
  - `--export <FORMAT>`: Export telemetry data in `json` or `ndjson` format.

### 11. `eval`
Execute autonomous evaluation harness benchmarks.

- **`eval run`**: Run acceptance evaluation scenarios.
  - `--scenario, -s <ID>`: Target specific scenario by ID (e.g. `a`, `b`, `c`, `d`, `e`, `f`, `g`, `h`).
  - `--all`: Execute all canonical acceptance scenarios in isolated temporary fixtures.
  - `--iterations, -i <N>`: Number of benchmark iterations (default: 1).
  - `--output, -o <FORMAT>`: Output format (`markdown` or `json`).

### 12. `tui`
Launch the full interactive Ratatui cockpit interface.

- **`tui`**: Connects to the local SQLite runtime and presents a conversation-first
  agent session: header with mission/model/lifecycle state, conversation timeline
  with contextual governance cards (discovery, plan review, task review, execution
  authorization, execution, verification, failure), composer, and contextual key
  hints. Requires a terminal supporting raw mode.
- The TUI is a projection of runtime truth: plan acceptance, task acceptance, and
  execution authorization are distinct explicit decisions routed through canonical
  `ApplicationAction`s to the `PreExecutionCoordinator`. Generic tool/policy
  approvals never substitute for governance decisions.
- Navigation has a single authority (`tui::navigation::NavigationRouter`): the
  40-view registry resolves to workspace routes or contextual detail/overlay
  inspectors on the active route. Without the governed runtime bridge, composer
  submissions fail closed with an explicit error instead of executing.

### 13. `init`
Initialize workspace onboarding state (idempotent).

- **`init`**: Reports whether the current workspace is initialized. If
  initialized, exits 0 with no changes. If not, prints guidance to run
  `m31a tui` for the interactive first-run setup — it never fakes completion.
  - `--force`: Explicitly re-enter first-run onboarding for recovery workflows.
    Rewinds both durable authorities (sentinel and SQLite record); completion
    still requires the interactive wizard.

### 14. `version`
Print version information derived from the compiled binary.

- **`version`**: Prints `m31a <version>`.

---

## Workspace Instance Lifecycle

Each workspace (`--workspace <DIR>`, default: current directory) owns a
persistent instance bound to its canonical root with a deterministic instance
id. Startup resolves this instance once (`init::resolve_startup`, the single
startup authority):

1. Resolve workspace root → load or create the persistent instance.
2. If initialized (`Ready`/`Onboarded` in `.m31a/init.json` and the canonical
   SQLite `system_state` record): skip onboarding, launch the normal
   interactive experience directly.
3. If uninitialized: run first-run onboarding exactly once, persist successful
   completion to both authorities, then continue startup.

Every `m31a` invocation is a new process, never a new installation:
repeat runs in the same workspace reuse durable state and never re-launch
setup. A different workspace onboards independently. Corrupt, version-skewed, or
identity-mismatched state aborts fail-closed with an explicit error instead
of silently re-running setup. Configuration (`.m31a/config.toml`) alone never
implies initialization.

---

## Machine-Readable Output

For CI consumers, use `--output json` or `--output stream-json`:

```bash
# JSON output from any command
m31a --output json doctor
m31a --output json mission list
m31a --output json mission run "Fix the failing test"
```

JSON output is written exclusively to stdout. Errors are written to stderr and the process exits with a non-zero code. No ANSI escape codes are emitted in JSON mode.
