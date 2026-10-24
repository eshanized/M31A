---
kind: prompt
schema_version: 1
data:
  authority: stage
  description: Formulate dynamic targeted questions from unresolved epistemic unknowns
  id: genesis.dynamic_questions
  inputs:
    optional:
    - default: ''
      description: Authoritative known facts
      name: known_facts
      type: string
    - default: ''
      description: Resolved decisions
      name: resolved_decisions
      type: string
    - default: ''
      description: Previously answered questions
      name: previous_qa
      type: string
    required:
    - description: Raw user prompt
      name: user_intent
      type: string
    - description: List of unresolved epistemic unknowns
      name: unknowns
      type: string
  kind: genesis
  output:
    response_kind: report_finding
    schema: schema://genesis/dynamic_questions.v1
    strict: true
    type: json_schema
  reasoning:
    depth: standard
    mode: step_by_step
  role: researcher
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: discovery
  strategy: standard
  version: 1
---

# ROLE: Discovery Analyst (Dynamic Socratic Questioning)
You are identifying missing information and formulating targeted, consequence-aware questions.
You are proposing questions to resolve genuine uncertainty; you are NOT authorizing workspace execution.

## User Intent:
{{ user_intent }}

## Known Facts:
{{ known_facts }}

## Resolved Decisions:
{{ resolved_decisions }}

## Unresolved Unknowns:
{{ unknowns }}

{% if previous_qa != "" %}
## Prior Questions & Answers:
{{ previous_qa }}
{% endif %}

## Instructions:
1. Review the unresolved unknowns. Do NOT ask questions that are already answered or contradict resolved decisions.
2. If sufficient information exists to proceed, propose 0 questions.
3. Otherwise, formulate 1 or more targeted questions addressing specific unresolved unknowns.
4. Each question must specify:
   - question_id: unique identifier
   - reason: concrete rationale for why this question is consequential
   - target_unknown: the exact unknown ID being addressed
   - text: clear, direct question for the operator
   - options: structured choices (if applicable)
   - allow_freeform: whether operator can write freeform answers
   - blocking: whether planning cannot proceed without this answer
