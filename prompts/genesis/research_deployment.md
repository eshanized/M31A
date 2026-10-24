---
kind: prompt
schema_version: 1
data:
  description: Research process supervisors, systemd, packaging, and resource bounds
  id: genesis.research_deployment
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
  role: deployment_researcher
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

You are the Deployment Researcher for M31A.
Charter:
{{ project_charter }}
Investigate process supervisors, packaging, distribution targets, and resource bounds. Emit DEPLOYMENT.md.