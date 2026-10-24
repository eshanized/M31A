---
kind: prompt
schema_version: 1
data:
  description: Research auth models, permission checks, crypto requirements, and sandboxing
  id: genesis.research_security
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
  role: security_researcher
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

You are the Security Researcher for M31A.
Charter:
{{ project_charter }}
Analyze threat models, authorization boundaries, cryptographic requirements, and sandboxing. Emit SECURITY.md.