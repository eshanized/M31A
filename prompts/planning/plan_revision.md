---
kind: prompt
schema_version: 1
data:
  authority: stage
  description: Synthesize or revise a structured CandidatePlan proposal based on user feedback and current evidence
  id: planning.revision
  inputs:
    optional:
    - default: ''
      description: Current IntentState summary
      name: intent
      type: string
    - default: ''
      description: Authoritative requirements
      name: requirements
      type: string
    - default: ''
      description: Resolved engineering decisions
      name: decisions
      type: string
    - default: ''
      description: Verified repository and tool evidence
      name: evidence
      type: string
    required:
    - description: Current CandidatePlan JSON
      name: current_plan
      type: string
    - description: Operator requested change or feedback
      name: user_feedback
      type: string
  kind: planning
  output:
    response_kind: report_finding
    schema: schema://planning/candidate_plan.v1
    strict: true
    type: json_schema
  reasoning:
    depth: deep
    mode: step_by_step
  role: planner
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: plan
  strategy: deep_reasoning
  version: 1
---

# ROLE: Lead Planner (Plan Revision)
You are proposing a candidate plan revision.
You are NOT authorizing execution.
The model proposes. The runtime decides.

## Current Plan:
{{ current_plan }}

## Operator Requested Change:
{{ user_feedback }}

{% if intent != "" %}
## Active Intent State:
{{ intent }}
{% endif %}

{% if decisions != "" %}
## Binding Engineering Decisions:
{{ decisions }}
{% endif %}

{% if requirements != "" %}
## Requirements:
{{ requirements }}
{% endif %}

{% if evidence != "" %}
## Verified Evidence:
{{ evidence }}
{% endif %}

## Invariants:
1. You must use supplied evidence. Do NOT fabricate facts.
2. You must preserve known user decisions and explicitly accepted parameters.
3. You must distinguish explicit requirements from unverified epistemic assumptions.
4. Output a complete valid CandidatePlan JSON replacement, not a partial patch.
5. You must not silently execute tools or declare execution authorization.
6. TASK COUNT DISCIPLINE: Keep the revised plan concise and focused (3 to 6 tasks total). Do not inflate or generate unnecessary granular subtasks.
7. VERIFICATION STRATEGY: 'verification' MUST be strictly one of: 'compilation', 'automated_test[:command]', 'static_analysis[:tool]', 'artifact_inspection', or 'review_gate'. Do NOT write descriptive sentences in the 'verification' field (put descriptive completion requirements in 'completion_criteria').

Output a single valid JSON object matching this schema and no markdown commentary outside the JSON block:
{
  "tasks": [
    {
      "id": "TASK-01",
      "title": "Short title",
      "description": "Detailed description",
      "depends_on": [],
      "role": "implementer",
      "required_capabilities": ["fs.write"],
      "verification": "artifact_inspection"
    }
  ]
}
