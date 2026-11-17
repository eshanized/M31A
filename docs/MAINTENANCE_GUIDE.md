# M31A Autonomous Repository Maintenance Guide

Welcome to the **M31A Autonomous Repository Maintenance Engine**. This guide explains how to configure, operate, monitor, and approve autonomous maintenance workflows.

---

## 1. Quick Start

### 1.1 Configuration Setup
Initialize your repository maintenance configuration:
```bash
cp .m31a/maintenance.toml.example .m31a/maintenance.toml
```
Review the settings in `.m31a/maintenance.toml`. By default, M31A runs in `tier2_guarded_remediation` mode, safely executing isolated compilation fixes and dependency updates in isolated git worktrees.

### 1.2 Inspect Current Maintenance Status
```bash
m31a maintenance status
```
Output summarizes active cases, configured schedules, and policy rules.

### 1.3 Trigger a Local AST Scan
```bash
m31a maintenance scan
```
M31A analyzes your workspace for compiler diagnostics, dependency drift, doc drift, and architectural complexity smells, grouping any findings into persistent cases.

---

## 2. CLI Command Reference

The `m31a maintenance` command group provides end-to-end operational control:

| Command | Description |
|---|---|
| `m31a maintenance status` | Displays repository health overview, active cases, and policy settings |
| `m31a maintenance scan` | Executes AST compiler, dependency, doc, and complexity scans |
| `m31a maintenance cases` | Lists persistent maintenance cases with priority and status filters |
| `m31a maintenance inspect <case_id>` | Detailed view of case hypotheses, plans, and execution history |
| `m31a maintenance triage <case_id>` | Evaluates multi-factor composite priority score and autonomy tier |
| `m31a maintenance plan <case_id>` | Formulates an evidence-based maintenance plan with verification requirements |
| `m31a maintenance run <case_id>` | Launches execution of an approved maintenance plan in an isolated worktree |
| `m31a maintenance approve <case_id>` | Grants explicit human approval for Tier 3 or high-risk plans |
| `m31a maintenance cancel <case_id>` | Cancels an ongoing maintenance workflow and cleans up worktrees |
| `m31a maintenance schedules` | Lists configured recurring maintenance jobs (daily health, weekly deps) |
| `m31a maintenance pause <schedule_id>` | Pauses an automated recurring schedule |
| `m31a maintenance resume <schedule_id>`| Resumes an automated recurring schedule |
| `m31a maintenance history <case_id>` | Displays full remediation attempt history and verification hashes |
| `m31a maintenance report <case_id>` | Formats the pull request deliverable markdown report |

---

## 3. TUI Maintenance Experience

Within the M31A TUI:
- Type `/maintenance` or `/maint` into the composer prompt to view the status overview.
- Use `/maintenance scan` to launch an asynchronous background repository audit.
- Use `/maintenance cases` to review open cases.
- Use `/maintenance schedules` to inspect recurring background cron jobs.
- Switch to the **Diagnostics Screen** (`Screen 0`) to view live telemetry cards for the Autonomous Maintenance Engine.

---

## 4. JSON-RPC 2.0 API Reference

The M31A App Server exposes maintenance operations for IDEs, extensions, and automated CI pipelines:

- `maintenance/status`: Returns repository maintenance health summary.
- `maintenance/scan`: Triggers non-destructive AST and toolchain scan.
- `maintenance/cases/list`: Lists cases with optional `status` filter.
- `maintenance/cases/get`: Returns case details and active plan.
- `maintenance/triage`: Evaluates multi-factor triage scoring.
- `maintenance/plan`: Generates an evidence-backed plan.
- `maintenance/run`: Launches execution in an isolated worktree.
- `maintenance/approve`: Approves a pending plan.
- `maintenance/cancel`: Cancels execution.
- `maintenance/schedules/list`: Lists active schedules.
