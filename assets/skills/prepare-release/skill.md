---
kind: skill
schema_version: 1
data:
  description: Prepares release artifacts, updates changelog, and validates commit tags
  execution:
    mode: sub_dag
  id: prepare-release
  name: Prepare Release
  procedure:
    instructions: 1. Validate clean git working tree. 2. Bump versions. 3. Build release. 4. Tag release.
    steps:
    - allowed_tools:
      - git_status
      instruction: Ensure git status is clean
      name: clean_check
    - allowed_tools:
      - verifier
      instruction: Run all verification gates
      name: verify_all
    - allowed_tools:
      - builder
      instruction: Build release artifacts
      name: package
  required_capabilities:
  - workspace_fs_write
  - git_ops
  risk_profile:
    level: high
    requires_approval: true
  schema_version: 1
  verification:
    commands:
    - cargo check --release
    - cargo test --all-targets
    evidence_required:
    - release_metadata.json
    tier: 5
  version: 1.0.0
---

# Prepare Release

1. Validate clean git working tree. 2. Bump versions. 3. Build release. 4. Tag release.
