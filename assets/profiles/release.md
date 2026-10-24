---
kind: profile
schema_version: 1
data:
  autonomy_mode: assisted
  capabilities:
  - workspace_fs_read
  - workspace_fs_write
  - compiler_exec
  - test_runner
  - repo_index
  - git_ops
  - packager
  name: release
  policy:
    default_action: ask
    interactive_approvals: true
    sandbox_mode: standard
  runtime:
    sandbox_mode: standard
  verification_tier: 5
---

# Profile: release

Execution profile declaring capabilities, runtime constraints, and policy overrides.
