# Maintenance Policy & Autonomy Governance

## 1. Autonomy Tiers

M31A implements a four-tier autonomy model to balance developer productivity with production safety:

| Tier | Name | Permissions | Typical Use Cases |
|---|---|---|---|
| **Tier 0** | `tier0_observe` | Read-only observation, telemetry collection, issue/CI intake, triage report generation. Zero files modified. | Initial onboarding, audit-only environments, CI runner outage monitoring. |
| **Tier 1** | `tier1_prepare` | Read-only investigation, plan generation, test reproduction authoring, branch creation. No code remediation applied. | Complex bug reports, ambiguous documentation drift, unreproducible issues. |
| **Tier 2** | `tier2_guarded_remediation` | Full autonomous synthesis and verified execution in isolated git worktrees. Automated pull request preparation. | Broken CI compilation fixes, patch/minor semver dependency updates, doc syncs. |
| **Tier 3** | `tier3_restricted_high_impact` | High-risk remediation strictly gated by human approval. Requires explicit cryptographic approval binding. | Major semver upgrades, critical CVE mitigations, wide architectural refactoring. |

---

## 2. Policy File Format (`.m31a/maintenance.toml`)

The authoritative configuration file is stored in `.m31a/maintenance.toml`.

### Validation Rules
1. **Plaintext Credential Prohibition**: Plaintext tokens (e.g. `github_token = "ghp_..."`) are strictly prohibited and will cause policy loading to fail closed with a fatal security error. Secrets must be referenced via environment variable names (`github_token_env = "GITHUB_TOKEN"`).
2. **Branch Protection**: Autonomous changes are restricted to explicitly allowlisted branches (default: `master`, `main`).
3. **Excluded Paths**: Sensitive directories (`.git/**`, `.github/workflows/**`, `deploy/**`) cannot be modified by autonomous maintenance workflows under any circumstance.
4. **Risk Gatekeeping**: Changes exceeding `risk_threshold_for_approval` (default: `medium`) pause at `AwaitingApproval` until approved via CLI or JSON-RPC.

---

## 3. Human Approval Gatekeeping Protocol

When a plan requires human approval:
1. The case status transitions to `CaseStatus::AwaitingApproval`.
2. A cryptographic audit trail entry is recorded with actor and timestamp.
3. The user inspects the plan via `m31a maintenance inspect <case_id>`.
4. Approval is granted via `m31a maintenance approve <case_id>` or rejected via `m31a maintenance cancel <case_id>`.
5. Only once approval is bound in persistent storage can the `MaintenanceWorkflowRunner` proceed to dispatch execution tasks.
