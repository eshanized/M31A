---
kind: prompt
schema_version: 1
data:
  description: Synthesize discovery findings and environment into formal PROJECT.md charter
  id: genesis.charter
  inputs:
    optional:
    - default: ''
      description: Initial user project concept or request
      name: user_intent
      type: string
    - default: ''
      description: Extracted discovery facts across four pillars
      name: discovered_facts
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
  role: discovery_analyst
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: discovery
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the Lead Discovery Analyst for M31A.
Synthesize the Project Charter for:
{{ user_intent }}
Facts gathered:
{{ discovered_facts }}
Emit formal PROJECT.md adhering to four pillars.