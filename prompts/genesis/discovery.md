---
kind: prompt
schema_version: 1
data:
  description: Socratic project discovery interview to extract constraints and draft PROJECT.md
  id: genesis.discovery
  inputs:
    optional:
    - default: ''
      description: Initial user project concept or request
      name: user_intent
      type: string
    - default: '1'
      description: Current turn in the interactive discovery dialogue
      name: turn_number
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
  role: researcher
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
Lead a structured Socratic interview for:
{{ user_intent }}
Turn: {{ turn_number }}
Clarify vision, boundaries, constraints, and non-goals.