# M31A Autonomous Repository Maintenance Architecture

## 1. System Vision & Objective

The **M31A Autonomous Repository Maintenance Engine** (`m31a-maintenance`) evolves M31A from reactive agentic code generation into a continuous, policy-governed repository maintenance subsystem. It operates autonomously to identify maintenance opportunities, evaluate operational impact, formulate bounded plans, execute verified remediations in isolated git worktrees, independently verify results via Initiative 2, and produce reviewable pull requests.

### Core Product Promise
1. **Evidence-Driven**: Every maintenance action is grounded in concrete observations (compiler diagnostics, AST audits, dependency semver diffs, CI run logs, security advisories).
2. **Zero Transient Conversations**: All maintenance state is persistently tracked across 6 SQLite tables with strict state-machine lifecycles.
3. **No Unfenced Code Mutations**: Active developer workspaces are never directly modified by background maintenance. All mutations execute in isolated ephemeral worktrees and require verified pass proofs.
4. **Deterministic Gatekeeping**: High-risk changes (major semver upgrades, sensitive CVEs, cross-crate API shifts) require explicit human approval via the `ExternalEffectGateway`.

---

## 2. Component Architecture

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                   SIGNAL INTAKE                                        │
│  ┌──────────────────────────────┐                 ┌─────────────────────────────────┐  │
│  │     GitHub Webhook Adapter   │                 │ Local AST / Repo Signal Source  │  │
│  │ (HMAC-SHA256, Size Limit,    │                 │ (Diagnostics, Manifest Audits,  │  │
│  │  Replay Protection)          │                 │  Doc Sync, Architecture Smells) │  │
│  └──────────────┬───────────────┘                 └────────────────┬────────────────┘  │
└─────────────────┼──────────────────────────────────────────────────┼───────────────────┘
                  │                                                  │
                  ▼                                                  ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                             SIGNAL INTAKE MANAGER                                      │
│  • Deduplication Fingerprint Computation (`SHA-256(repo_id + provider + coords)`)     │
│  • Idempotent Deduplication vs Open Case Grouping                                      │
│  • Ingestion Outcome: Duplicate | CaseUpdated | CaseCreated                            │
└────────────────────────────────────────┬───────────────────────────────────────────────┘
                                         │
                                         ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                              DETERMINISTIC TRIAGE ENGINE                               │
│  • Multi-Factor Composite Scoring:                                                     │
│    composite = 0.35*Severity + 0.30*Impact + 0.15*BlastRadius + 0.10*Conf + 0.10*Feas  │
│  • Priority Assignment: P1 (Critical) to P4 (Low)                                      │
│  • Autonomy Tier Recommendation (Tier 0 Observe -> Tier 3 Restricted High Impact)       │
└────────────────────────────────────────┬───────────────────────────────────────────────┘
                                         │
                                         ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                             MAINTENANCE PLANNING ENGINE                                │
│  Strategy Registry:                                                                    │
│  • BrokenCiStrategy: Distinguishes code compilation/tests from CI infrastructure      │
│    outages. Zero code modifications proposed for runner network timeouts.              │
│  • DependencyUpdateStrategy: Automatic Tier 2 for safe semver patch/minor updates;    │
│    Tier 3 Pending Approval for major semver breaking migrations.                       │
│  • IssueBugFixStrategy: Strictly requires reproduction steps; returns NeedsInformation │
│    without hallucinating fixes if unreproducible.                                      │
│  • SecurityAdvisoryStrategy: Preserves CVE provenance, protects sensitive exploits.   │
└────────────────────────────────────────┬───────────────────────────────────────────────┘
                                         │
                                         ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                       WORKFLOW ORCHESTRATION & EXECUTION                               │
│  • Translates MaintenancePlan into authoritative DAG via `WorkflowOrchestrator`:       │
│    Wave 0: Analysis Task (ReadOnlyCopy)                                                │
│    Wave 1: Implementation Task (Isolated Git Worktree)                                 │
│    Wave 2: Independent Verification Task (Initiative 2 Contract)                        │
│    Wave 3: Delivery Preparation Task                                                   │
└────────────────────────────────────────┬───────────────────────────────────────────────┘
                                         │
                                         ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                          INITIATIVE 2 VERIFIED INTEGRATION                             │
│  • `IndependentVerificationEngine` executes isolated build, test, and audit suites     │
│  • Candidate ChangeSet frozen into cryptographically verifiable `ChangeSetManifest`    │
│  • `ExternalEffectGateway` enforces worker lease fencing tokens (rejection on stale)   │
└────────────────────────────────────────┬───────────────────────────────────────────────┘
                                         │
                                         ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                          DELIVERY & OBSERVABILITY ADAPTER                              │
│  • Synthesizes reviewable Draft Pull Requests with frozen ChangeSet manifest hash,     │
│    root cause hypothesis, and verification pass proof.                                 │
│  • Comprehensive Structured Audit Trail logging for every lifecycle transition.       │
│  • Exposed via CLI (`m31a maintenance`), JSON-RPC (`maintenance/*`), and TUI.         │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Subsystem Boundaries & Interactions

### 3.1 Interaction with Vibe Loop
`VibeEngine` provides low-latency local developer feedback. When fast self-healing passes, local workspace changes are committed safely. When Vibe exhausts retry attempts without resolving a compile error, it emits `VibeEvent::MaintenanceSignalAvailable`. This escalates the failure into the persistent maintenance engine, preventing infinite loops and ensuring high-effort analysis happens in an isolated worktree rather than disrupting the active user workspace.

### 3.2 Interaction with Initiative 2 Verified Execution
All code edits executed during maintenance pass through the Initiative 2 contract:
- Code changes take place on temporary git worktrees (`.m31a/worktrees/maint-<id>`).
- Changes are frozen into a `ChangeSetManifest`.
- `IndependentVerificationEngine` executes declared requirements independently of the generating agent.
- `ExternalEffectGateway` guarantees only the current leaseholder token can dispatch external effects (PR creation, issue commenting). Stale attempts are rejected with `FencingDenied`.

### 3.3 State Machine Lifecycles
```
[Open] ──> [Triaged] ──> [Investigating] ──> [Planning] ──> [PlanReady]
                                                                  │
              ┌───────────────────────────────────────────────────┘
              ▼
    [AwaitingApproval] (if Tier 3 or high risk) ──> [Executing]
                                                        │
                                                        ▼
                                                   [Verifying]
                                                        │
                                                        ▼
                                                 [ReviewPending]
                                                        │
                                                        ▼
                                                   [Resolved]
```
Terminal or exception states include `Failed`, `Blocked`, `NeedsInformation`, and `Cancelled`. A resolved case reopens to `Open` if a new signal on the same dedup key arrives.
