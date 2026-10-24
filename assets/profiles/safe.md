---
kind: profile
schema_version: 1
data:
  autonomy_mode: safe
  capabilities:
  - workspace_fs_read
  - repo_index
  name: safe
  policy:
    default_action: ask
    denied_tools:
    - shell_exec
    - terminal_write
    - git_push
    interactive_approvals: true
    sandbox_mode: strict
  runtime:
    concurrency_limit: 2
    sandbox_mode: strict
  verification_tier: 1
---

# Profile: safe

Execution profile declaring capabilities, runtime constraints, and policy overrides.
