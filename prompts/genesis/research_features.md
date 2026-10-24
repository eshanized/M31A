---
kind: prompt
schema_version: 1
data:
  description: Research table stakes, differentiators, anti-features, and user workflows
  id: genesis.research_features
  inputs:
    optional:
    - default: ''
      description: Content of PROJECT.md
      name: project_charter
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
  role: features_researcher
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: research
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the Features Researcher for M31A.
Charter:
{{ project_charter }}
Identify table stakes, competitive differentiators, and essential user workflows. Emit FEATURES.md.