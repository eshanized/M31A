---
kind: adapter
schema_version: 1
data:
  adapter:
    compiler_command: ''
    linter_command: ruff check
    manifest_file: pyproject.toml
    project_type: python
    test_command: pytest
    timeout_secs: 300
---

# python Verification Adapter

Canonical build, test, and lint commands for python projects.
