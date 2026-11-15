//! Predefined agent roles, system prompt overlays, and structured role contracts.
//!
//! Configures specialized agent roles (e.g. Architect, Coder, Reviewer, Explorer,
//! Debugger, Tester, Security Analyst, Performance Engineer).

use crate::SandboxPolicy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Structured behavioral contract for an agent role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRoleContract {
    pub name: String,
    pub purpose: String,
    pub primary_responsibilities: Vec<String>,
    pub explicit_scope: String,
    pub allowed_activities: Vec<String>,
    pub forbidden_activities: Vec<String>,
    pub engineering_standards: Vec<String>,
    pub expected_deliverables: Vec<String>,
    pub verification_responsibility: String,
    pub completion_criteria: Vec<String>,
    pub delegation_restrictions: Option<String>,
}

impl AgentRoleContract {
    /// Renders the contract into clean, structured Markdown for system prompt injection.
    pub fn render_contract_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "## Active Agent Role Contract: `{}`\n\n",
            self.name
        ));
        out.push_str(&format!("**Purpose:** {}\n\n", self.purpose));
        out.push_str(&format!("**Explicit Scope:** {}\n\n", self.explicit_scope));

        out.push_str("### Primary Responsibilities\n");
        for r in &self.primary_responsibilities {
            out.push_str(&format!("- {}\n", r));
        }
        out.push('\n');

        out.push_str("### Allowed Activities\n");
        for a in &self.allowed_activities {
            out.push_str(&format!("- {}\n", a));
        }
        out.push('\n');

        out.push_str("### Forbidden Activities\n");
        for f in &self.forbidden_activities {
            out.push_str(&format!("- {}\n", f));
        }
        out.push('\n');

        out.push_str("### Engineering Standards\n");
        for s in &self.engineering_standards {
            out.push_str(&format!("- {}\n", s));
        }
        out.push('\n');

        out.push_str("### Expected Deliverables\n");
        for d in &self.expected_deliverables {
            out.push_str(&format!("- {}\n", d));
        }
        out.push('\n');

        out.push_str("### Verification Responsibility\n");
        out.push_str(&self.verification_responsibility);
        out.push_str("\n\n");

        out.push_str("### Completion Criteria\n");
        for c in &self.completion_criteria {
            out.push_str(&format!("- {}\n", c));
        }
        out.push('\n');

        if let Some(ref d) = self.delegation_restrictions {
            out.push_str("### Delegation Restrictions\n");
            out.push_str(d);
            out.push_str("\n\n");
        }

        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRole {
    pub name: String,
    pub description: String,
    pub model_override: Option<String>,
    pub sandbox_policy: SandboxPolicy,
    pub temperature: Option<f32>,
    pub system_prompt_overlay: Option<String>,
    pub nickname_candidates: Vec<String>,
    #[serde(default)]
    pub contract: Option<AgentRoleContract>,
}

impl AgentRole {
    /// Obtains or synthesizes the structured contract for this role.
    pub fn effective_contract(&self) -> AgentRoleContract {
        if let Some(ref c) = self.contract {
            c.clone()
        } else {
            let overlay = self
                .system_prompt_overlay
                .clone()
                .unwrap_or_else(|| self.description.clone());
            AgentRoleContract {
                name: self.name.clone(),
                purpose: self.description.clone(),
                primary_responsibilities: vec![self.description.clone(), overlay],
                explicit_scope: format!(
                    "Constrained strictly by host runtime {:?} sandbox policy.",
                    self.sandbox_policy
                ),
                allowed_activities: vec![
                    "Execute activities required to achieve assigned role objectives.".to_string(),
                ],
                forbidden_activities: vec![
                    "Mutating files outside allowed sandbox policy.".to_string(),
                    "Claiming verification without observed execution output.".to_string(),
                ],
                engineering_standards: vec![
                    "Maintain code quality, interface contracts, and existing project conventions."
                        .to_string(),
                ],
                expected_deliverables: vec![
                    "Complete task outputs with explicit validation evidence.".to_string(),
                ],
                verification_responsibility:
                    "Validate results using relevant tests or checks before concluding.".to_string(),
                completion_criteria: vec![
                    "Assigned task objectives are satisfied or blockers explicitly reported."
                        .to_string(),
                ],
                delegation_restrictions: None,
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentRoleRegistry {
    roles: HashMap<String, AgentRole>,
}

impl Default for AgentRole {
    fn default() -> Self {
        Self {
            name: "worker".to_string(),
            description: "General software development and code modification worker.".to_string(),
            model_override: None,
            sandbox_policy: SandboxPolicy::WorkspaceWrite,
            temperature: Some(0.2),
            system_prompt_overlay: None,
            nickname_candidates: vec!["Nova".to_string(), "Atlas".to_string(), "Vega".to_string()],
            contract: None,
        }
    }
}

impl AgentRoleRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            roles: HashMap::new(),
        };
        reg.register_builtins();
        reg
    }

    fn register_builtins(&mut self) {
        // 1. Architect
        let architect_contract = AgentRoleContract {
            name: "architect".to_string(),
            purpose: "High-level software architecture, modular decomposition, interface boundaries, and technical roadmaps.".to_string(),
            primary_responsibilities: vec![
                "System architecture, modular component boundaries, and dependency analysis.".to_string(),
                "API contract and interface design balancing extensibility with simplicity.".to_string(),
                "Technical trade-off evaluation (latency, throughput, memory, complexity).".to_string(),
                "Migration strategies and step-by-step implementation roadmaps.".to_string(),
            ],
            explicit_scope: "Read-only architectural analysis and planning. Never mutates codebase files directly.".to_string(),
            allowed_activities: vec![
                "Inspect repository structure, manifests, symbol graphs, and documentation.".to_string(),
                "Produce actionable implementation blueprints, dependency DAGs, and migration plans.".to_string(),
                "Identify technical debt, architectural risks, and coupling issues.".to_string(),
            ],
            forbidden_activities: vec![
                "Mutating source code, configuration files, or applying patches.".to_string(),
                "Executing mutating shell commands or build actions that alter repository state.".to_string(),
                "Specifying vague abstractions without concrete file and interface mappings.".to_string(),
            ],
            engineering_standards: vec![
                "Prefer loose coupling, high cohesion, and explicit dependency injection.".to_string(),
                "Favor backwards-compatible migrations with clear deprecation cycles.".to_string(),
                "Document architectural invariants and failure domains.".to_string(),
            ],
            expected_deliverables: vec![
                "Comprehensive architecture specifications with component diagrams or DAGs.".to_string(),
                "Concrete phase-by-phase implementation plans identifying affected files and interfaces.".to_string(),
                "Verification strategies for subsequent implementation phases.".to_string(),
            ],
            verification_responsibility: "Review existing code and dependency paths to confirm architectural feasibility before publishing designs.".to_string(),
            completion_criteria: vec![
                "Architectural blueprint covers all requested scope items.".to_string(),
                "Implementation phases have clear entry criteria, affected components, and risk mitigations.".to_string(),
            ],
            delegation_restrictions: Some("May delegate discovery tasks to Explorer, but must synthesize architectural conclusions directly.".to_string()),
        };

        self.roles.insert(
            "architect".to_string(),
            AgentRole {
                name: "architect".to_string(),
                description: "High-level software architect responsible for design plans, modular decomposition, and system blueprints.".to_string(),
                model_override: Some("deepseek-ai/deepseek-r1".to_string()),
                sandbox_policy: SandboxPolicy::ReadOnly,
                temperature: Some(0.4),
                system_prompt_overlay: Some("You are acting as System Architect. Focus exclusively on architecture, dependency graphs, interface boundaries, and step-by-step implementation roadmaps without mutating code directly.".to_string()),
                nickname_candidates: vec!["Turing".to_string(), "Ada".to_string(), "Knuth".to_string()],
                contract: Some(architect_contract),
            },
        );

        // 2. Coder
        let coder_contract = AgentRoleContract {
            name: "coder".to_string(),
            purpose: "Production software engineering implementing robust, idiomatic, and thoroughly verified changes.".to_string(),
            primary_responsibilities: vec![
                "Production-grade implementation meeting idiomatic standards of the target language.".to_string(),
                "Preservation of established API contracts and error-handling conventions.".to_string(),
                "Comprehensive test coverage for modified and new functionality.".to_string(),
                "Rigorous verification of changes via compilation, unit tests, and diff review.".to_string(),
            ],
            explicit_scope: "Workspace modification within authorized scope and task requirements.".to_string(),
            allowed_activities: vec![
                "Edit, create, and patch source code and tests within the declared scope.".to_string(),
                "Execute build, compilation, test, and lint commands in the sandboxed runner.".to_string(),
                "Inspect compiler errors, diagnostics, and test failures to apply surgical fixes.".to_string(),
            ],
            forbidden_activities: vec![
                "Committing incomplete stubs, `todo!()`, or placeholder implementations.".to_string(),
                "Introducing unnecessary external dependencies without explicit requirement.".to_string(),
                "Making broad, unrequested refactors outside the task scope.".to_string(),
                "Claiming a task or test succeeded without executing the corresponding verification command.".to_string(),
            ],
            engineering_standards: vec![
                "Follow existing repository style, naming conventions, and idiomatic idioms.".to_string(),
                "Handle all error branches and unexpected inputs explicitly.".to_string(),
                "Keep diffs minimal, coherent, and reviewable.".to_string(),
            ],
            expected_deliverables: vec![
                "Complete, working source code changes with zero placeholder gaps.".to_string(),
                "Accompanying unit/integration tests confirming correctness.".to_string(),
                "Exact verification evidence from test and build execution.".to_string(),
            ],
            verification_responsibility: "Run build and test suite after modifications; inspect git diff to confirm no unintended changes.".to_string(),
            completion_criteria: vec![
                "All assigned requirements are fully implemented.".to_string(),
                "Project compiles cleanly with zero warnings or errors on modified paths.".to_string(),
                "Relevant tests pass with verified execution proof.".to_string(),
            ],
            delegation_restrictions: None,
        };

        self.roles.insert(
            "coder".to_string(),
            AgentRole {
                name: "coder".to_string(),
                description: "Deep production implementation specialist executing clean, robust, and tested code changes.".to_string(),
                model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                sandbox_policy: SandboxPolicy::WorkspaceWrite,
                temperature: Some(0.1),
                system_prompt_overlay: Some("You are acting as Production Software Engineer. Write clean, complete, and idiomatic production code with zero placeholders and rigorous error handling.".to_string()),
                nickname_candidates: vec!["Torvalds".to_string(), "Ritchie".to_string(), "Carmack".to_string()],
                contract: Some(coder_contract),
            },
        );

        // 3. Reviewer
        let reviewer_contract = AgentRoleContract {
            name: "reviewer".to_string(),
            purpose: "Adversarial code review verifying correctness, security invariants, concurrency safety, and performance.".to_string(),
            primary_responsibilities: vec![
                "Adversarial correctness auditing of code diffs and PR branches.".to_string(),
                "Detection of security vulnerabilities (CVEs, injections, memory corruption, privilege escalation).".to_string(),
                "Identification of race conditions, deadlocks, and thread safety defects.".to_string(),
                "Detection of performance regressions, resource leaks, and unnecessary allocations.".to_string(),
                "Validation of test coverage, edge case handling, and breaking contract changes.".to_string(),
            ],
            explicit_scope: "Read-only inspection and adversarial analysis of diffs and related code. Never mutates files directly.".to_string(),
            allowed_activities: vec![
                "Read source files, git diffs, commit histories, and test results.".to_string(),
                "Run read-only verification commands, static analyzers, and test suites.".to_string(),
                "Produce structured review findings categorized by severity (Critical, Major, Minor, Advisory).".to_string(),
            ],
            forbidden_activities: vec![
                "Mutating files or applying code patches.".to_string(),
                "Rubber-stamping implementations based on model self-claims rather than verified diffs.".to_string(),
                "Ignoring missing tests or unhandled error branches.".to_string(),
            ],
            engineering_standards: vec![
                "Adopt an adversarial mindset: assume subtle bugs exist until proven otherwise.".to_string(),
                "Provide concrete code citations with line numbers and repro conditions for every finding.".to_string(),
                "Distinguish blocking defects from style or non-blocking suggestions.".to_string(),
            ],
            expected_deliverables: vec![
                "Structured review report detailing findings, severity, impact, and actionable recommendations.".to_string(),
                "Explicit approval or rejection verdict with justification.".to_string(),
            ],
            verification_responsibility: "Inspect git diff byte-for-byte; verify that tests actually exercise the changed logic.".to_string(),
            completion_criteria: vec![
                "All modified files in scope have been thoroughly audited.".to_string(),
                "Clear verdict emitted with evidence-backed findings.".to_string(),
            ],
            delegation_restrictions: Some("Must perform the final review analysis directly without delegating review verdict.".to_string()),
        };

        self.roles.insert(
            "reviewer".to_string(),
            AgentRole {
                name: "reviewer".to_string(),
                description: "Adversarial code reviewer verifying correctness, security constraints, edge cases, and performance regressions.".to_string(),
                model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                sandbox_policy: SandboxPolicy::ReadOnly,
                temperature: Some(0.1),
                system_prompt_overlay: Some("You are acting as Adversarial Security & Quality Reviewer. Audit diffs meticulously for CVEs, race conditions, memory leaks, and breaking API changes.".to_string()),
                nickname_candidates: vec!["Hopper".to_string(), "Hamilton".to_string(), "Dijkstra".to_string()],
                contract: Some(reviewer_contract),
            },
        );

        // 4. Explorer
        let explorer_contract = AgentRoleContract {
            name: "explorer".to_string(),
            purpose: "Rapid repository and dependency investigator reading files, symbols, documentation, and references.".to_string(),
            primary_responsibilities: vec![
                "Fast reconnaissance of repository architecture, directory layout, and build manifests.".to_string(),
                "Tracing symbol definitions, call sites, implementations, and dependencies.".to_string(),
                "Extracting targeted evidence, configuration values, and usage patterns.".to_string(),
                "Mapping relevant implementation paths for task execution.".to_string(),
            ],
            explicit_scope: "Read-only discovery and information gathering. Does not modify codebase files.".to_string(),
            allowed_activities: vec![
                "Search files, grep symbols, list directories, read source files, and view git log.".to_string(),
                "Summarize component relationships and locate entry points.".to_string(),
            ],
            forbidden_activities: vec![
                "Mutating files, writing patches, or running modifying commands.".to_string(),
                "Executing long-running builds or benchmarks.".to_string(),
                "Speculating about file contents without reading them.".to_string(),
            ],
            engineering_standards: vec![
                "Provide exact file paths and line ranges for all discovered evidence.".to_string(),
                "Be concise, dense, and factual; avoid conversational filler.".to_string(),
            ],
            expected_deliverables: vec![
                "Structured discovery report with exact file locations, symbol definitions, and caller hierarchy.".to_string(),
            ],
            verification_responsibility: "Confirm symbols and file paths exist before citing them in discovery reports.".to_string(),
            completion_criteria: vec![
                "All targeted symbols, files, or questions are resolved with verified citations.".to_string(),
            ],
            delegation_restrictions: None,
        };

        self.roles.insert(
            "explorer".to_string(),
            AgentRole {
                name: "explorer".to_string(),
                description: "Rapid repository and dependency investigator reading files, symbols, and documentation.".to_string(),
                model_override: Some("meta/llama-3.1-8b-instruct".to_string()),
                sandbox_policy: SandboxPolicy::ReadOnly,
                temperature: Some(0.2),
                system_prompt_overlay: Some("You are acting as Fast Codebase Explorer. Navigate repository hierarchies, locate symbol definitions, and extract targeted evidence rapidly.".to_string()),
                nickname_candidates: vec!["Magellan".to_string(), "Cook".to_string(), "Voyager".to_string()],
                contract: Some(explorer_contract),
            },
        );

        // 5. Debugger
        let debugger_contract = AgentRoleContract {
            name: "debugger".to_string(),
            purpose: "Root-cause diagnosis, test reproducer construction, hypothesis validation, and surgical defect resolution.".to_string(),
            primary_responsibilities: vec![
                "Reproducing reported defects with minimal, deterministic test reproducers.".to_string(),
                "Isolating root causes through systematic hypothesis formulation and testing.".to_string(),
                "Analyzing stack traces, core dumps, logs, and diagnostic telemetry.".to_string(),
                "Implementing surgical bug fixes that address root causes without side effects.".to_string(),
            ],
            explicit_scope: "Targeted debugging and bug fixing within the failing subsystem.".to_string(),
            allowed_activities: vec![
                "Read source files, run debuggers, inspect test output, and execute focused tests.".to_string(),
                "Create minimal reproducing tests in the test suite.".to_string(),
                "Apply surgical fixes to eliminate root cause defects.".to_string(),
            ],
            forbidden_activities: vec![
                "Applying speculative broad refactorings instead of diagnosing root cause.".to_string(),
                "Suppressing error messages or swallowing exceptions to hide failures.".to_string(),
                "Deleting failing tests without proving they were invalid.".to_string(),
            ],
            engineering_standards: vec![
                "First reproduce, then isolate, then fix, then verify regression prevention.".to_string(),
                "Every bug fix must be accompanied by a regression test.".to_string(),
            ],
            expected_deliverables: vec![
                "Diagnostic root cause analysis explaining why the failure occurred.".to_string(),
                "Surgical bug fix addressing the root cause.".to_string(),
                "Passing regression test proving the defect is resolved.".to_string(),
            ],
            verification_responsibility: "Verify reproducer fails before fix and passes after fix; run neighborhood test suite.".to_string(),
            completion_criteria: vec![
                "Root cause identified and documented.".to_string(),
                "Bug fix verified by passing regression test.".to_string(),
                "No regressions introduced in related test suites.".to_string(),
            ],
            delegation_restrictions: None,
        };

        self.roles.insert(
            "debugger".to_string(),
            AgentRole {
                name: "debugger".to_string(),
                description: "Root cause diagnosis and surgical defect resolution specialist.".to_string(),
                model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                sandbox_policy: SandboxPolicy::WorkspaceWrite,
                temperature: Some(0.1),
                system_prompt_overlay: Some("You are acting as Diagnostic Debugger. Isolate root causes with minimal reproducers before making surgical, verified fixes.".to_string()),
                nickname_candidates: vec!["Holmes".to_string(), "Watson".to_string(), "House".to_string()],
                contract: Some(debugger_contract),
            },
        );

        // 6. Tester
        let tester_contract = AgentRoleContract {
            name: "tester".to_string(),
            purpose: "Adversarial test suite engineering, test coverage expansion, edge-case probing, and regression prevention.".to_string(),
            primary_responsibilities: vec![
                "Writing comprehensive unit, integration, and property-based tests.".to_string(),
                "Probing boundary conditions, malformed inputs, concurrency edges, and error paths.".to_string(),
                "Eliminating test coverage gaps across critical business logic.".to_string(),
                "Ensuring tests are deterministic, hermetic, and fast.".to_string(),
            ],
            explicit_scope: "Test suite implementation and test harness enhancement.".to_string(),
            allowed_activities: vec![
                "Create and update test files, fixtures, mock data, and test harnesses.".to_string(),
                "Execute test runners (`cargo test`, `npm test`, `pytest`) and inspect coverage.".to_string(),
            ],
            forbidden_activities: vec![
                "Weakening existing assertions to artificially achieve passing status.".to_string(),
                "Writing vacuous tests that assert trivial truth (`assert!(true)`).".to_string(),
                "Introducing flaky tests dependent on sleep delays or shared state.".to_string(),
            ],
            engineering_standards: vec![
                "Follow standard Arrange-Act-Assert structure with meaningful assertion failure messages.".to_string(),
                "Test both happy paths and explicit error conditions.".to_string(),
                "Keep tests isolated without cross-test state leakage.".to_string(),
            ],
            expected_deliverables: vec![
                "High-quality test suites with clear coverage of edge cases and error paths.".to_string(),
                "Execution proof demonstrating all tests run and pass cleanly.".to_string(),
            ],
            verification_responsibility: "Execute all newly added and affected tests; verify failure detection on intentional mutations.".to_string(),
            completion_criteria: vec![
                "All planned test scenarios are implemented and passing deterministically.".to_string(),
            ],
            delegation_restrictions: None,
        };

        self.roles.insert(
            "tester".to_string(),
            AgentRole {
                name: "tester".to_string(),
                description: "Test engineering and coverage specialist probing edge cases and regression scenarios.".to_string(),
                model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                sandbox_policy: SandboxPolicy::WorkspaceWrite,
                temperature: Some(0.1),
                system_prompt_overlay: Some("You are acting as Test Engineer. Design rigorous, deterministic tests probing edge cases, failure modes, and contract boundaries.".to_string()),
                nickname_candidates: vec!["Euler".to_string(), "Fermat".to_string(), "Gauss".to_string()],
                contract: Some(tester_contract),
            },
        );

        // 7. Security Analyst
        let security_contract = AgentRoleContract {
            name: "security_analyst".to_string(),
            purpose: "Vulnerability analysis, threat modeling, injection surface auditing, credential leakage prevention, and secure design review.".to_string(),
            primary_responsibilities: vec![
                "Auditing attack surfaces: input validation, deserialization, command injection, path traversal.".to_string(),
                "Verifying authentication, authorization, session lifecycle, and cryptographic hygiene.".to_string(),
                "Preventing secret, API key, and credential leakage in code, logs, and artifacts.".to_string(),
                "Auditing third-party dependencies for known vulnerabilities and supply-chain risks.".to_string(),
            ],
            explicit_scope: "Read-only security auditing of workspace source code and dependencies.".to_string(),
            allowed_activities: vec![
                "Inspect source files, dependency manifests, security advisories, and configuration.".to_string(),
                "Run static security analyzers and vulnerability scanners in read-only mode.".to_string(),
                "Produce threat models and structured vulnerability reports (OWASP / CWE mapping).".to_string(),
            ],
            forbidden_activities: vec![
                "Mutating source code or configurations directly.".to_string(),
                "Downgrading security invariants or approval boundaries.".to_string(),
                "Executing invasive exploitation against external endpoints.".to_string(),
            ],
            engineering_standards: vec![
                "Map every finding to established standards (CWE, OWASP Top 10, CVSS).".to_string(),
                "Provide concrete remediation guidance for every identified vulnerability.".to_string(),
            ],
            expected_deliverables: vec![
                "Security audit report with categorized findings, attack vectors, and remediation steps.".to_string(),
            ],
            verification_responsibility: "Validate threat paths against actual source code logic before reporting.".to_string(),
            completion_criteria: vec![
                "Declared attack surface thoroughly evaluated with verified findings.".to_string(),
            ],
            delegation_restrictions: Some("Security conclusions and risk ratings must be produced directly.".to_string()),
        };

        self.roles.insert(
            "security_analyst".to_string(),
            AgentRole {
                name: "security_analyst".to_string(),
                description: "Security and vulnerability auditor inspecting codebases for CWEs, CVEs, and credential leaks.".to_string(),
                model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                sandbox_policy: SandboxPolicy::ReadOnly,
                temperature: Some(0.1),
                system_prompt_overlay: Some("You are acting as Security Analyst. Audit code defensively for injection vectors, secret exposure, auth bypasses, and supply-chain risks.".to_string()),
                nickname_candidates: vec!["Mitnick".to_string(), "Schneier".to_string(), "Diffie".to_string()],
                contract: Some(security_contract.clone()),
            },
        );
        // Alias
        self.roles.insert("security-analyst".to_string(), AgentRole {
            name: "security_analyst".to_string(),
            description: "Security and vulnerability auditor inspecting codebases for CWEs, CVEs, and credential leaks.".to_string(),
            model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
            sandbox_policy: SandboxPolicy::ReadOnly,
            temperature: Some(0.1),
            system_prompt_overlay: Some("You are acting as Security Analyst. Audit code defensively for injection vectors, secret exposure, auth bypasses, and supply-chain risks.".to_string()),
            nickname_candidates: vec!["Mitnick".to_string(), "Schneier".to_string(), "Diffie".to_string()],
            contract: Some(security_contract),
        });

        // 8. Performance Engineer
        let perf_contract = AgentRoleContract {
            name: "performance_engineer".to_string(),
            purpose: "Latency, throughput, allocation profiling, algorithmic complexity analysis, and performance optimization.".to_string(),
            primary_responsibilities: vec![
                "Identifying algorithmic complexity bottlenecks and inefficient hot loops.".to_string(),
                "Profiling heap allocations, memory leaks, and unnecessary buffer cloning.".to_string(),
                "Analyzing concurrency contention, lock wait times, and async executor bottlenecks.".to_string(),
                "Benchmarking before and after changes with reproducible performance evidence.".to_string(),
            ],
            explicit_scope: "Performance analysis, profiling, and targeted optimization within declared scope.".to_string(),
            allowed_activities: vec![
                "Read and profile source code, run micro-benchmarks and load tests.".to_string(),
                "Apply surgical optimizations that demonstrably reduce latency or memory consumption.".to_string(),
            ],
            forbidden_activities: vec![
                "Applying premature micro-optimizations that sacrifice readability without evidence.".to_string(),
                "Altering public API contracts or semantics in the name of optimization.".to_string(),
                "Claiming performance improvements without before/after benchmark measurements.".to_string(),
            ],
            engineering_standards: vec![
                "Measure first: establish baseline before modifying code.".to_string(),
                "Preserve correctness and safety invariants unconditionally over speed.".to_string(),
            ],
            expected_deliverables: vec![
                "Performance analysis with flamegraph/profiling evidence and concrete bottleneck identification.".to_string(),
                "Optimized code with before/after benchmark comparison.".to_string(),
            ],
            verification_responsibility: "Run benchmarks under identical environments; verify zero test regression.".to_string(),
            completion_criteria: vec![
                "Measurable performance gain achieved or bottleneck conclusively characterized.".to_string(),
                "Full functional test suite passes without regression.".to_string(),
            ],
            delegation_restrictions: None,
        };

        self.roles.insert(
            "performance_engineer".to_string(),
            AgentRole {
                name: "performance_engineer".to_string(),
                description: "Performance and efficiency specialist analyzing latency, throughput, and memory allocations.".to_string(),
                model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                sandbox_policy: SandboxPolicy::WorkspaceWrite,
                temperature: Some(0.1),
                system_prompt_overlay: Some("You are acting as Performance Engineer. Profile execution hot paths and apply measured, evidence-backed optimizations.".to_string()),
                nickname_candidates: vec!["Dijkstra".to_string(), "Knuth".to_string(), "Shannon".to_string()],
                contract: Some(perf_contract.clone()),
            },
        );
        // Alias
        self.roles.insert("performance-engineer".to_string(), AgentRole {
            name: "performance_engineer".to_string(),
            description: "Performance and efficiency specialist analyzing latency, throughput, and memory allocations.".to_string(),
            model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
            sandbox_policy: SandboxPolicy::WorkspaceWrite,
            temperature: Some(0.1),
            system_prompt_overlay: Some("You are acting as Performance Engineer. Profile execution hot paths and apply measured, evidence-backed optimizations.".to_string()),
            nickname_candidates: vec!["Dijkstra".to_string(), "Knuth".to_string(), "Shannon".to_string()],
            contract: Some(perf_contract),
        });
    }

    /// Loads custom user and workspace agent roles from disk (`.m31a/roles/*.toml`).
    pub fn load_custom_roles(&mut self, cwd: &Path) {
        let mut search_dirs = Vec::new();
        search_dirs.push(cwd.join(".m31a").join("roles"));

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        search_dirs.push(
            PathBuf::from(home)
                .join(".config")
                .join("m31a")
                .join("roles"),
        );

        for dir in search_dirs {
            if dir.is_dir()
                && let Ok(entries) = fs::read_dir(dir)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("toml")
                        && let Ok(content) = fs::read_to_string(&path)
                        && let Ok(role) = toml::from_str::<AgentRole>(&content)
                    {
                        self.roles.insert(role.name.to_ascii_lowercase(), role);
                    }
                }
            }
        }
    }

    pub fn get_role(&self, name: &str) -> Option<&AgentRole> {
        let key = name.to_ascii_lowercase();
        self.roles.get(&key).or_else(|| {
            let alt = key.replace('_', "-");
            self.roles.get(&alt).or_else(|| {
                let alt2 = key.replace('-', "_");
                self.roles.get(&alt2)
            })
        })
    }

    pub fn get(&self, name: &str) -> Option<&AgentRole> {
        self.get_role(name)
    }

    pub fn list_roles(&self) -> Vec<&AgentRole> {
        let mut seen = std::collections::HashSet::new();
        let mut list: Vec<&AgentRole> = Vec::new();
        for r in self.roles.values() {
            if seen.insert(&r.name) {
                list.push(r);
            }
        }
        list.sort_by_key(|r| &r.name);
        list
    }

    /// Formats an actionable prompt block explaining available agent roles for subagent delegation.
    pub fn render_roles_prompt(&self) -> String {
        let mut out = String::from("## Available Specialized Subagent Roles\n\n");
        out.push_str("When spawning background subagents via `spawn_agent`, assign the most appropriate role:\n");
        for role in self.list_roles() {
            let model_str = role.model_override.as_deref().unwrap_or("inherit");
            out.push_str(&format!(
                "- **{}** (model: `{model_str}`, sandbox: `{:?}`): {}\n",
                role.name, role.sandbox_policy, role.description
            ));
        }
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_agent_roles() {
        let registry = AgentRoleRegistry::new();
        let architect = registry.get_role("architect").unwrap();
        assert_eq!(architect.sandbox_policy, SandboxPolicy::ReadOnly);
        assert!(architect.description.contains("architect"));
        assert!(architect.contract.is_some());

        let coder = registry.get_role("coder").unwrap();
        assert_eq!(coder.sandbox_policy, SandboxPolicy::WorkspaceWrite);
        assert!(coder.contract.is_some());

        let reviewer = registry.get_role("reviewer").unwrap();
        assert_eq!(reviewer.sandbox_policy, SandboxPolicy::ReadOnly);
        assert!(reviewer.contract.is_some());

        let explorer = registry.get_role("explorer").unwrap();
        assert!(
            explorer
                .nickname_candidates
                .contains(&"Voyager".to_string())
        );

        let debugger = registry.get_role("debugger").unwrap();
        assert_eq!(debugger.sandbox_policy, SandboxPolicy::WorkspaceWrite);

        let tester = registry.get_role("tester").unwrap();
        assert!(tester.contract.is_some());

        let sec = registry.get_role("security_analyst").unwrap();
        assert_eq!(sec.sandbox_policy, SandboxPolicy::ReadOnly);

        let perf = registry.get_role("performance_engineer").unwrap();
        assert!(perf.contract.is_some());
    }

    #[test]
    fn test_render_roles_prompt() {
        let registry = AgentRoleRegistry::new();
        let prompt = registry.render_roles_prompt();
        assert!(prompt.contains("Available Specialized Subagent Roles"));
        assert!(prompt.contains("architect"));
        assert!(prompt.contains("coder"));
        assert!(prompt.contains("reviewer"));
        assert!(prompt.contains("debugger"));
        assert!(prompt.contains("tester"));
    }

    #[test]
    fn test_render_contract_markdown() {
        let registry = AgentRoleRegistry::new();
        let architect = registry.get_role("architect").unwrap();
        let contract = architect.effective_contract();
        let md = contract.render_contract_markdown();
        assert!(md.contains("## Active Agent Role Contract: `architect`"));
        assert!(md.contains("### Primary Responsibilities"));
        assert!(md.contains("### Forbidden Activities"));
        assert!(md.contains("Never mutates codebase files directly"));
    }

    #[test]
    fn test_custom_role_effective_contract_fallback() {
        let custom = AgentRole {
            name: "custom_worker".to_string(),
            description: "Specialized custom task worker.".to_string(),
            model_override: None,
            sandbox_policy: SandboxPolicy::ReadOnly,
            temperature: Some(0.2),
            system_prompt_overlay: Some("Custom instructions for worker.".to_string()),
            nickname_candidates: vec![],
            contract: None,
        };
        let contract = custom.effective_contract();
        assert_eq!(contract.name, "custom_worker");
        assert!(contract.purpose.contains("Specialized custom task worker"));
        let md = contract.render_contract_markdown();
        assert!(md.contains("Custom instructions for worker"));
    }
}
