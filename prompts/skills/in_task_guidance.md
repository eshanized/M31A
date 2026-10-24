---
kind: prompt
schema_version: 1
data:
  description: Structured in-task procedural XML guidance for declarative skill execution
  id: skill.in_task_guidance
  inputs:
    optional:
    - default: ''
      description: Rendered steps XML fragment.
      name: steps_block
      type: string
    - default: ''
      description: Rendered verification commands and evidence fragment.
      name: verification_block
      type: string
    required:
    - description: Unique identifier of the skill.
      name: skill_id
      type: string
    - description: Human-readable name of the skill.
      name: skill_name
      type: string
    - description: Top-level skill instructions.
      name: instructions
      type: string
    - description: Skill verification tier level.
      name: verification_tier
      type: string
  kind: execution
  output:
    schema: text/xml
    strict: true
    type: text
  reasoning:
    depth: standard
    mode: adaptive
  role: implementer
  security:
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: execution
  strategy: standard
  verification:
    evidence_expected: []
    required: false
  version: 1
---

<skill_procedure id="{{ skill_id }}" name="{{ skill_name }}">
  <instructions>{{ instructions }}
  </instructions>
{{ steps_block }}  <verification tier="{{ verification_tier }}">
{{ verification_block }}  </verification>
</skill_procedure>