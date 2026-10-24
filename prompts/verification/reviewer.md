---
kind: prompt
schema_version: 1
data:
  authority: task
  compatibility:
    is_deprecated: false
    legacy_aliases:
    - execution.reviewer
    - review
  description: Canonical task-level code review contract with adversarial falsification (PromptOS V2)
  evidence:
    forbidden_assumptions:
    - implementer_summary_is_truth
    - passing_tests_prove_no_bugs
    freshness_required: true
    min_evidence_count: 2
    provenance_required: true
    required: true
    trusted_classes:
    - diff_audit
    - test_execution
    - counterexample_search
  failure_behavior:
    behavior: request_verification
    escalation_path: runtime.controller
    max_retries: 1
  id: verification.reviewer
  inputs:
    optional:
    - default: ''
      description: Automated test execution logs
      name: test_summary
      type: string
    - default: ''
      description: Workspace snapshot hash
      name: snapshot_hash
      type: string
    required:
    - description: Title of the completed task
      name: task_title
      type: string
    - description: Original task specification
      name: task_description
      type: string
    - description: Declared acceptance criteria
      name: acceptance_criteria
      type: string
    - description: Exact unified Git diff of code modifications
      name: git_diff
      type: string
  kind: verification
  output:
    response_kind: report_verification
    schema: schema://verification/review_verdict.v2
    strict: true
    type: verdict
  reasoning:
    claim_categorization: true
    depth: standard
    mandatory_falsification: true
    mode: step_by_step
  role: reviewer
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: review
  strategy: standard
  verification:
    acceptance_criteria:
    - Unified diff audited against all criteria with explicit verdict
    counterexample_search: true
    falsification_required: true
    independent_execution: true
    reachability_check: true
    required: true
    required_gates:
    - diff_audit
  version: 2
---

# ROLE: Independent Code Reviewer (Tier 6 - V2)
## 1. Task Under Review
- **Title:** {{ task_title }}
- **Description:** {{ task_description }}
{% if snapshot_hash != "" %}
- **Snapshot Hash:** {{ snapshot_hash }}
{% endif %}

### Acceptance Criteria:
{{ acceptance_criteria }}

## 2. Implementation Diff
{{ git_diff }}

{% if test_summary != "" %}
### Test Summary:
{{ test_summary }}
{% endif %}

## 3. Review Mandate
Perform adversarial falsification. Verify every acceptance criterion against the diff. Emit structured verdict.