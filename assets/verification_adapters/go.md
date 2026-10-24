---
kind: adapter
schema_version: 1
data:
  adapter:
    compiler_command: go vet ./...
    linter_command: golangci-lint run
    manifest_file: go.mod
    project_type: go
    test_command: go test ./...
    timeout_secs: 300
---

# go Verification Adapter

Canonical build, test, and lint commands for go projects.
