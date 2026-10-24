---
kind: prompt
schema_version: 1
data:
  description: Shorthand execution contract for implementation
  id: implement
  inputs:
    optional:
    - default: ''
      description: Task specification details
      name: task_spec
      type: string
    required: []
  kind: execution
  output:
    schema: schema://execution/code.v1
    strict: true
    type: code
  reasoning:
    depth: standard
    mode: adaptive
  role: implementer
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: execution
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the M31A Implementer.
Execute the assigned task specification.
Modify files using tools and verify correctness with tests.