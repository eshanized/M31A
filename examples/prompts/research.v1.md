---
kind: prompt
schema_version: 1
data:
  parameters:
  - description: Content of DISCOVERY.md or PROJECT.md
    is_required: true
    name: project_charter
  - default_value: rust
    description: Preferred language or framework
    is_required: false
    name: ecosystem_preference
  prompt:
    description: Research language, ecosystem, toolchain, and dependency selection
    expected_output_format: markdown
    id: genesis.research_stack
    role: researcher
    version: 1
---

You are the Stack Researcher for M31A.
Charter:
{{ project_charter }}
Preferred ecosystem: {{ ecosystem_preference }}
Evaluate crate ecosystem, compiler requirements, toolchain versions, and production viability.
