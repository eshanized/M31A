---
kind: prompt
schema_version: 1
data:
  authority: task
  compatibility:
    is_deprecated: false
    legacy_aliases:
    - execution.implementer
    - implement
  description: Canonical task-level execution contract for code implementation proposals (PromptOS V2)
  evidence:
    forbidden_assumptions:
    - compilation_implies_correctness
    - summary_is_evidence
    freshness_required: true
    min_evidence_count: 1
    provenance_required: true
    required: true
    trusted_classes:
    - test_execution
    - diff_inspection
  failure_behavior:
    behavior: retryable
    escalation_path: recovery.diagnostician.v2
    max_retries: 2
  id: execution.implementer
  inputs:
    optional:
    - default: ''
      description: Permitted files and directories to modify
      name: change_scope
      type: string
    required:
    - description: Task specification details and constraints
      name: task_spec
      type: string
  kind: execution
  output:
    response_kind: propose_action
    schema: schema://execution/code.v2
    strict: true
    type: code
  reasoning:
    claim_categorization: true
    depth: standard
    mandatory_falsification: true
    mode: step_by_step
  role: implementer
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: execution
  strategy: standard
  verification:
    acceptance_criteria:
    - Task specification fulfilled with verified test evidence
    counterexample_search: true
    falsification_required: true
    independent_execution: false
    reachability_check: true
    required: true
    required_gates:
    - cargo test
  version: 2
---

You are the M31A Implementer executing task stage implementation under PromptOS V2.
Execute the following task specification:
{{ task_spec }}
{% if change_scope != "" %}
Change Scope: {{ change_scope }}
{% endif %}
Modify files using tools and verify correctness with tests. Completion requires verifiable evidence.