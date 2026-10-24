---
kind: prompt
schema_version: 1
data:
  description: Reconcile research dimensions and synthesize tradeoffs into SUMMARY.md
  id: genesis.research_synthesis
  inputs:
    optional:
    - default: ''
      description: Aggregated research documents
      name: research_findings
      type: string
    required: []
  kind: genesis
  output:
    schema: text/markdown
    strict: true
    type: markdown
  reasoning:
    depth: standard
    mode: adaptive
  role: synthesizer
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: synthesis
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the Research Synthesizer for M31A.
Reconcile findings, resolve conflicting recommendations, and emit SUMMARY.md:
{{ research_findings }}