---
kind: prompt
schema_version: 1
data:
  authority: task
  compatibility:
    is_deprecated: false
    legacy_aliases:
    - execution.verifier
    - verify
  description: Canonical task-level verification contract requiring test output evidence (PromptOS V2)
  evidence:
    forbidden_assumptions:
    - implementer_summary_is_truth
    freshness_required: true
    min_evidence_count: 1
    provenance_required: true
    required: true
    trusted_classes:
    - test_execution
  failure_behavior:
    behavior: retryable
    escalation_path: recovery.diagnostician.v2
    max_retries: 1
  id: verification.task
  inputs:
    optional:
    - default: ''
      description: Test output log
      name: test_output
      type: string
    required:
    - description: Identifier of the task
      name: task_id
      type: string
    - description: Declared acceptance criteria
      name: acceptance_criteria
      type: string
    - description: Verification test command
      name: test_command
      type: string
  kind: verification
  output:
    response_kind: report_verification
    schema: schema://verification/task_verdict.v2
    strict: true
    type: verdict
  reasoning:
    claim_categorization: true
    depth: standard
    mandatory_falsification: true
    mode: step_by_step
  role: verifier
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: verify
  strategy: standard
  verification:
    acceptance_criteria:
    - Test suite verified with raw execution evidence
    counterexample_search: true
    falsification_required: true
    independent_execution: true
    reachability_check: true
    required: true
    required_gates:
    - cargo test
  version: 2
---

# TASK VERIFICATION (V2)
- **Task ID:** {{ task_id }}
- **Command:** {{ test_command }}

### Acceptance Criteria:
{{ acceptance_criteria }}

{% if test_output != "" %}
### Output Log:
{{ test_output }}
{% endif %}

Verify test results and produce verdict based strictly on empirical evidence.