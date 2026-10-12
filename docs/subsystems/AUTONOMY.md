# M31A Autonomy Modes & Execution Loop

This document specifies the autonomy governance model, the 5 autonomy modes, loop stages, sliding-window oscillation detection, and unconstrained execution governed by verification and cancellation in M31 Autonomous (M31A).

## The 5 Autonomy Modes

M31A supports 5 distinct operational autonomy modes defined in `src/state/intake.rs`:

| Autonomy Mode | Mutating Actions Permitted? | Approval Behavior (`ASK`) | Primary Use Case |
|:---|:---|:---|:---|
| **`Plan`** | No (Strictly Read-Only) | Denied immediately | Architecture planning, DAG generation, codebase inspection. |
| **`Safe`** | Requires Approval | Pauses mission; awaits operator confirmation | Sensitive repositories, production codebases, unfamiliar tasks. |
| **`Assisted`** | Allowed inside Workspace | Pauses mission only for external side effects (git push, network) | Standard interactive developer pairing. |
| **`Autonomous`** | Allowed inside Workspace | Pauses mission only if explicit policy veto triggers `ASK` | Self-directed refactoring, bug fixes, feature implementation. |
| **`Unattended`** | Allowed inside Workspace | **Converts `ASK` to `DENY` fail-closed** (never blocks on stdin) | Headless CI/CD runners, scheduled nightly evaluations. |

---

## Budget System Decommissioned (Independent Safeguards Preserved)

The central budget subsystem (admission ledgers, grants, receipts, and monetary/token budget ceilings) has been decommissioned. Instead of central budget-gated admission, execution safety is enforced through independent, deterministic safeguards: OS process confinement, streaming artifact quotas, per-agent turn limit ceilings (loop prevention), sliding-window oscillation detection, circuit breakers, and policy authorization gates.

### Architectural Invariants Post-Decommissioning

1. **No Budget-Gated Admission**: Normal task execution, model calls, and agent scheduling proceed without central budget grants, reservation receipts, or settlement checks. The application never pauses, blocks, or halts for budget replenishment.
2. **Independent Security Boundaries**: Resource protection is maintained strictly through independent security boundaries:
   - OS-level process confinement (POSIX `RLIMIT_CPU`, `RLIMIT_AS`, `RLIMIT_NPROC`, cgroups v2, Windows Job Objects) via `crate::platform::resources::ResourceBudget`.
   - Streaming artifact disk quotas (`QuotaEnforcer`).
   - Operation timeouts (`timeout_secs`, `verification_secs`, `workflow_step_secs`).
   - Policy authorization gates and human approval requirements.
   - Process supervision, cancellation tokens, and process-tree cleanup.
3. **Loop Detection & Life-Cycle Safeguards**: Runaway loops are prevented by per-agent turn limit ceilings and sliding-window oscillation detection.
4. **Passive Usage Telemetry**: Model tokens, tool invocations, and cost estimates are captured as factual observability telemetry, completely decoupled from admission decisions.

---

## Sliding-Window Loop & Oscillation Detection

Agents occasionally get stuck in repetitive action cycles (e.g. repeatedly editing the same file or oscillating between two failing fixes). M31A incorporates an active `LoopDetector`:

- **Sliding Action Window**: Tracks the sequence of normalized tool invocations, argument hashes, and error messages.
- **Oscillation Detection**: Detects periodic repeating sub-sequences (period 1: immediate retry loops; period 2: ping-pong oscillating actions).
- **8-Step Escalation Ladder**:
  1. `Observe`: Record pattern in telemetry.
  2. `Warn`: Inject diagnostic guidance into prompt context.
  3. `Classify`: Route failure to `Diagnostician` agent.
  4. `ForceReplan`: Invalidate current task plan and trigger differential replanning.
  5. `SwitchModel`: Escalate to a larger or more capable reasoning model.
  6. `Pause`: Pause execution for human review in interactive modes.
  7. `Halt`: Abort mission fail-closed in unattended mode.
  8. `Audit`: Persist complete loop diagnostic evidence to SQLite.
