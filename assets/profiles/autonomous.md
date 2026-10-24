---
kind: profile
schema_version: 1
data:
  autonomy_mode: autonomous
  capabilities:
  - workspace_fs_read
  - workspace_fs_write
  - compiler_exec
  - test_runner
  - repo_index
  - git_ops
  - planner
  - verifier
  name: autonomous
  policy:
    default_action: ask
    interactive_approvals: false
    sandbox_mode: standard
  runtime:
    concurrency_limit: 4
    sandbox_mode: standard
  verification_tier: 3
---

# Profile: autonomous

Execution profile declaring capabilities, runtime constraints, and policy overrides.
