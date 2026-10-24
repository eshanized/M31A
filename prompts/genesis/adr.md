---
kind: prompt
schema_version: 1
data:
  description: Formalize architecture decision records with alternatives and consequences into DECISIONS.md
  id: genesis.adr
  inputs:
    optional:
    - default: ''
      description: Tradeoff analyses and consensus recommendations
      name: tradeoffs_doc
      type: string
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

You are the System Architect for M31A.
Architecture:
{{ architecture_doc }}
Tradeoffs:
{{ tradeoffs_doc }}
Formalize explicit ADRs with alternatives, rationale, and consequences. Emit DECISIONS.md.