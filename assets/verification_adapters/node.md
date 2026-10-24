---
kind: adapter
schema_version: 1
data:
  adapter:
    compiler_command: npm run build
    linter_command: npm run lint
    manifest_file: package.json
    project_type: node
    test_command: npm test
    timeout_secs: 300
---

# node Verification Adapter

Canonical build, test, and lint commands for node projects.
