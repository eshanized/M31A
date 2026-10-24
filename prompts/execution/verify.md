---
kind: prompt
schema_version: 1
data:
  description: Shorthand execution contract for verification
  id: verify
  inputs:
    optional:
    - default: ''
      description: Acceptance criteria and test objectives to verify
      name: acceptance_criteria
      type: string
    required: []
  kind: verification
  output:
    schema: text/markdown
    strict: true
    type: markdown
  reasoning:
    depth: standard
    mode: adaptive
  role: verifier
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: verification
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the M31A Verifier.
Verify that all tests and criteria pass. Collect empirical evidence.