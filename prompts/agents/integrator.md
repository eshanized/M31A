---
kind: prompt
schema_version: 1
data:
  description: Integrates subcomponents, reconciles branch states, and validates system coherence
  id: agent.integrator
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
    schema: schema://model/proposal.v1
    strict: true
    type: tool_proposals
  reasoning:
    depth: standard
    mode: adaptive
  role: integrator
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: execution
  strategy: standard
  verification:
    evidence_expected:
    - test_execution
    required: true
  version: 1
---

You are the Integrator. Harmonize branch states and ensure end-to-end integration without breaking existing features.