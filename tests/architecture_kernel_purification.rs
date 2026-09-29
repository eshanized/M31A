//! Phase 18 — Kernel Contract Purification & Dependency Cycle Elimination Tests
//!
//! Enforces:
//! 1. Kernel contracts must not depend upward into domain implementations.
//! 2. Context must not depend on Agent.
//! 3. Process must not depend on Capability.
//! 4. Telemetry must not depend on CLI.
//! 5. Kernel defines and owns pure contracts (plan, invariants, seams).
//! 6. Zero cycles involve the kernel module.

use std::fs;
use std::path::Path;

fn collect_rs_files(dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Some(p) = path.to_str()
            {
                out.push(p.to_string());
            }
        }
    }
}

fn find_violations(dir: &str, forbidden_patterns: &[&str]) -> Vec<(String, String, usize, String)> {
    let mut files = Vec::new();
    let root = Path::new(dir);
    if root.exists() {
        collect_rs_files(root, &mut files);
    }

    let mut violations = Vec::new();
    for file in files {
        if let Ok(content) = fs::read_to_string(&file) {
            for (line_no, line) in content.lines().enumerate() {
                for pattern in forbidden_patterns {
                    if line.contains(pattern) {
                        violations.push((
                            file.clone(),
                            pattern.to_string(),
                            line_no + 1,
                            line.to_string(),
                        ));
                    }
                }
            }
        }
    }
    violations
}

#[test]
fn test_kernel_has_zero_upward_dependencies() {
    let forbidden = [
        "crate::context::",
        "crate::planning::",
        "crate::policy::",
        "crate::scheduler::",
        "crate::agent::",
        "crate::state::",
        "crate::persistence::",
        "crate::dag::",
        "crate::pipeline::",
        "crate::capability::",
        "crate::process::",
        "crate::tools::",
        "crate::sandbox::",
        "crate::cli::",
        "crate::tui::",
        "crate::controller::",
        "crate::runtime::",
        "crate::workflow::",
        "crate::verification::",
        "use crate::context",
        "use crate::planning",
        "use crate::policy",
        "use crate::scheduler",
        "use crate::agent",
        "use crate::state::",
        "use crate::persistence",
        "use crate::dag",
        "use crate::pipeline",
        "use crate::capability",
        "use crate::process",
        "use crate::tools",
        "use crate::sandbox",
        "use crate::cli",
        "use crate::tui",
        "use crate::controller",
        "use crate::runtime",
        "use crate::workflow",
        "use crate::verification",
    ];

    let violations = find_violations("src/kernel", &forbidden);
    if !violations.is_empty() {
        let details: Vec<String> = violations
            .iter()
            .map(|(f, p, ln, l)| format!("  {}:{} — pattern '{}' found in: {}", f, ln, p, l.trim()))
            .collect();
        panic!(
            "Phase 18 Violation: src/kernel imports upward domain modules:\n{}",
            details.join("\n")
        );
    }
}

#[test]
fn test_context_does_not_depend_on_agent() {
    let forbidden = ["crate::agent", "use crate::agent"];
    let violations = find_violations("src/context", &forbidden);
    assert!(
        violations.is_empty(),
        "Phase 18 Violation: src/context must not depend on crate::agent: {:?}",
        violations
    );
}

#[test]
fn test_process_does_not_depend_on_capability() {
    let forbidden = ["crate::capability", "use crate::capability"];
    let violations = find_violations("src/process", &forbidden);
    assert!(
        violations.is_empty(),
        "Phase 18 Violation: src/process must not depend on crate::capability: {:?}",
        violations
    );
}

#[test]
fn test_telemetry_does_not_depend_on_cli() {
    let forbidden = ["crate::cli", "use crate::cli"];
    let violations = find_violations("src/telemetry", &forbidden);
    assert!(
        violations.is_empty(),
        "Phase 18 Violation: src/telemetry must not depend on crate::cli: {:?}",
        violations
    );
}

#[test]
fn test_kernel_plan_types_accessible() {
    use m31a::kernel::plan::{
        CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode,
        CapabilityRequirement, ResourceEstimate, TaskResult, VerificationStrategy,
    };

    let plan = CandidatePlan {
        plan_id: "test-plan".into(),
        objective: "test objective".into(),
        tasks: vec![CandidateTask {
            id: CandidateTaskKey("task-1".into()),
            objective: "Task 1".into(),
            description: Some("A candidate task".into()),
            role: m31a::state_machine::agent::AgentRole::implementer(),
            depends_on: vec![],
            capabilities: vec![CapabilityRequirement::new(
                "fs.read",
                CapabilityAccessMode::Read,
            )],
            verification: VerificationStrategy::AutomatedTest {
                command: Some("cargo check".into()),
            },
            estimates: ResourceEstimate::default(),
            ..Default::default()
        }],
        created_at: chrono::Utc::now(),
        ..Default::default()
    };

    assert_eq!(plan.tasks.len(), 1);
    assert_eq!(plan.tasks[0].id.as_str(), "task-1");

    let result = TaskResult {
        summary: "Task executed successfully".into(),
        output_artifacts: vec![],
        metadata: std::collections::HashMap::new(),
    };
    assert_eq!(result.summary, "Task executed successfully");
}

#[test]
fn test_kernel_path_protection_invariants() {
    use m31a::kernel::invariants::{contains_protected_component, is_protected_component};

    assert!(is_protected_component(".git"));
    assert!(is_protected_component(".m31a"));
    assert!(is_protected_component(".GIT"));
    assert!(is_protected_component(".M31A"));
    assert!(!is_protected_component("src"));
    assert!(!is_protected_component("git"));

    assert!(contains_protected_component(Path::new(".git/config")));
    assert!(contains_protected_component(Path::new(
        "subdir/.m31a/state.db"
    )));
    assert!(!contains_protected_component(Path::new("src/main.rs")));
}

#[test]
fn test_prompt_reference_for_all_roles() {
    use m31a::prompt::PromptReference;
    use m31a::state_machine::agent::AgentRole;

    let roles = [
        AgentRole::planner(),
        AgentRole::researcher(),
        AgentRole::architect(),
        AgentRole::implementer(),
        AgentRole::reviewer(),
        AgentRole::verifier(),
        AgentRole::diagnostician(),
        AgentRole::integrator(),
        AgentRole::discovery_analyst(),
        AgentRole::stack_researcher(),
        AgentRole::features_researcher(),
        AgentRole::architecture_researcher(),
        AgentRole::pitfalls_researcher(),
        AgentRole::security_researcher(),
        AgentRole::deployment_researcher(),
        AgentRole::synthesizer(),
    ];

    for role in roles {
        let p_ref = PromptReference::for_role(role);
        assert_eq!(p_ref.version, 1);
        assert!(p_ref.id.starts_with("agent."));
    }
}

#[test]
fn test_process_types_and_reexports() {
    use m31a::capability::traits::jobs::{JobDescriptor, JobOutputChunk, JobStatusInfo};
    use m31a::capability::traits::process::ProcessOutput;

    let out = ProcessOutput {
        exit_code: 0,
        stdout: "ok".into(),
        stderr: "".into(),
    };
    assert_eq!(out.exit_code, 0);

    let desc = JobDescriptor {
        job_id: "job-1".into(),
        command: "ls".into(),
        pid: Some(1234),
        started_at_ms: 1000,
    };
    assert_eq!(desc.job_id, "job-1");

    let status = JobStatusInfo {
        job_id: "job-1".into(),
        state: "Running".into(),
        exit_code: None,
        running_ms: 50,
    };
    assert_eq!(status.state, "Running");

    let chunk = JobOutputChunk {
        job_id: "job-1".into(),
        stdout: "hello".into(),
        stderr: "".into(),
        next_offset: 5,
        is_eof: true,
    };
    assert!(chunk.is_eof);
}

#[test]
fn test_telemetry_inspection_report_conversion() {
    use m31a::cli::dispatch::CliOutput;
    use m31a::telemetry::TelemetryInspectionReport;

    let report =
        TelemetryInspectionReport::success("inspection output", serde_json::json!({"total": 5}));
    assert_eq!(report.exit_code, 0);

    let cli_output = CliOutput::from(report);
    assert_eq!(cli_output.exit_code, 0);
    assert_eq!(cli_output.text, "inspection output");
    assert_eq!(cli_output.data["total"], 5);
}

#[test]
fn test_context_seam_contract_inversion() {
    use m31a::kernel::seams::context::{
        CompiledContext, ContextCompilationContract, ContextSectionContract,
    };

    let contract = ContextCompilationContract::new(
        1,
        100,
        20,
        80,
        vec![ContextSectionContract {
            section_id: "mission".into(),
            original_tokens: 40,
            final_tokens: 40,
            compaction_action: "retained_full".into(),
            provenance: "static".into(),
        }],
    );

    let compiled = CompiledContext {
        context_id: "ctx-1".into(),
        token_count: 80,
        system_prompt: "system".into(),
        messages: vec![],
        manifest: Some(contract),
    };

    assert_eq!(compiled.token_count, 80);
    let m = compiled.manifest.as_ref().unwrap();
    assert_eq!(m.sections.len(), 1);
    assert_eq!(m.sections[0].section_id, "mission");
}

#[test]
fn test_policy_seam_contract_inversion() {
    use m31a::kernel::seams::policy::{PolicyDecision, PolicyDecisionContract};

    let contract = PolicyDecisionContract {
        decision: PolicyDecision::Allow,
        matched_rule_id: Some("rule-1".into()),
        matched_layer: Some("workspace".into()),
        precedence_rank: Some(1),
        authority_source: "static_policy".into(),
        explanation: "safe read".into(),
        policy_version_or_hash: "v1".into(),
    };

    assert_eq!(contract.decision, PolicyDecision::Allow);
    assert_eq!(contract.authority_source, "static_policy");
}
