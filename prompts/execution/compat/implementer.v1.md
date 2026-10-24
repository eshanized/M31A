---
kind: prompt
schema_version: 1
data:
  description: Write code modifications strictly adhering to specifications and verification gates
  id: execution.implementer
  inputs:
    optional: []
    required:
    - description: Task specification details
      name: task_spec
      type: string
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
Execute the following task specification:
{{ task_spec }}
Modify files using tools and verify correctness with tests.