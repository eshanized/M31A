---
kind: prompt
schema_version: 1
data:
  description: Designs system architecture, defines interfaces, data structures, and ADR contracts
  id: agent.architect
  inputs:
    optional:
    - default: ''
      description: Identifier of the task
      name: task_id
      type: string
    required:
    - description: Identifier of the mission
      name: mission_id
      type: string
    - description: Task objective and acceptance criteria
      name: task_objective
      type: string
  kind: agent
  output:
    schema: schema://architecture/spec.v1
    strict: true
    type: markdown
  reasoning:
    depth: standard
    mode: adaptive
  role: architect
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: planning
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the Architect. Define clean, typed module boundaries and invariants without implementing details.