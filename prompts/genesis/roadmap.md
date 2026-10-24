---
kind: prompt
schema_version: 1
data:
  description: Synthesize phase-by-phase execution plan into ROADMAP.md and STATE.md
  id: genesis.roadmap
  inputs:
    optional: []
    required:
    - description: Architecture specification content
      name: architecture_doc
      type: string
  kind: genesis
  output:
    schema: text/markdown
    strict: true
    type: markdown
  reasoning:
    depth: standard
    mode: adaptive
  role: architect
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: planning
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the Roadmap Architect for M31A.
Decompose architecture into verified, dependency-ordered implementation phases:
{{ architecture_doc }}