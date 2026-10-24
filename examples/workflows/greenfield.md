---
kind: workflow
schema_version: 1
data:
  manifest_version: 1
  steps:
  - allows_parallelism: false
    depends_on: []
    key: discovery
    name: Project Discovery
    outputs:
    - artifact_name: DISCOVERY.md
      relative_path: .m31a/project/DISCOVERY.md
      schema_type: markdown.discovery.v1
    prompt: genesis.discovery.v1
    quality_gate:
      max_ambiguity_percent: 15
      require_human_approval: false
      required_artifacts:
      - DISCOVERY.md
      schema: discovery.v1
    role: researcher
    timeout_secs: 1800
  - allows_parallelism: true
    depends_on:
    - discovery
    inputs:
    - artifact_name: DISCOVERY.md
      is_optional: false
      parameter_name: project_charter
      source_step_key: discovery
    key: research_stack
    name: Stack Research
    outputs:
    - artifact_name: STACK.md
      relative_path: .m31a/project/research/STACK.md
      schema_type: markdown.research.v1
    prompt: genesis.research_stack.v1
    quality_gate:
      require_human_approval: false
      required_artifacts:
      - STACK.md
      schema: research.stack.v1
    role: researcher
    timeout_secs: 1200
  workflow:
    default_recovery_strategy:
      type: fail
    description: Discover, research, synthesize, architect, and roadmap a new project
    id: greenfield
    name: Greenfield Project Genesis
    version: 1
---

# Greenfield Project Genesis

Discover, research, synthesize, architect, and roadmap a new project
