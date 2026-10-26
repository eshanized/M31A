//! Phase 12 Automated Documentation Contract Tests (DOC-01, DOC-02, TST-05).
//!
//! Verifies that all 11 canonical root documentation files exist, maintain minimum depth,
//! and remain in strict parity with the codebase's CLI subcommands, 28 core tools,
//! 7 canonical profiles, 8 agent roles, and 15 capability families.

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
    assert!(readme.exists(), "README.md must exist in repository root");
    let content = fs::read_to_string(readme).expect("Failed to read README.md");
    assert!(content.len() >= 100);
    assert!(content.contains("# "));

    for filename in &CANONICAL_DOC_FILES {
        let path = resolve_doc_path(filename)
            .unwrap_or_else(|| panic!("Canonical documentation file '{}' must exist", filename));
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

#[test]
fn test_cli_subcommands_documented() {
    let cli_path = resolve_doc_path("CLI.md")
        .expect("CLI.md must exist in canonical documentation directories");
    let cli_doc = fs::read_to_string(&cli_path).expect("CLI.md must be readable");

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
    let tools_path = resolve_doc_path("TOOLS.md")
        .expect("TOOLS.md must exist in canonical documentation directories");
    let tools_doc = fs::read_to_string(&tools_path).expect("TOOLS.md must be readable");

    // Cross-verify against runtime ToolRegistry
    let cap_reg = std::sync::Arc::new(m31a::capability::registry::CapabilityRegistry::new());
    let tool_reg = m31a::tools::registry::ToolRegistry::new_default(cap_reg);
    assert_eq!(
        tool_reg.len(),
        28,
        "ToolRegistry must register exactly 28 core tools"
    );

    for tool in tool_reg.list_tools() {
        let tool_name = tool.id();
        assert!(
            tools_doc.contains(&format!("`{tool_name}`")),
            "TOOLS.md does not document core tool from ToolRegistry '{}'",
            tool_name
        );
    }
}

#[test]
fn test_canonical_profiles_documented() {
    let config_path = resolve_doc_path("CONFIGURATION.md")
        .expect("CONFIGURATION.md must exist in canonical documentation directories");
    let config_doc = fs::read_to_string(&config_path).expect("CONFIGURATION.md must be readable");

    let profile_resolver = m31a::config::profile::ProfileResolver::with_canonical_profiles();
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
            profile_resolver.resolve_profile(profile).is_ok(),
            "ProfileResolver must contain canonical profile '{profile}'"
        );
        assert!(
            config_doc.contains(&format!("`{profile}`")),
            "CONFIGURATION.md does not document canonical profile '{}'",
            profile
        );
    }
}

#[test]
fn test_agent_roles_documented() {
    let arch_path = resolve_doc_path("ARCHITECTURE.md")
        .expect("ARCHITECTURE.md must exist in canonical documentation directories");
    let arch_doc = fs::read_to_string(&arch_path).expect("ARCHITECTURE.md must be readable");

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
fn test_budget_system_documented_as_decommissioned() {
    let autonomy_path = resolve_doc_path("AUTONOMY.md")
        .expect("AUTONOMY.md must exist in canonical documentation directories");
    let autonomy_doc = fs::read_to_string(&autonomy_path).expect("AUTONOMY.md must be readable");

    assert!(
        autonomy_doc.contains("Budget System Decommissioned")
            || autonomy_doc.contains("no budget governance")
            || autonomy_doc.contains("unconstrained"),
        "AUTONOMY.md must document that the budget system is removed"
    );
}

#[test]
fn test_capability_families_documented() {
    let arch_path = resolve_doc_path("ARCHITECTURE.md")
        .expect("ARCHITECTURE.md must exist in canonical documentation directories");
    let arch_doc = fs::read_to_string(&arch_path).expect("ARCHITECTURE.md must be readable");

    for family in CapabilityFamily::all() {
        let family_name = format!("{:?}", family);
        assert!(
            arch_doc.contains(&format!("`{family_name}`")),
            "ARCHITECTURE.md does not document capability family '{family_name}'"
        );
    }
}

#[test]
fn test_all_documentation_relative_links_resolve() {
    let re = regex::Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
    let mut broken_links = Vec::new();

    let doc_dirs = ["docs", "."];
    for base in doc_dirs {
        for entry in walkdir(Path::new(base)) {
            let path = entry.as_path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let content = match fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let parent = path.parent().unwrap_or(Path::new("."));

                for cap in re.captures_iter(&content) {
                    let link = &cap[2];
                    if link.starts_with("http://")
                        || link.starts_with("https://")
                        || link.starts_with('#')
                        || link.starts_with("mailto:")
                    {
                        continue;
                    }
                    let target_file = link.split('#').next().unwrap_or("").trim();
                    if target_file.is_empty() {
                        continue;
                    }
                    let resolved = parent.join(target_file);
                    if !resolved.exists() {
                        broken_links.push(format!(
                            "In {}: link '{}' -> resolved '{:?}' not found",
                            path.display(),
                            link,
                            resolved
                        ));
                    }
                }
            }
        }
    }

    assert!(
        broken_links.is_empty(),
        "Detected broken documentation links:\n{}",
        broken_links.join("\n")
    );
}

#[test]
fn test_no_obsolete_phase_test_filenames() {
    let mut violations = Vec::new();
    let tests_dir = Path::new("tests");
    if let Ok(entries) = fs::read_dir(tests_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                if file_name.starts_with("phase_") || file_name.starts_with("promptos_phase") {
                    violations.push(format!("tests/{}", file_name));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Obsolete phase-numbered test filenames detected in tests/:\n{}\nAll test files must use descriptive, subsystem-oriented names.",
        violations.join("\n")
    );
}

#[test]
fn test_testing_docs_references_resolve() {
    let testing_doc_path = Path::new("docs/subsystems/TESTING.md");
    assert!(
        testing_doc_path.exists(),
        "docs/subsystems/TESTING.md must exist in canonical documentation directories"
    );
    let content = fs::read_to_string(testing_doc_path).expect("TESTING.md must be readable");
    let re = regex::Regex::new(r"`([a-z0-9_]+\.rs)`").unwrap();
    let mut missing = Vec::new();

    for cap in re.captures_iter(&content) {
        let filename = &cap[1];
        let test_path = Path::new("tests").join(filename);
        if !test_path.exists() {
            missing.push(format!(
                "In TESTING.md: `{filename}` does not exist in tests/"
            ));
        }
    }

    assert!(
        missing.is_empty(),
        "Detected stale test references in docs/subsystems/TESTING.md:\n{}",
        missing.join("\n")
    );
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "target" || name == "tmp" || name == "dist" {
                continue;
            }
            if p.is_dir() {
                if dir == Path::new(".") && name != "docs" {
                    continue;
                }
                files.extend(walkdir(&p));
            } else {
                files.push(p);
            }
        }
    }
    files
}
