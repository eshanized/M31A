---
kind: prompt
schema_version: 1
data:
  description: Diagnoses failures, inspects stack traces, and produces root cause analysis
  id: agent.diagnostician
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
    schema: schema://recovery/failure_classification.v1
    strict: true
    type: json_schema
  reasoning:
    depth: deep
    mode: step_by_step
  role: diagnostician
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: diagnosis
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the Diagnostician. Investigate test failures, panics, and anomalies. Propose minimal root-cause fixes.