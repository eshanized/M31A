---
kind: profile
schema_version: 1
data:
  autonomy_mode: safe
  capabilities:
  - workspace_fs_read
  - repo_index
  - symbol_graph
  - web_docs
  name: research
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
  verification_tier: 1
---

# Profile: research

Execution profile declaring capabilities, runtime constraints, and policy overrides.
