---
schema_version: 1
open_count: 2
waived_count: 0
fixed_count: 0
total_count: 2
last_updated: 2026-08-25T10:45:39.715Z
---

# Broken Windows Ledger

> Cross-phase defect register. With `workflow.windows_enforce` enabled, `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 4 | unrun-verify | cmd/m31a |  | In-repo make build / go test ./cmd/m31a/ blocked by pre-existing legacy compile cascade (engine/session, taskrunner, ledger, todo -> workflow -> TUI stack); explain CLI behavior proven via throwaway harness compiling cmd/m31a/explain.go verbatim against real APIs and executing usage/not-found/real-symbol paths | open |  | 2026-08-25T10:45:39.598Z |  |
| 2 | 4 | deviation | .planning/phases/04-intelligence-features/04-02-SUMMARY.md |  | Tracer binary checks substituted with sandbox-harness verification; human confirmation deferred to UAT (coverage D5 human_judgment true) | open |  | 2026-08-25T10:45:39.715Z |  |

````json
[
  {
    "id": 1,
    "kind": "unrun-verify",
    "phase": "4",
    "file": "cmd/m31a",
    "line": null,
    "description": "In-repo make build / go test ./cmd/m31a/ blocked by pre-existing legacy compile cascade (engine/session, taskrunner, ledger, todo -> workflow -> TUI stack); explain CLI behavior proven via throwaway harness compiling cmd/m31a/explain.go verbatim against real APIs and executing usage/not-found/real-symbol paths",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-25T10:45:39.598Z",
    "resolved_at": null
  },
  {
    "id": 2,
    "kind": "deviation",
    "phase": "4",
    "file": ".planning/phases/04-intelligence-features/04-02-SUMMARY.md",
    "line": null,
    "description": "Tracer binary checks substituted with sandbox-harness verification; human confirmation deferred to UAT (coverage D5 human_judgment true)",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-25T10:45:39.715Z",
    "resolved_at": null
  }
]
````
