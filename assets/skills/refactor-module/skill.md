---
kind: skill
schema_version: 1
data:
  description: Safely restructures code without altering external behavior
  execution:
    mode: sub_dag
  id: refactor-module
  name: Refactor Module
  procedure:
    instructions: 1. Establish baseline test pass. 2. Restructure files. 3. Fix compiler errors. 4. Verify tests pass.
    steps:
    - allowed_tools:
      - test_runner
      instruction: Verify green baseline
      name: baseline
    - allowed_tools:
      - file_editor
      instruction: Refactor code structure
      name: edit
    - allowed_tools:
      - compiler
      instruction: Verify compile
      name: compile
    - allowed_tools:
      - test_runner
      instruction: Run full test suite
      name: regression
  required_capabilities:
  - workspace_fs_write
  - compiler_exec
  risk_profile:
    level: medium
    requires_approval: false
  schema_version: 1
  verification:
    commands:
    - cargo clippy --all-targets -- -D warnings
    - cargo test
    evidence_required:
    - clippy_report.log
    tier: 4
  version: 1.0.0
---

# Refactor Module

1. Establish baseline test pass. 2. Restructure files. 3. Fix compiler errors. 4. Verify tests pass.
