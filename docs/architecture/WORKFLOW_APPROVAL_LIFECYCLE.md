# M31A Workflow Approval & Integration Lifecycle Architecture

**Status:** Authoritative Architectural Specification  
**Scope:** Human Authorization, Patch Integrity, and Workspace Integration  
**Governing Invariant:** No Unapproved or Unverified Changes May Touch the Primary Workspace  

---

## 1. Governing Invariant & Purpose

Autonomous agents produce code changes in isolated environments (git worktrees or sandboxed directories). Before any change is merged or applied to the user's primary workspace, M31A enforces rigorous authorization and verification controls:

> **Governing Invariant:**  
> If integration approval is required (`require_approval_for_integration = true`), no execution path may modify the primary workspace, record a commit, or mark an integration task as completed without an explicit, durable, and verifiable human approval decision corresponding exactly to the change-set hash of the proposed patch.

There is strictly no automatic approval bypass. Synthetic substitutes such as:
```rust
// FORBIDDEN IN M31A
is_approved: !config.require_approval_for_integration
```
have been completely eliminated from the codebase.

---

## 2. Elimination of Duplicated Integration Paths

Prior to Round 3, three disparate code paths existed for integrating code into the primary workspace:
1. `EngineAgentRuntime::integrate_agent` (performed ad-hoc file copying, git commits, and isolated testing).
2. `WorkflowTaskKind::Integration` handling in `WorkflowOrchestrator::execute_task`.
3. `SpeculativeArena` promotion in speculative workflows.

In Round 3, all integration paths were unified into a single authoritative component: **`WorkflowIntegrationService`** (in `crates/m31a-agent-engine/src/integration.rs`).

```
┌──────────────────────────────┐ ┌──────────────────────────────┐ ┌──────────────────────────────┐
│  EngineAgentRuntime          │ │   WorkflowOrchestrator       │ │      SpeculativeArena        │
│  (Isolated Agent Worktree)   │ │   (Plan Integration Task)    │ │   (Hypothesis Winner)        │
└──────────────┬───────────────┘ └──────────────┬───────────────┘ └──────────────┬───────────────┘
               │                                │                                │
               │ Delegates Diff                 │ Invokes Task                   │ Promotes Winner
               ▼                                ▼                                ▼
┌────────────────────────────────────────────────────────────────────────────────────────────────┐
│                         AUTHORITATIVE WorkflowIntegrationService                               │
│                                                                                                │
│  1. Primary Workspace Lock (Mutex + Lease)                                                     │
│  2. Path Containment & Symlink Validation                                                      │
│  3. Canonical Change-Set SHA-256 Calculation                                                   │
│  4. Durable Approval Verification (WorkflowStore)                                              │
│  5. Review Engine Pre-Check (m31a-review)                                                      │
│  6. Clean Workspace Verification (Dirty Check)                                                 │
│  7. Pre-Integration Per-File Snapshots                                                         │
│  8. Unified Patch Application                                                                  │
│  9. Autonomous Build, Test, Clippy, and Safety Verification                                    │
│ 10. Atomic Rollback on Any Verification Failure                                                │
│ 11. Git Attribution Commit (Author/Email Preserved)                                            │
│ 12. Integration Outcome & Event Persistence                                                    │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`EngineAgentRuntime::integrate_agent` is now a thin adapter that extracts the worktree diff and delegates directly to `WorkflowIntegrationService::integrate`.

---

## 3. Durable Approval State Machine

### 3.1 State Diagram

The lifecycle of an integration request follows this deterministic state machine:

```
                      ┌──────────────────────┐
                      │  IntegrationPending  │
                      └──────────┬───────────┘
                                 │
                                 ▼
                      ┌──────────────────────┐
                      │   AwaitingApproval   │
                      └────┬───────┬───────┬─┘
                           │       │       │
              Operator     │       │       │  Operator
              Approves     │       │       │  Rejects
                           ▼       │       ▼
                    ┌──────────┐   │   ┌──────────┐
                    │ Approved │   │   │ Rejected │
                    └────┬─────┘   │   └──────────┘
                         │         │
                         │         │ Workflow Cancelled
                         ▼         ▼
                    ┌──────────┐ ┌───────────┐
                    │Integrating│ │ Cancelled │
                    └────┬─────┘ └───────────┘
                         │
             ┌───────────┴───────────┐
             │                       │
      Success│                Failure│
             ▼                       ▼
      ┌────────────┐        ┌───────────────────┐
      │ Integrated │        │ IntegrationFailed │
      └────────────┘        └───────────────────┘
```

### 3.2 State Definitions

| State | Description | Next Permitted States |
| :--- | :--- | :--- |
| **`IntegrationPending`** | Integration request formulated; preflight validation in progress. | `AwaitingApproval`, `Integrating` (if approval not required), `Cancelled` |
| **`AwaitingApproval`** | Change set calculated; waiting for operator authorization via RPC/CLI/TUI. | `Approved`, `Rejected`, `Cancelled` |
| **`Approved`** | Operator approved the exact change-set hash for this attempt ID. | `Integrating`, `Cancelled` |
| **`Rejected`** | Operator denied the integration request with a documented reason. | *Terminal* |
| **`Cancelled`** | Workflow or task cancelled while awaiting approval or during integration. | *Terminal* |
| **`Integrating`** | Patch is being applied, verified, and committed to primary workspace. | `Integrated`, `IntegrationFailed`, `Cancelled` |
| **`Integrated`** | Patch applied, verified by test suite, committed to git, and evidence saved. | *Terminal Success* |
| **`IntegrationFailed`**| Patch application, verification, or commit failed; exact rollback completed.| *Terminal Failure* |

---

## 4. Durable Storage Schema (Migration 17)

Approval state is stored in SQLite within the `workflow_approvals` table, added in Storage Migration 17 (`crates/m31a-storage/src/migrations.rs`):

```sql
CREATE TABLE IF NOT EXISTS workflow_approvals (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    attempt_id INTEGER NOT NULL,
    change_set_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    requested_at INTEGER NOT NULL,
    decided_at INTEGER,
    decision_by TEXT,
    reason TEXT,
    diff_content TEXT,
    FOREIGN KEY(workflow_id) REFERENCES workflows(id) ON DELETE CASCADE,
    FOREIGN KEY(task_id) REFERENCES workflow_tasks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_workflow_approvals_workflow 
    ON workflow_approvals(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_approvals_task 
    ON workflow_approvals(task_id, attempt_id);
CREATE INDEX IF NOT EXISTS idx_workflow_approvals_hash 
    ON workflow_approvals(change_set_hash);
CREATE INDEX IF NOT EXISTS idx_workflow_approvals_status 
    ON workflow_approvals(status);
```

### Storage Interface Methods (`WorkflowStore`):
- `create_approval_request(req: &WorkflowApprovalRequest) -> StorageResult<()>`
- `record_approval_decision(id, status, decision_by, reason, decided_at) -> StorageResult<()>`
- `get_approval_request(id: &str) -> StorageResult<Option<WorkflowApprovalRequest>>`
- `list_approvals(workflow_id, status_filter) -> StorageResult<Vec<WorkflowApprovalRequest>>`
- `is_patch_approved(task_id, attempt_id, change_set_hash) -> StorageResult<bool>`

---

## 5. Change-Set Hash Verification

To guarantee that the exact code reviewed and approved is the code applied to the primary workspace:

1. **Canonical Change-Set Calculation:**
   For all files in the proposed patch, relative paths are canonicalized and sorted lexicographically.
2. **Cryptographic Hashing:**
   A SHA-256 digest is computed over the concatenation of:
   ```
   SHA256( canonical_path_1 + "\0" + file_content_1 + "\0" + canonical_path_2 + ... )
   ```
3. **Approval Binding:**
   The resulting hex digest is recorded in `WorkflowApprovalRequest.change_set_hash`.
4. **Integration Pre-Check:**
   Before modifying any file on disk, `WorkflowIntegrationService` recalculates the change-set hash of the incoming diff and verifies:
   ```rust
   let is_approved = store.is_patch_approved(&task_id, attempt_id, &calculated_hash)?;
   if !is_approved {
       return Err(IntegrationError::ApprovalRequired(
           "Patch is not approved for this attempt and change-set hash".into()
       ));
   }
   ```
If an agent mutates the diff or a stale attempt presents an old approval, the hash check fails closed.

---

## 6. Preflight Containment & Atomic Rollback

### 6.1 Containment & Path Traversal Prevention
Before writing any file, paths are resolved against the primary workspace root using `canonicalize()` or path prefix checks:
- Any path containing `..` escaping the workspace root is rejected with `IntegrationError::SecurityViolation`.
- Any path traversing a symlink pointing outside the workspace root is rejected.

### 6.2 Atomic Snapshotting & Exact Rollback
Before applying any patch:
1. Every file targeted for modification or deletion is read into memory or copied to `.m31a/snapshots/`.
2. Targeted new files are recorded as "newly created".
3. If patch application, build verification, test verification, or git commit fails:
   - Modified files are restored to their original bytes.
   - Newly created files are deleted.
   - Deleted files are restored.
   - Rollback execution validates that every restored file matches its pre-integration hash.

---

## 7. Verification & Review Engine Gates

Even if a human operator approves an integration, M31A's governing invariant prohibits integrating broken or insecure code.

1. **Review Engine Gate (`m31a-review`):**
   Prior to applying the patch, the review engine scans the proposed diff for security vulnerabilities, secrets, and policy violations. If any `Critical` finding is detected, integration is blocked immediately.
2. **Autonomous Verification Gate (`m31a-verify`):**
   After applying the patch into the primary workspace, automated verification commands execute:
   - `cargo check` / `cargo build`
   - `cargo test`
   - `cargo clippy -- -D warnings`
   - Safety scans
3. **Unsupported Manifest Fail-Closed:**
   If a repository does not contain a recognized build manifest (`Cargo.toml`, `package.json`, `pyproject.toml`, etc.), automated verification cannot establish correctness. The service fails closed with `IntegrationError::VerificationFailed("Unsupported project manifest")` and rolls back all changes.

---

## 8. Git Commit Handling & Attribution

When operating in a Git repository:
1. The author and email are determined from the task execution context or git config.
2. Changes are staged (`git add -A`).
3. Commit is executed via `git commit -m "..." --author="..."`.
4. **No Silent Commit Failures:** If `git commit` exits with a non-zero code, `WorkflowIntegrationService` does **not** treat it as a success. It initiates rollback, records `IntegrationOutcome::IntegrationFailed`, and returns an explicit error.
5. The resulting commit SHA is persisted into `WorkflowTaskExecutionResult.commit_sha` and recorded in the audit event log.

---

## 9. Operator Supervision Interfaces

Operators inspect and decide on approval requests across three interfaces:

### 9.1 App Server JSON-RPC Endpoints
- **`workflow/approvals`:**
  - Request: `{ "workflow_id": "wf-123", "status": "awaiting_approval" }`
  - Response: `{ "approvals": [ { "id": "appr-1", "task_id": "task-4", "change_set_hash": "...", ... } ] }`
- **`workflow/approve`:**
  - Request: `{ "approval_id": "appr-1", "reason": "LGTM, verified sandbox tests", "decision_by": "operator" }`
  - Response: `{ "status": "approved", "approval_id": "appr-1" }`
- **`workflow/reject`:**
  - Request: `{ "approval_id": "appr-1", "reason": "Requires refactoring", "decision_by": "operator" }`
  - Response: `{ "status": "rejected", "approval_id": "appr-1" }`

### 9.2 CLI Commands
```bash
# List pending approvals
m31a workflow approvals --workflow-id <WORKFLOW_ID>

# Approve an integration request
m31a workflow approve <APPROVAL_ID> --reason "Approved after code review"

# Reject an integration request
m31a workflow reject <APPROVAL_ID> --reason "Contains unauthorized dependency"
```

### 9.3 Interactive TUI Terminal
In `m31a-tui`:
- The Workflow & Agents screen displays active tasks with a visual pointer `▶`.
- Tasks in `AwaitingApproval` status are highlighted in yellow.
- Operator hotkeys:
  - `a`: Authorize / Approve selected integration.
  - `r`: Reject selected integration.
  - `c`: Cancel active workflow.
  - `u`: Refresh status.
  - `Enter` / `i`: Inspect task details and proposed diff.
