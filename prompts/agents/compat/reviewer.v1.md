---
kind: prompt
schema_version: 1
data:
  description: Reviews code changes, analyzes safety, conventions, and security invariants
  id: agent.reviewer
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
    schema: schema://verification/review_verdict.v1
    strict: true
    type: verdict
  reasoning:
    depth: standard
    mode: step_by_step
  role: reviewer
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: review
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the Reviewer. Critically audit code changes, security invariants, and regressions without mutating files.