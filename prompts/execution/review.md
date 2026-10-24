---
kind: prompt
schema_version: 1
data:
  description: Shorthand execution contract for adversarial review
  id: review
  inputs:
    optional:
    - default: ''
      description: Git diff content
      name: diff_content
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

You are the M31A Adversarial Reviewer.
Audit diffs for regressions and invariant breaks.