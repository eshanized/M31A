//! Comprehensive tests for canonical Markdown architecture, asset validation,
//! legacy TOML migration fidelity, and precedence invariants.

use m31a::config::migrate::*;
use m31a::prompt::contract::PromptContract;
use m31a::skill::manifest::SkillManifest;
use m31a::workflow::manifest::WorkflowManifest;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_all_canonical_markdown_assets_exist_and_validate() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // 1. Verify Profiles (10 files)
    let profiles_dir = repo_root.join("assets/profiles");
    let profile_names = [
        "autonomous",
        "balanced",
        "ci",
        "code_reviewer",
        "coding",
        "conservative",
        "release",
        "research",
        "safe",
        "security_review",
    ];
    for name in &profile_names {
        let md_path = profiles_dir.join(format!("{name}.md"));
        assert!(md_path.exists(), "Profile {} must exist", md_path.display());
        let md_content = fs::read_to_string(&md_path).expect("read profile md");
        let doc = m31a::document::parse_markdown_document::<serde_yaml::Value>(
            &md_content,
            m31a::document::DocumentKind::Profile,
        )
        .expect("profile must parse as Markdown document");
        assert_eq!(doc.kind, m31a::document::DocumentKind::Profile);
        assert_eq!(doc.schema_version, 1);
    }

    // 2. Verify Skills (3 files)
    let skills_dir = repo_root.join("assets/skills");
    let skill_ids = ["fix-test-failure", "prepare-release", "refactor-module"];
    for skill_id in &skill_ids {
        let md_path = skills_dir.join(skill_id).join("skill.md");
        assert!(md_path.exists(), "Skill {} must exist", md_path.display());
        let md_content = fs::read_to_string(&md_path).expect("read skill md");
        let manifest = SkillManifest::parse_markdown(&md_content)
            .expect("skill must parse as Markdown document");
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.id, *skill_id);
    }

    // 3. Verify Verification Adapters (5 files)
    let adapters_dir = repo_root.join("assets/verification_adapters");
    let adapter_names = ["custom", "go", "node", "python", "rust"];
    for name in &adapter_names {
        let md_path = adapters_dir.join(format!("{name}.md"));
        assert!(md_path.exists(), "Adapter {} must exist", md_path.display());
        let md_content = fs::read_to_string(&md_path).expect("read adapter md");
        let doc = m31a::document::parse_markdown_document::<serde_yaml::Value>(
            &md_content,
            m31a::document::DocumentKind::Adapter,
        )
        .expect("adapter must parse as Markdown document");
        assert_eq!(doc.kind, m31a::document::DocumentKind::Adapter);
        assert_eq!(doc.schema_version, 1);
    }

    // 4. Verify Examples (5 files)
    let examples_dir = repo_root.join("examples");
    let cfg_path = examples_dir.join("config.example.md");
    assert!(cfg_path.exists());
    let cfg_content = fs::read_to_string(&cfg_path).unwrap();
    let _ = m31a::config::schema::parse_and_validate_config(&cfg_content).unwrap();

    let disc_path = examples_dir.join("prompts/discovery.v1.md");
    assert!(disc_path.exists());
    let disc_content = fs::read_to_string(&disc_path).unwrap();
    let _ = PromptContract::from_markdown_str(&disc_content).unwrap();

    let res_path = examples_dir.join("prompts/research.v1.md");
    assert!(res_path.exists());
    let res_content = fs::read_to_string(&res_path).unwrap();
    let _ = PromptContract::from_markdown_str(&res_content).unwrap();

    let cmd_path = examples_dir.join("user_commands/atomic-commit.md");
    assert!(cmd_path.exists());
    let cmd_content = fs::read_to_string(&cmd_path).unwrap();
    let _ =
        m31a::interaction::user_commands::parse_user_command_markdown(&cmd_content, None).unwrap();

    let wf_path = examples_dir.join("workflows/greenfield.md");
    assert!(wf_path.exists());
    let wf_content = fs::read_to_string(&wf_path).unwrap();
    let _ = WorkflowManifest::from_markdown_str(&wf_content).unwrap();

    // 5. Verify Prompts (58 files)
    let prompt_paths = [
        // Active Canonical Agent Prompts (18)
        "prompts/agents/architect.md",
        "prompts/agents/architecture_researcher.md",
        "prompts/agents/auditor.md",
        "prompts/agents/deployment_researcher.md",
        "prompts/agents/diagnostician.md",
        "prompts/agents/discovery_analyst.md",
        "prompts/agents/features_researcher.md",
        "prompts/agents/implementer.md",
        "prompts/agents/integrator.md",
        "prompts/agents/pitfalls_researcher.md",
        "prompts/agents/planner.md",
        "prompts/agents/release_certifier.md",
        "prompts/agents/researcher.md",
        "prompts/agents/reviewer.md",
        "prompts/agents/security_researcher.md",
        "prompts/agents/stack_researcher.md",
        "prompts/agents/synthesizer.md",
        "prompts/agents/verifier.md",
        // Agent Compatibility Prompts (4)
        "prompts/agents/compat/diagnostician.v1.md",
        "prompts/agents/compat/implementer.v1.md",
        "prompts/agents/compat/reviewer.v1.md",
        "prompts/agents/compat/verifier.v1.md",
        // Core Prompts (2)
        "prompts/core/safety.md",
        "prompts/core/compat/safety.v1.md",
        // Execution Prompts (8)
        "prompts/execution/authorization_explanation.md",
        "prompts/execution/diagnose.md",
        "prompts/execution/implement.md",
        "prompts/execution/implementer.md",
        "prompts/execution/compat/implementer.v1.md",
        "prompts/execution/review.md",
        "prompts/execution/verifier.md",
        "prompts/execution/verify.md",
        // Genesis Prompts (16)
        "prompts/genesis/adr.md",
        "prompts/genesis/architecture.md",
        "prompts/genesis/charter.md",
        "prompts/genesis/discovery.md",
        "prompts/genesis/dynamic_questions.md",
        "prompts/genesis/requirements.md",
        "prompts/genesis/research_architecture.md",
        "prompts/genesis/research_deployment.md",
        "prompts/genesis/research_features.md",
        "prompts/genesis/research_pitfalls.md",
        "prompts/genesis/research_security.md",
        "prompts/genesis/research_stack.md",
        "prompts/genesis/research_synthesis.md",
        "prompts/genesis/risks.md",
        "prompts/genesis/roadmap.md",
        "prompts/genesis/synthesis.md",
        // Planning Prompts (4)
        "prompts/planning/decompose.md",
        "prompts/planning/compat/decompose.v1.md",
        "prompts/planning/plan_revision.md",
        "prompts/planning/task_revision.md",
        // Recovery Prompts (2)
        "prompts/recovery/diagnostician.md",
        "prompts/recovery/compat/diagnostician.v1.md",
        // Skill Guidance Prompts (1)
        "prompts/skills/in_task_guidance.md",
        // Verification Prompts (3)
        "prompts/verification/reviewer.md",
        "prompts/verification/compat/reviewer.v1.md",
        "prompts/verification/task.md",
    ];

    assert_eq!(
        prompt_paths.len(),
        58,
        "must cover exactly 58 canonical prompt files"
    );

    for md_rel in &prompt_paths {
        let md_path = repo_root.join(md_rel);
        assert!(md_path.exists(), "Prompt {} must exist", md_path.display());

        let md_str = fs::read_to_string(&md_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", md_path.display()));
        let md_contract = PromptContract::from_markdown_str(&md_str)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", md_path.display()));

        assert!(!md_contract.id.is_empty());
        assert!(md_contract.version >= 1);
        assert_eq!(md_contract.content_hash.len(), 64);
    }
}

#[test]
fn test_legacy_toml_migration_fidelity_via_fixtures() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = repo_root.join("tests/fixtures/legacy_toml");

    // 1. Convert prompt fixture
    let prompt_toml = fs::read_to_string(fixtures_dir.join("discovery.v1.toml")).unwrap();
    let prompt_md = convert_prompt_toml_to_markdown(&prompt_toml).unwrap();
    let toml_c = PromptContract::from_toml_str(&prompt_toml).unwrap();
    let md_c = PromptContract::from_markdown_str(&prompt_md).unwrap();
    assert_eq!(toml_c.id, md_c.id);
    assert_eq!(toml_c.version, md_c.version);
    assert_eq!(toml_c.content_hash, md_c.content_hash);

    // 2. Convert config fixture
    let cfg_toml = fs::read_to_string(fixtures_dir.join("config.example.toml")).unwrap();
    let cfg_md = convert_config_toml_to_markdown(&cfg_toml).unwrap();
    let toml_app = m31a::config::schema::parse_and_validate_config(&cfg_toml).unwrap();
    let md_app = m31a::config::schema::parse_and_validate_config(&cfg_md).unwrap();
    assert_eq!(toml_app.provider.default, md_app.provider.default);
    assert_eq!(
        toml_app.runtime.concurrency_limit,
        md_app.runtime.concurrency_limit
    );

    // 3. Convert workflow fixture
    let wf_toml = fs::read_to_string(fixtures_dir.join("greenfield.toml")).unwrap();
    let wf_md = convert_workflow_toml_to_markdown(&wf_toml).unwrap();
    let toml_wf = WorkflowManifest::from_toml_str(&wf_toml).unwrap();
    let md_wf = WorkflowManifest::from_markdown_str(&wf_md).unwrap();
    assert_eq!(toml_wf.workflow.id, md_wf.workflow.id);
    assert_eq!(toml_wf.steps.len(), md_wf.steps.len());

    // 4. Convert profile fixture
    let prof_toml = fs::read_to_string(fixtures_dir.join("profile.toml")).unwrap();
    let prof_md = convert_profile_toml_to_markdown(&prof_toml, "safe").unwrap();
    let doc = m31a::document::parse_markdown_document::<serde_yaml::Value>(
        &prof_md,
        m31a::document::DocumentKind::Profile,
    )
    .unwrap();
    assert_eq!(doc.kind, m31a::document::DocumentKind::Profile);

    // 5. Convert skill fixture
    let skill_toml = fs::read_to_string(fixtures_dir.join("skill.toml")).unwrap();
    let skill_md = convert_skill_toml_to_markdown(&skill_toml).unwrap();
    let toml_sk = SkillManifest::parse_toml(&skill_toml).unwrap();
    let md_sk = SkillManifest::parse_markdown(&skill_md).unwrap();
    assert_eq!(toml_sk.id, md_sk.id);

    // 6. Convert adapter fixture
    let ad_toml = fs::read_to_string(fixtures_dir.join("adapter.toml")).unwrap();
    let ad_md = convert_adapter_toml_to_markdown(&ad_toml, "rust").unwrap();
    let doc = m31a::document::parse_markdown_document::<serde_yaml::Value>(
        &ad_md,
        m31a::document::DocumentKind::Adapter,
    )
    .unwrap();
    assert_eq!(doc.kind, m31a::document::DocumentKind::Adapter);

    // 7. Convert command fixture
    let cmd_toml = fs::read_to_string(fixtures_dir.join("command.toml")).unwrap();
    let cmd_md = convert_command_toml_to_markdown(&cmd_toml).unwrap();
    let toml_cmd =
        m31a::interaction::user_commands::parse_user_command_toml(&cmd_toml, None).unwrap();
    let md_cmd =
        m31a::interaction::user_commands::parse_user_command_markdown(&cmd_md, None).unwrap();
    assert_eq!(toml_cmd.name, md_cmd.name);
    assert_eq!(toml_cmd.template_body, md_cmd.template_body);
}

#[test]
fn test_cli_migrate_dry_run_and_in_place() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = repo_root.join("tests/fixtures/legacy_toml");

    let toml_copy = tmp.path().join("discovery.v1.toml");
    fs::copy(fixtures_dir.join("discovery.v1.toml"), &toml_copy).unwrap();

    // Dry-run should succeed but not modify disk
    let result = migrate_path(&toml_copy, true, false);
    assert_eq!(result.migrated.len(), 1);
    assert!(!tmp.path().join("discovery.v1.md").exists());
    assert!(toml_copy.exists());

    // Live migration (non-in-place)
    let result = migrate_path(&toml_copy, false, false);
    assert_eq!(result.migrated.len(), 1);
    assert!(tmp.path().join("discovery.v1.md").exists());
    assert!(toml_copy.exists());

    // In-place migration
    let toml_copy2 = tmp.path().join("profile.toml");
    fs::copy(fixtures_dir.join("profile.toml"), &toml_copy2).unwrap();
    let result = migrate_path(&toml_copy2, false, true);
    assert_eq!(result.migrated.len(), 1);
    assert!(tmp.path().join("profile.md").exists());
    assert!(!toml_copy2.exists());
}
