//! Phase 12 Skills Architecture & Canonical Profiles Suite (SKL-01, SKL-02, SKL-03).

use tempfile::tempdir;

use m31a::config::merge::ConfigError;
use m31a::config::profile::ProfileResolver;
use m31a::ids::TaskId;
use m31a::skill::compiler::SkillCompiler;
use m31a::skill::discovery::{SkillDiscovery, SkillOriginTier};
use m31a::skill::manifest::{SkillExecutionMode, SkillManifest, SkillVerificationSpec};
use m31a::skill::registry::SkillRegistry;
use m31a::skill::verification::SkillVerificationCompiler;
use m31a::verification::types::CheckTier;

#[test]
fn test_skill_manifest_validation() {
    let valid_toml = r#"
schema_version = 1
id = "audit-dependencies"
name = "Audit Dependencies"
version = "1.2.0"
description = "Scans cargo dependencies for known security vulnerabilities"
required_capabilities = ["workspace_fs_read", "shell_exec"]

[execution]
mode = "in_task"

[procedure]
instructions = "Run cargo audit and format security report"
steps = [
  { name = "audit", instruction = "cargo audit", allowed_tools = ["shell"] }
]

[verification]
tier = 4
commands = ["cargo audit"]
evidence_required = ["audit_report.json"]

[risk_profile]
level = "low"
requires_approval = false
"#;

    let manifest = SkillManifest::parse_toml(valid_toml).expect("Valid manifest parses");
    assert_eq!(manifest.id, "audit-dependencies");
    assert_eq!(manifest.verification.tier, 4);
    assert_eq!(manifest.execution.mode, SkillExecutionMode::InTask);

    // Rejects unknown keys
    let invalid_toml = r#"
schema_version = 1
id = "bad-skill"
name = "Bad"
version = "1.0.0"
description = "invalid"
unknown_field = 42

[execution]
mode = "in_task"

[procedure]
instructions = "test"

[verification]
tier = 1

[risk_profile]
level = "low"
"#;
    assert!(SkillManifest::parse_toml(invalid_toml).is_err());

    // Rejects unsupported schema versions
    let future_schema = valid_toml.replace("schema_version = 1", "schema_version = 2");
    assert!(SkillManifest::parse_toml(&future_schema).is_err());
}

#[test]
fn test_skill_discovery_hierarchy() {
    let dir = tempdir().unwrap();
    let ws_skills = dir.path().join(".m31a").join("skills");
    std::fs::create_dir_all(ws_skills.join("custom-fix")).unwrap();

    let ws_toml = r#"
schema_version = 1
id = "fix-test-failure"
name = "Workspace Custom Fix Test Failure"
version = "2.0.0"
description = "Overridden fix-test-failure with custom steps"
required_capabilities = ["workspace_fs_write", "compiler_exec"]

[execution]
mode = "in_task"

[procedure]
instructions = "Custom workspace instructions"

[verification]
tier = 4
commands = ["cargo test --all"]

[risk_profile]
level = "medium"
requires_approval = false
"#;
    std::fs::write(ws_skills.join("custom-fix").join("SKILL.toml"), ws_toml).unwrap();

    let discovered = SkillDiscovery::discover_all(Some(dir.path()));
    assert!(!discovered.is_empty());

    let mut registry = SkillRegistry::new();
    for pkg in discovered {
        registry.register(pkg).unwrap();
    }

    let resolved = registry.get("fix-test-failure").expect("found skill");
    // Workspace definition overrides Builtin definition because Workspace tier has higher precedence
    assert_eq!(resolved.origin, SkillOriginTier::Workspace);
    assert_eq!(resolved.manifest.name, "Workspace Custom Fix Test Failure");
    assert_eq!(resolved.manifest.verification.tier, 4);
}

#[test]
fn test_skill_subdag_compilation() {
    let toml_str = r#"
schema_version = 1
id = "pipeline-skill"
name = "Pipeline Skill"
version = "1.0.0"
description = "test subdag"

[execution]
mode = "sub_dag"

[procedure]
instructions = "Execute steps in order"
steps = [
  { name = "step_1", instruction = "Fetch data", allowed_tools = ["reader"] },
  { name = "step_2", instruction = "Process data", allowed_tools = ["writer"], dependencies = ["step_1"] }
]

[verification]
tier = 3

[risk_profile]
level = "low"
"#;

    let manifest = SkillManifest::parse_toml(toml_str).unwrap();
    let parent_id = TaskId::new();
    let subdag = SkillCompiler::compile_sub_dag(&manifest, parent_id).unwrap();

    assert_eq!(subdag.tasks.len(), 2);
    assert_eq!(subdag.edges.len(), 1);
    assert_eq!(subdag.tasks[0].name, "step_1");
    assert_eq!(subdag.tasks[1].name, "step_2");
    assert_eq!(subdag.tasks[1].dependencies[0], subdag.tasks[0].task_id);
}

#[test]
fn test_skill_verification_compilation() {
    let spec = SkillVerificationSpec {
        tier: 2,
        commands: vec!["cargo check".to_string()],
        evidence_required: vec!["compile.log".to_string()],
    };

    // Strengthening: Profile requires Tier 4, so effective tier becomes Tier 4
    let plan =
        SkillVerificationCompiler::compile("sample-skill", &spec, Some(CheckTier::StaticAnalysis))
            .unwrap();

    assert_eq!(plan.declared_tier, CheckTier::Compiler);
    assert_eq!(plan.effective_tier, CheckTier::StaticAnalysis);
    assert_eq!(plan.commands, vec!["cargo check"]);
}

#[test]
fn test_seven_canonical_profiles_loaded() {
    let resolver = ProfileResolver::with_canonical_profiles();

    let expected_profiles = [
        "safe",
        "coding",
        "research",
        "autonomous",
        "ci",
        "security_review",
        "release",
    ];

    for name in &expected_profiles {
        let resolved = resolver.resolve_profile(name);
        assert!(
            resolved.is_ok(),
            "Profile '{}' must resolve successfully: {:?}",
            name,
            resolved.err()
        );
        let val = resolved.unwrap();
        assert_eq!(val.get("name").unwrap().as_str().unwrap(), *name);
    }
}

#[test]
fn test_canonical_profiles_monotonicity() {
    let mut resolver = ProfileResolver::with_canonical_profiles();

    // 1. Attempting to relax default_action from 'ask' (in safe) to 'allow' in child profile -> Rejected!
    let illegal_downgrade = r#"
name = "illegal_child"
extends = "safe"

[policy]
default_action = "allow"
"#;
    resolver.register_profile("illegal_child", toml::from_str(illegal_downgrade).unwrap());
    let res = resolver.resolve_profile("illegal_child");
    assert!(
        matches!(res, Err(ConfigError::SecurityDowngradeDenied { .. })),
        "Must reject relaxing default_action"
    );

    // 2. Attempting to add unauthorized capabilities when extending safe -> Rejected!
    let illegal_caps = r#"
name = "unsafe_child"
extends = "safe"
capabilities = ["workspace_fs_read", "shell_exec"]
"#;
    resolver.register_profile("unsafe_child", toml::from_str(illegal_caps).unwrap());
    let res_caps = resolver.resolve_profile("unsafe_child");
    assert!(
        matches!(res_caps, Err(ConfigError::SecurityDowngradeDenied { .. })),
        "Must reject expanding capabilities beyond parent"
    );

    // 3. Attempting to lower verification tier in child -> Rejected!
    let illegal_tier = r#"
name = "low_tier_child"
extends = "ci"
verification_tier = 1
"#;
    resolver.register_profile("low_tier_child", toml::from_str(illegal_tier).unwrap());
    let res_tier = resolver.resolve_profile("low_tier_child");
    assert!(
        matches!(res_tier, Err(ConfigError::SecurityDowngradeDenied { .. })),
        "Must reject lowering verification tier"
    );
}
