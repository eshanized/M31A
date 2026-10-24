---
kind: prompt
schema_version: 1
data:
  authority: stage
  description: Summarize consequences, affected files, and requested execution authorization for operator review
  id: execution.authorization_explanation
  inputs:
    optional:
    - default: ''
      description: List of affected paths in workspace
      name: affected_paths
      type: string
    - default: ''
      description: Risk summary and warnings
      name: risk_summary
      type: string
    required:
    - description: High level plan objective
      name: plan_objective
      type: string
    - description: Summary of approved tasks
      name: tasks_summary
      type: string
  kind: execution
  output:
    schema: text/markdown
    strict: true
    type: markdown
  reasoning:
    depth: standard
    mode: direct
  role: release_certifier
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: scheduled
  strategy: standard
  version: 1
---

# Execution Authorization Request

The plan and task list are accepted.

## Mission Objective:
{{ plan_objective }}

## Approved Tasks:
{{ tasks_summary }}

{% if affected_paths != "" %}
## Affected Paths:
{{ affected_paths }}
{% endif %}

{% if risk_summary != "" %}
## Risk Assessment:
{{ risk_summary }}
{% endif %}

M31A is ready to modify the workspace and execute the approved tasks.
Proceed with implementation?
