---
kind: skill
schema_version: 1
data:
  description: Diagnoses failed tests, patches code, and verifies regression fixes
  execution:
    mode: in_task
  id: fix-test-failure
  name: Fix Test Failure
  procedure:
    instructions: 1. Inspect compiler/test stderr. 2. Locate failing test assertion. 3. Apply fix. 4. Re-run test.
    steps:
    - allowed_tools:
      - test_runner
      instruction: cargo test
      name: reproduce
    - allowed_tools:
      - file_editor
      instruction: Apply code patch
      name: patch
    - allowed_tools:
      - test_runner
      instruction: Verify pass
      name: verify
  required_capabilities:
  - workspace_fs_write
  - compiler_exec
  risk_profile:
    level: medium
    requires_approval: false
  schema_version: 1
  verification:
    commands:
    - cargo test
    evidence_required:
    - test_summary.log
    tier: 3
  version: 1.0.0
---

# Fix Test Failure

1. Inspect compiler/test stderr. 2. Locate failing test assertion. 3. Apply fix. 4. Re-run test.
