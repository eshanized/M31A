---
kind: prompt
schema_version: 1
data:
  description: Independently verify acceptance criteria and test passes
  id: execution.verifier
  inputs:
    optional: []
    required:
    - description: Acceptance criteria and test objectives to verify
      name: acceptance_criteria
      type: string
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
Verify the following criteria:
{{ acceptance_criteria }}
Run compilation, test suites, and linters. Collect empirical evidence.