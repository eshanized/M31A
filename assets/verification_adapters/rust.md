---
kind: adapter
schema_version: 1
data:
  adapter:
    compiler_command: cargo check
    linter_command: cargo clippy -- -D warnings
    manifest_file: Cargo.toml
    project_type: rust
    source_dir: src
    test_command: cargo test
    timeout_secs: 300
---

# rust Verification Adapter

Canonical build, test, and lint commands for rust projects.
