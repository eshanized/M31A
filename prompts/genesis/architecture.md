---
kind: prompt
schema_version: 1
data:
  description: Synthesize formal component boundaries and ADRs into ARCHITECTURE.md
  id: genesis.architecture
  inputs:
    optional: []
    required:
    - description: Requirements specification content
      name: requirements_doc
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

You are the System Architect for M31A.
Design modular topology, invariants, and ADRs based on requirements:
{{ requirements_doc }}