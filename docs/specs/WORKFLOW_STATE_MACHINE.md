# Workflow State Machine Specification

**Subsystem:** `m31a-core::workflow`, `m31a-agent-engine::workflow`  
**Version:** 1.0.0 (Foundation Milestone)  
**Status:** Approved & Implemented

---

## 1. Workflow Level State Machine

```
              ┌───────────────┐
              │    Created    │
              └───────┬───────┘
                      │
                      ▼
              ┌───────────────┐
              │   Analyzing   │
              └───────┬───────┘
                      │
                      ▼
              ┌───────────────┐
              │   Planning    │
              └───────┬───────┘
                      │
                      ▼
              ┌───────────────┐
              │   PlanReady   │
              └───────┬───────┘
                      │
         ┌────────────┴────────────┐
         │ (requires approval)     │ (auto / approved)
         ▼                         ▼
┌──────────────────┐       ┌───────────────┐
│ AwaitingApproval │──────>│     Ready     │
└──────────────────┘       └───────┬───────┘
                                   │
                                   ▼
                           ┌───────────────┐
                     ┌────>│    Running    │<────┐ (replan revision)
                     │     └───────┬───────┘     │
                     │             │             │
                     │             ▼             │
                     │     ┌───────────────┐     │
                     │     │   Verifying   │     │
                     │     └───────┬───────┘     │
                     │             │             │
                     │             ▼             │
                     │     ┌───────────────────┐ │
                     │     │IntegrationPending │ │
                     │     └───────┬───────────┘ │
                     │             │             │
                     │             ▼             │
                     │     ┌───────────────┐     │
                     │     │   Completed   │     │ (Terminal)
                     │     └───────────────┘     │
                     │                           │
                     │     ┌───────────────┐     │
                     ├────>│    Failed     │─────┘ (if replan attempts < max)
                     │     └───────────────┘ (Terminal if replan exhausted)
                     │     ┌───────────────┐
                     ├────>│    Blocked    │
                     │     └───────────────┘
                     │     ┌───────────────┐
                     └────>│   Cancelled   │ (Terminal)
                           └───────────────┘
```

### Valid Workflow State Transitions

| Current State | Permitted Next States | Trigger / Condition |
| :--- | :--- | :--- |
| **Created** | `Analyzing`, `Planning`, `Running`, `Cancelled` | Workflow initialized; planning or execution starts. |
| **Analyzing** | `Planning`, `Failed`, `Cancelled` | Static analysis, repository indexing complete. |
| **Planning** | `PlanReady`, `Failed`, `Cancelled` | Plan DAG generated and topologically validated. |
| **PlanReady** | `AwaitingApproval`, `Ready`, `Running`, `Cancelled` | Plan ready for review or immediate dispatch. |
| **AwaitingApproval**| `Ready`, `Running`, `Cancelled` | Human approval granted or rejected. |
| **Ready** | `Running`, `Cancelled` | Workers dispatched to execute first wave. |
| **Running** | `Verifying`, `IntegrationPending`, `Completed`, `Failed`, `Blocked`, `Cancelled` | Wave tasks running; wave checks or task completion. |
| **Verifying** | `Running`, `IntegrationPending`, `Completed`, `Failed`, `Blocked`, `Cancelled` | Multi-stage verification executing. |
| **IntegrationPending**| `Completed`, `Failed`, `Cancelled` | Changes merging into primary branch. |
| **Completed** | *(None)* | **Terminal State:** Workflow completed and verified. |
| **Failed** | `Planning`, `Ready`, `Running` | Replanning attempt within `max_replan_attempts` budget. Otherwise terminal. |
| **Blocked** | `Planning`, `Ready`, `Running`, `Cancelled` | External dependency or policy block resolved. |
| **Cancelled** | *(None)* | **Terminal State:** User or policy cancellation. |

---

## 2. Task Level State Machine

```
               ┌───────────────┐
               │    Pending    │ (waiting on wave or dependencies)
               └───────┬───────┘
                       │ (dependencies satisfied)
                       ▼
               ┌───────────────┐
        ┌─────>│     Ready     │ (eligible for worker lease)
        │      └───────┬───────┘
        │              │ (claim_ready_task)
        │              ▼
        │      ┌───────────────┐
        │      │    Claimed    │ (worker lease active)
        │      └───────┬───────┘
        │              │ (worker starts execution)
        │              ▼
        │      ┌───────────────┐
        │      │    Running    │
        │      └───────┬───────┘
        │              │ (tool run complete)
        │              ▼
        │      ┌───────────────┐
        │      │   Verifying   │
        │      └───────┬───────┘
        │              │
        │        ┌─────┴───────────────────────┐
        │        │ (passed)                    │ (failed verification)
        │        ▼                             ▼
        │ ┌───────────────┐             ┌───────────────┐
        │ │   Completed   │(Terminal)   │ RetryPending  │
        │ └───────────────┘             └───────┬───────┘
        │                                       │
        └───────────────────────────────────────┤ (retry < max_retries)
                                                ▼
                                        ┌───────────────┐
                                        │    Failed     │ (Terminal, retries exhausted)
                                        └───────────────┘
```

### Valid Task State Transitions

| Current State | Permitted Next States |
| :--- | :--- |
| **Pending** | `Ready`, `Blocked`, `Cancelled`, `Failed` |
| **Ready** | `Claimed`, `Running`, `Blocked`, `Cancelled`, `Failed` |
| **Claimed** | `Running`, `Ready`, `RetryPending`, `Failed`, `Cancelled`, `NeedsApproval` |
| **Running** | `Verifying`, `Completed`, `RetryPending`, `Failed`, `Blocked`, `Cancelled`, `NeedsApproval` |
| **Verifying** | `Completed`, `RetryPending`, `Failed`, `Cancelled`, `NeedsApproval` |
| **RetryPending** | `Ready`, `Claimed`, `Running`, `Failed`, `Cancelled` |
| **Blocked** | `Pending`, `Ready`, `Failed`, `Cancelled` |
| **NeedsApproval** | `Ready`, `Running`, `Blocked`, `Cancelled`, `Failed` |
| **Completed** | *(Terminal — no transitions permitted)* |
| **Failed** | *(Terminal — no transitions permitted)* |
| **Cancelled** | *(Terminal — no transitions permitted)* |
