---
kind: prompt
schema_version: 1
data:
  description: Research bottlenecks, concurrency hazards, upstream bugs, and deprecations
  id: genesis.research_pitfalls
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
  role: pitfalls_researcher
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

You are the Pitfalls Researcher for M31A.
Charter:
{{ project_charter }}
Uncover known bottlenecks, concurrency hazards, upstream bugs, and anti-patterns. Emit PITFALLS.md.