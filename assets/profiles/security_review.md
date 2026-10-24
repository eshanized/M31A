---
kind: profile
schema_version: 1
data:
  autonomy_mode: safe
  capabilities:
  - workspace_fs_read
  - repo_index
  - sast_linter
  - secret_scanner
  - symbol_graph
  name: security_review
  policy:
    default_action: ask
    denied_tools:
    - workspace_fs_write
    - shell_exec
    - git_commit
    - git_push
    interactive_approvals: true
    sandbox_mode: strict
  runtime:
    sandbox_mode: strict
  verification_tier: 6
---

# Profile: security_review

Execution profile declaring capabilities, runtime constraints, and policy overrides.
