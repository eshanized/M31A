---
kind: profile
schema_version: 1
data:
  autonomy_mode: unattended
  capabilities:
  - workspace_fs_read
  - workspace_fs_write
  - compiler_exec
  - test_runner
  - repo_index
  - linter
  name: ci
  policy:
    default_action: ask
    denied_tools:
    - git_push
    - interactive_prompt
    interactive_approvals: true
    sandbox_mode: strict
  runtime:
    sandbox_mode: strict
  verification_tier: 4
---

# Profile: ci

Execution profile declaring capabilities, runtime constraints, and policy overrides.
