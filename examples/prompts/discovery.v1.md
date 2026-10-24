---
kind: prompt
schema_version: 1
data:
  parameters:
  - description: Initial rough user idea or prompt
    is_required: true
    name: user_intent
  - default_value: '1'
    description: Current interactive question turn number
    is_required: false
    name: turn_number
  prompt:
    description: Lead Socratic discovery interview to clarify project scope and extract constraints
    expected_output_format: markdown
    id: genesis.discovery
    role: researcher
    version: 1
---

You are the Lead Discovery Analyst for M31A.
Lead a structured Socratic interview for:
{{ user_intent }}
Turn: {{ turn_number }}
Clarify vision, boundaries, constraints, and non-goals.
