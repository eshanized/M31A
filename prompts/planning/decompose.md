---
kind: prompt
schema_version: 1
data:
  authority: stage
  compatibility:
    is_deprecated: false
    legacy_aliases:
    - planning.decompose
  description: Canonical task decomposition generating DAG execution plans with explicit verification and evidence requirements per task (PromptOS V2)
  evidence:
    forbidden_assumptions:
    - linear_execution_is_optimal
    freshness_required: true
    min_evidence_count: 1
    provenance_required: true
    required: true
    trusted_classes:
    - dag_validation
    - dependency_analysis
  failure_behavior:
    behavior: retryable
    escalation_path: runtime.controller
    max_retries: 2
  id: planning.decompose
  inputs:
    optional:
    - default: ''
      description: Existing plan DAG if refining
      name: existing_dag
      type: string
    - default: ''
      description: Upstream requirement statements
      name: requirements
      type: string
    - default: ''
      description: Upstream assumption statements
      name: assumptions
      type: string
    - default: ''
      description: Upstream engineering decisions
      name: decisions
      type: string
    - default: ''
      description: Upstream research summary
      name: research_summary
      type: string
    - default: ''
      description: Unresolved epistemic unknowns from discovery and research
      name: unknowns
      type: string
    - default: ''
      description: Consequential decisions requiring explicit operator direction
      name: user_decisions
      type: string
    - default: ''
      description: Definitively resolved invariants from discovery
      name: resolved_invariants
      type: string
    - default: implementer, researcher, verifier
      description: Comma-separated list of admissible agent role ids; each task must use exactly one
      name: available_roles
      type: string
    required:
    - description: Project charter and requirements
      name: charter
      type: string
    - description: System architecture specification
      name: architecture
      type: string
    - description: High-level goal to decompose
      name: goal
      type: string
  kind: planning
  output:
    response_kind: report_finding
    schema: schema://planning/plan_dag.v2
    strict: true
    type: json_schema
  reasoning:
    claim_categorization: true
    depth: deep
    mandatory_falsification: true
    mode: step_by_step
  role: planner
  security:
    immutable_safety_layer: true
    runtime_authority: true
    treat_context_as_untrusted: true
  stage: plan
  strategy: deep_reasoning
  verification:
    acceptance_criteria:
    - DAG is acyclic with clear topological ordering
    - Every task specifies explicit acceptance criteria and verification commands
    counterexample_search: true
    falsification_required: true
    independent_execution: true
    reachability_check: true
    required: true
    required_gates:
    - dag_cycle_check
  version: 2
---

# ROLE: Lead Planner (PromptOS V2)
## Project Charter & Architecture
### Charter:
{{ charter }}

### Architecture:
{{ architecture }}

### Goal to Decompose:
{{ goal }}

{% if requirements != "" %}
### Upstream Requirements:
{{ requirements }}
{% endif %}

{% if assumptions != "" %}
### Upstream Assumptions (challenge rather than assume silently):
{{ assumptions }}
{% endif %}

{% if decisions != "" %}
### Upstream Engineering Decisions (binding unless contradicted by evidence):
{{ decisions }}
{% endif %}

{% if research_summary != "" %}
### Upstream Research Summary:
{{ research_summary }}
{% endif %}

{% if user_decisions != "" %}
### Consequential User Decisions (explicit operator direction required; DO NOT invent answers):
{{ user_decisions }}
{% endif %}

{% if unknowns != "" %}
### Unresolved Epistemic Unknowns (requires research or clarification; DO NOT assume silently):
{{ unknowns }}
{% endif %}

{% if resolved_invariants != "" %}
### Definitively Resolved Invariants:
{{ resolved_invariants }}
{% endif %}

{% if existing_dag != "" %}
### Existing Plan DAG:
{{ existing_dag }}
{% endif %}

Decompose the goal into an acyclic Directed Acyclic Graph (DAG) of actionable tasks.
TASK COUNT DISCIPLINE (match granularity to the work, never inflate):
1. 0 tasks (direct informational ONLY): ONLY for purely conversational questions requiring no repository inspection — e.g. "What is Rust?", "Explain this error message". Output {"tasks": []}.
2. 1+ tasks (repository research/study/audit): objectives requesting codebase study, analysis, architecture review, security audit, or repository investigation MUST produce at least one task with role 'researcher' and read-only capabilities (fs.read, repo.read). Examples: "Study the codebase", "Analyze the architecture", "Audit for security issues". These are executable research work, NOT zero-task requests.
3. 1 task (atomic work): focused bug fixes and single-module changes SHOULD be exactly 1 task with role 'implementer'. The implementer inspects, edits, and verifies using cargo test.
4. N tasks (multi-step work): only genuinely multi-component, multi-stage, or multi-file efforts decompose further. Each task must specify prerequisites in depends_on. Do NOT create a 'verifier' task before the 'implementer' task; use 'researcher' or 'diagnostician' pre-investigation only when the fault location is truly unknown.
For each task declare:
1. Unique task_id and title
2. Direct dependencies (parents)
3. Assigned agent role
4. Clear acceptance criteria
5. Empirical verification command (e.g. `cargo test --test ...`)
6. Verification strategy: one of `compilation`, `automated_test[:command]`,
   `static_analysis[:tool]`, `artifact_inspection`, `review_gate`
   (omit to use the role default; unknown values are rejected)

### Available agent roles (each task MUST use exactly one of these role ids):
{{ available_roles }}

ROLE CAPABILITY BOUNDARIES:
- Only the 'implementer' role is permitted mutating capabilities (e.g. 'fs.write'). Any task that modifies code MUST use role 'implementer'.
- The roles 'researcher', 'verifier', 'reviewer', and 'architect' are strictly read-only and must never be assigned code modification tasks.

REQUIRED CAPABILITY VOCABULARY (use ONLY these capability ids; any other id is rejected):
fs.read, fs.write, cargo.test, cargo.check, shell.exec, git_ops, artifacts.read, artifacts.write, repo.read, docs.search, web.search
- Code modification tasks: role 'implementer' with fs.read, repo.read, fs.write, cargo.test, cargo.check (never docs.search, web.search).
- Test-execution tasks: role 'verifier' with fs.read, cargo.test (never cargo.check).
- Investigation tasks: role 'researcher' with fs.read, repo.read, docs.search.
- Diagnosis tasks: role 'diagnostician' with fs.read, repo.read, logs.inspect (never docs.search, web.search).
- 'cargo.check' is an implementer/integrator capability, never verifier/researcher.
- Every other role (reviewer, architect, researcher specialists, certifiers) is strictly read-only: give them ONLY fs.read, repo.read, artifacts.read — never cargo.test, cargo.check, shell.exec, git_ops, web.search, or fs.write.
- Never assign cargo.test or cargo.check to reviewer, auditor, release_certifier, diagnostician, discovery_analyst, synthesizer, planner, or architect: test execution belongs to verifier (cargo.test) and implementer (cargo.test, cargo.check).
- If unsure which capabilities a role permits, OMIT required_capabilities for that task: the runtime assigns the role's default capabilities. Never guess capability ids.

Output a single valid JSON object with this exact schema and no prose outside it. Keep task descriptions concise (1-2 sentences) to avoid token truncation:
{"tasks": [{"id": "TASK-01", "title": "Short title", "description": "Concise actions and expected outputs", "depends_on": [], "required_capabilities": ["fs.read"], "role": "implementer", "verification": "automated_test:cargo test", "completion_criteria": ["criterion"], "requirement_keys": []}]}
Accepted field aliases: task_id for id, dependencies for depends_on, assigned_role for role, acceptance_criteria for completion_criteria.
depends_on MUST reference only task ids defined in this plan (e.g. TASK-01) — never ADR, REQ, file, or external ids.