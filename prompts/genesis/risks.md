---
kind: prompt
schema_version: 1
data:
  description: Synthesize risk register with qualitative ratings and mitigations into RISKS.md
  id: genesis.risks
  inputs:
    optional: []
    required:
    - description: Requirements specification content
      name: requirements_doc
      type: string
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
Requirements:
{{ requirements_doc }}
Architecture:
{{ architecture_doc }}
Construct bounded risk register with mitigations and verification plans. Emit RISKS.md.