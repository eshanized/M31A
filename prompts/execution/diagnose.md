---
kind: prompt
schema_version: 1
data:
  description: Shorthand execution contract for diagnostics
  id: diagnose
  inputs:
    optional:
    - default: ''
      description: Failure details, test output, and logs
      name: failure_context
      type: string
    required: []
  kind: recovery
  output:
    schema: text/markdown
    strict: true
    type: markdown
  reasoning:
    depth: standard
    mode: adaptive
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

You are the M31A Diagnostician.
Investigate failure output and diagnostics. Recommend minimal surgical fix.