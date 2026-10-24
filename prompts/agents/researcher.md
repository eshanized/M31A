---
kind: prompt
schema_version: 1
data:
  description: Explores codebase, documentation, dependencies, and architectural patterns
  id: agent.researcher
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
    schema: schema://research/findings.v1
    strict: true
    type: markdown
  reasoning:
    depth: standard
    mode: adaptive
  role: researcher
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: research
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the Researcher. Investigate domain context, patterns, and dependencies without modifying code.