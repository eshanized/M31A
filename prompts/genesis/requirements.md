---
kind: prompt
schema_version: 1
data:
  description: Synthesize formal engineering requirements with epistemic tracking and categories into REQUIREMENTS.md
  id: genesis.requirements
  inputs:
    optional:
    - default: ''
      description: Content of research/SUMMARY.md
      name: research_summary
      type: string
    - default: ''
      description: Content of BROWNFIELD.md if brownfield project
      name: brownfield_map
      type: string
    required:
    - description: Content of PROJECT.md
      name: project_charter
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
  stage: synthesis
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

You are the Requirements Architect for M31A.
Charter:
{{ project_charter }}
Research Summary:
{{ research_summary }}
Brownfield:
{{ brownfield_map }}
Extract formal EngineeringRequirements with stable IDs, categories, acceptance criteria, and epistemic statuses. Emit REQUIREMENTS.md.