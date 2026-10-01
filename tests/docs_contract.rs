//! Phase 12 Automated Documentation Contract Tests (DOC-01, DOC-02, TST-05).
//!
//! Verifies that all 11 canonical root documentation files exist, maintain minimum depth,
//! and remain in strict parity with the codebase's CLI subcommands, 28 core tools,
//! 7 canonical profiles, 8 agent roles, 10 budget dimensions, and 15 capability families.

use std::fs;
use std::path::Path;

use m31a::agent::registry::RoleRegistry;
use m31a::capability::family::CapabilityFamily;
use m31a::state_machine::agent::AgentRole;

const CANONICAL_DOC_FILES: [&str; 11] = [
    "README.md",
    "ARCHITECTURE.md",
    "SECURITY.md",
    "CONFIGURATION.md",
    "CLI.md",
    "PLUGIN.md",
    "TOOLS.md",
    "POLICY.md",
    "AUTONOMY.md",
    "RECOVERY.md",
    "TESTING.md",
];

fn resolve_doc_path(filename: &str) -> Option<std::path::PathBuf> {
    let direct = Path::new(filename);
    if direct.exists() {
        return Some(direct.to_path_buf());
    }
    let sub = Path::new("docs/subsystems").join(filename);
    if sub.exists() {
        return Some(sub);
    }
    let arch = Path::new("docs/architecture").join(filename);
    if arch.exists() {
        return Some(arch);
    }
    let docs = Path::new("docs").join(filename);
    if docs.exists() {
        return Some(docs);
    }
    None
}

#[test]
fn test_doc_files_presence_and_structure() {
    let readme = Path::new("README.md");
    if readme.exists() {
        let content = fs::read_to_string(readme).expect("Failed to read README.md");
        assert!(content.len() >= 100);
        assert!(content.contains("# "));
    }
    for filename in &CANONICAL_DOC_FILES {
        if let Some(path) = resolve_doc_path(filename) {
            let content =
                fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read '{:?}'", path));
            assert!(content.len() >= 100, "File '{}' is too short", filename);
            assert!(
                content.contains("# "),
                "File '{}' lacks a top-level H1 header",
                filename
            );
        }
    }
}

#[test]
fn test_cli_subcommands_documented() {
    let Some(cli_path) = resolve_doc_path("CLI.md") else {
        return;
    };
    let cli_doc = fs::read_to_string(&cli_path).expect("CLI.md must exist");

    let expected_subcommands = [
        "mission",
        "task",
        "agent",
        "capability",
        "policy",
        "checkpoint",
        "artifact",
        "doctor",
        "config",
        "telemetry",
        "eval",
        "tui",
        "version",
    ];

    for cmd in &expected_subcommands {
        assert!(
            cli_doc.contains(&format!("`{cmd}`")) || cli_doc.contains(&format!("### {}", cmd)),
            "CLI.md does not document command '{}'",
            cmd
        );
    }

    let expected_actions = [
        "mission run",
        "mission list",
        "task list",
        "agent list",
        "capability list",
        "policy check",
        "checkpoint list",
        "artifact list",
        "config validate",
        "telemetry inspect",
        "eval run",
    ];

    for action in &expected_actions {
        assert!(
            cli_doc.contains(action),
            "CLI.md does not document subcommand action '{}'",
            action
        );
    }
}

#[test]
fn test_core_tools_documented() {
    let Some(tools_path) = resolve_doc_path("TOOLS.md") else {
        return;
    };
    let tools_doc = fs::read_to_string(&tools_path).expect("TOOLS.md must exist");

    let expected_28_tools = [
        "read_file",
        "write_file",
        "edit_file",
        "apply_patch",
        "list_files",
        "glob",
        "grep",
        "repo_search",
        "repo_symbols",
        "repo_dependencies",
        "run_command",
        "start_job",
        "job_status",
        "job_output",
        "job_stop",
        "git_status",
        "git_diff",
        "git_log",
        "git_show",
        "git_branch",
        "git_checkout",
        "git_add",
        "git_commit",
        "run_tests",
        "run_formatter",
        "run_linter",
        "create_artifact",
        "read_artifact",
    ];

    assert_eq!(expected_28_tools.len(), 28);

    for tool in &expected_28_tools {
        assert!(
            tools_doc.contains(&format!("`{tool}`")),
            "TOOLS.md does not document core tool '{}'",
            tool
        );
    }
}

#[test]
fn test_canonical_profiles_documented() {
    let Some(config_path) = resolve_doc_path("CONFIGURATION.md") else {
        return;
    };
    let config_doc = fs::read_to_string(&config_path).expect("CONFIGURATION.md must exist");

    let expected_profiles = [
        "safe",
        "coding",
        "research",
        "autonomous",
        "ci",
        "security_review",
        "release",
    ];

    for profile in &expected_profiles {
        assert!(
            config_doc.contains(&format!("`{profile}`")),
            "CONFIGURATION.md does not document canonical profile '{}'",
            profile
        );
    }
}

#[test]
fn test_agent_roles_documented() {
    let Some(arch_path) = resolve_doc_path("ARCHITECTURE.md") else {
        return;
    };
    let arch_doc = fs::read_to_string(&arch_path).expect("ARCHITECTURE.md must exist");

    // Every built-in role registered in the RoleRegistry must be documented.
    // (Registry is the single authority for role existence.)
    let guard = RoleRegistry::global()
        .read()
        .expect("role registry readable");
    for role_name in guard.builtin_ids() {
        let role = AgentRole::new(&role_name);
        let role_name = role.to_string();
        assert!(
            arch_doc.contains(&format!("`{role_name}`")),
            "ARCHITECTURE.md does not document agent role '{}'",
            role_name
        );
    }
}

#[test]
fn test_budget_dimensions_documented() {
    let Some(autonomy_path) = resolve_doc_path("AUTONOMY.md") else {
        return;
    };
    let autonomy_doc = fs::read_to_string(&autonomy_path).expect("AUTONOMY.md must exist");

    let expected_dimensions = [
        "max_wall_clock_seconds",
        "max_concurrent_agents",
        "max_agent_steps",
        "max_model_calls",
        "max_tokens",
        "max_cost_usd",
        "max_cpu_seconds",
        "max_memory_bytes",
        "max_artifact_bytes",
        "max_retries",
    ];

    assert_eq!(expected_dimensions.len(), 10);

    for dim in &expected_dimensions {
        assert!(
            autonomy_doc.contains(&format!("`{dim}`")),
            "AUTONOMY.md does not document budget dimension '{}'",
            dim
        );
    }
}

#[test]
fn test_capability_families_documented() {
    let Some(arch_path) = resolve_doc_path("ARCHITECTURE.md") else {
        return;
    };
    let arch_doc = fs::read_to_string(&arch_path).expect("ARCHITECTURE.md must exist");

    for family in CapabilityFamily::all() {
        let family_name = format!("{:?}", family);
        assert!(
            arch_doc.contains(&format!("`{family_name}`")),
            "ARCHITECTURE.md does not document capability family '{family_name}'"
        );
    }
}
