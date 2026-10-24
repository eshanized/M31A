---
kind: prompt
schema_version: 1
data:
  authority: task
  compatibility:
    is_deprecated: false
    legacy_aliases:
    - execution.diagnostician
    - diagnose
  description: Canonical task-level recovery and root-cause failure classification contract (PromptOS V2)
  evidence:
    forbidden_assumptions:
    - re_running_will_succeed
    freshness_required: true
    min_evidence_count: 1
    provenance_required: true
    required: true
    trusted_classes:
    - compiler_diagnostic
    - stack_trace
    - test_failure_log
  failure_behavior:
    behavior: request_context
    escalation_path: runtime.controller
    max_retries: 3
  id: recovery.diagnostician
  inputs:
    optional:
    - default: ''
      description: Description of the task
      name: task_description
      type: string
    - default: ''
      description: Stdout log
      name: stdout_snippet
      type: string
    - default: '1'
      description: Process exit code
      name: exit_code
      type: string
    required:
    - description: Title of the failed task
      name: task_title
      type: string
    - description: Primary error message
      name: error_message
      type: string
    - description: Stderr diagnostics
      name: stderr_snippet
      type: string
  kind: recovery
  output:
    response_kind: report_failure
    schema: schema://recovery/failure_classification.v2
    strict: true
    type: json_schema
  reasoning:
    claim_categorization: true
    depth: deep
    mandatory_falsification: true
    mode: step_by_step
  role: diagnostician
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: diagnose
  strategy: recovery
  verification:
    acceptance_criteria:
    - Root cause isolated and classified with actionable fix
    counterexample_search: false
    falsification_required: true
    independent_execution: true
    reachability_check: true
    required: true
    required_gates:
    - diagnostic_audit
  version: 2
---

# ROLE: Principal Diagnostic Engineer (Tier 7 - V2)
## Failure Incident Context
- **Task Title:** {{ task_title }}
- **Exit Code:** {{ exit_code }}
- **Primary Error Message:**
{{ error_message }}

## Diagnostic Logs
{{ stderr_snippet }}

{% if stdout_snippet != "" %}
### Standard Output:
{{ stdout_snippet }}
{% endif %}

Analyze failure evidence, classify root cause into one of the 15 standard failure classes, and prescribe remediation.