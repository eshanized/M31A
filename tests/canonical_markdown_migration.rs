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
    let toml_val: toml::Value = cfg_toml.parse().unwrap();
    let toml_app: m31a::config::schema::AppConfig = toml_val.try_into().unwrap();
    let md_app = m31a::config::schema::parse_and_validate_config(&cfg_md).unwrap();
    assert_eq!(toml_app.provider.default, md_app.provider.default);
    assert_eq!(
        toml_app.runtime.concurrency_limit,
        md_app.runtime.concurrency_limit
    );

    // 3. Convert workflow fixture
    let wf_toml = fs::read_to_string(fixtures_dir.join("greenfield.toml")).unwrap();
    let legacy_value: toml::Value = wf_toml.parse().unwrap();
    let expected_id = legacy_value
        .get("workflow")
        .and_then(|workflow| workflow.get("id"))
        .and_then(toml::Value::as_str)
        .expect("legacy workflow fixture id");
    let expected_step_count = legacy_value
        .get("steps")
        .and_then(toml::Value::as_array)
        .map(Vec::len)
        .expect("legacy workflow steps");

    let wf_md = convert_workflow_toml_to_markdown(&wf_toml).unwrap();
    let md_wf = WorkflowManifest::from_markdown_str(&wf_md).unwrap();
    assert_eq!(expected_id, md_wf.workflow.id);
    assert_eq!(expected_step_count, md_wf.steps.len());

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

    let prompts_dir = tmp.path().join(".m31a/prompts");
    fs::create_dir_all(&prompts_dir).unwrap();
    let toml_copy = prompts_dir.join("discovery.v1.toml");
    fs::copy(fixtures_dir.join("discovery.v1.toml"), &toml_copy).unwrap();

    // Dry-run should succeed but not modify disk
    let result = migrate_path(&toml_copy, true, false);
    assert_eq!(result.migrated.len(), 1);
    assert!(!prompts_dir.join("discovery.v1.md").exists());
    assert!(toml_copy.exists());

    // Live migration (non-in-place)
    let result = migrate_path(&toml_copy, false, false);
    assert_eq!(result.migrated.len(), 1);
    assert!(prompts_dir.join("discovery.v1.md").exists());
    assert!(toml_copy.exists());

    // In-place migration
    let profiles_dir = tmp.path().join(".m31a/profiles");
    fs::create_dir_all(&profiles_dir).unwrap();
    let toml_copy2 = profiles_dir.join("profile.toml");
    fs::copy(fixtures_dir.join("profile.toml"), &toml_copy2).unwrap();
    let result = migrate_path(&toml_copy2, false, true);
    assert_eq!(result.migrated.len(), 1);
    assert!(profiles_dir.join("profile.md").exists());
    assert!(!toml_copy2.exists());
}

#[test]
fn test_document_roundtrip_fidelity_all_kinds() {
    let kinds = [
        (
            m31a::document::DocumentKind::Config,
            "runtime:\n  concurrency_limit: 4\n  timeout_secs: 60",
            "# Config\nBody notes.",
        ),
        (
            m31a::document::DocumentKind::Settings,
            "runtime:\n  concurrency_limit: 2",
            "# Settings\nBody notes.",
        ),
        (
            m31a::document::DocumentKind::Profile,
            "name: safe\nautonomy_mode: safe",
            "# Safe Profile\nBody notes.",
        ),
        (
            m31a::document::DocumentKind::Policy,
            "version: '1.0'\nname: test-pol\nrules: []",
            "# Policy\nBody notes.",
        ),
        (
            m31a::document::DocumentKind::Skill,
            "id: test-skill\nname: Test\nversion: '1.0'\nprocedure:\n  instructions: Run test",
            "# Skill\nBody notes.",
        ),
        (
            m31a::document::DocumentKind::Prompt,
            "id: test-prompt\nversion: 1\nkind: agent\nrole: reviewer\ndescription: Reviewer\ninputs:\n  required: []",
            "# Prompt\nBody notes.",
        ),
        (
            m31a::document::DocumentKind::Adapter,
            "adapter:\n  name: rust\n  language: rust\n  build_tool: cargo",
            "# Adapter\nBody notes.",
        ),
        (
            m31a::document::DocumentKind::Command,
            "name: test-cmd\ndescription: Test command\ntemplate_body: echo test",
            "# Command\nBody notes.",
        ),
        (
            m31a::document::DocumentKind::Workflow,
            "id: test-wf\nname: Test Workflow\nversion: 1\nphases: []",
            "# Workflow\nBody notes.",
        ),
    ];

    for (kind, yaml_str, body) in kinds {
        let data: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap();
        let original_doc =
            m31a::document::MarkdownDocument::new(kind, 1, data.clone(), body.to_string());
        let serialized =
            m31a::document::serialize_markdown_document(&original_doc).expect("serialize");
        let parsed_doc =
            m31a::document::parse_markdown_document::<serde_yaml::Value>(&serialized, kind)
                .expect("parse");

        assert_eq!(parsed_doc.kind, kind);
        assert_eq!(parsed_doc.schema_version, 1);
        assert_eq!(parsed_doc.data, data);
        assert_eq!(parsed_doc.body.trim(), body.trim());
    }
}

#[test]
fn test_convert_policy_toml_to_markdown_roundtrip() {
    let toml_str = r#"
version = "1.0"
name = "security-baseline"

[[rules]]
id = "deny-network-access"
description = "Block all outbound network calls"
decision = "deny"
tools = ["curl", "fetch_url"]
"#;
    let md = convert_policy_toml_to_markdown(toml_str).expect("convert policy toml to md");
    assert!(md.starts_with("---\nkind: policy\n"));
    assert!(md.contains("name: security-baseline"));
    assert!(md.contains("deny-network-access"));

    let doc =
        m31a::policy::rule::PolicyDocument::from_markdown_str(&md).expect("parse policy markdown");
    assert_eq!(doc.version, "1.0");
    assert_eq!(doc.name.as_deref(), Some("security-baseline"));
    assert_eq!(doc.rules.len(), 1);
    assert_eq!(doc.rules[0].id, "deny-network-access");
    assert_eq!(
        doc.rules[0].decision,
        m31a::kernel::seams::policy::PolicyDecision::Deny
    );
    assert_eq!(doc.rules[0].tools, vec!["curl", "fetch_url"]);
}

#[test]
fn test_cli_config_set_dotted_keys_and_body_preservation() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();

    let config_file = m31a_dir.join("config.md");
    let initial_content = r#"---
kind: config
schema_version: 1
data:
  runtime:
    concurrency_limit: 2
---

# Human-Readable Custom Comments
This documentation should NOT be lost when config is updated.
"#;
    fs::write(&config_file, initial_content).unwrap();

    // Verify initial load
    let resolved = m31a::config::ResolvedConfiguration::for_workspace(ws).unwrap();
    assert_eq!(resolved.app_config.runtime.concurrency_limit, 2);

    // Read and update dotted key using the canonical parser/serializer
    let raw_file_content = fs::read_to_string(&config_file).unwrap();
    let (_front_raw, body_raw) = m31a::document::split_frontmatter(&raw_file_content).unwrap();
    let doc: m31a::document::MarkdownDocument<serde_yaml::Value> =
        m31a::document::parse_markdown_value(
            &raw_file_content,
            m31a::document::DocumentKind::Config,
        )
        .unwrap();
    let mut data_map = match doc.data {
        serde_yaml::Value::Mapping(m) => m,
        _ => serde_yaml::Mapping::new(),
    };

    // Update runtime.concurrency_limit to 8
    let mut rt_map = match data_map.get("runtime") {
        Some(serde_yaml::Value::Mapping(m)) => m.clone(),
        _ => serde_yaml::Mapping::new(),
    };
    rt_map.insert(
        serde_yaml::Value::String("concurrency_limit".to_string()),
        serde_yaml::Value::Number(8.into()),
    );
    data_map.insert(
        serde_yaml::Value::String("runtime".to_string()),
        serde_yaml::Value::Mapping(rt_map),
    );

    let updated_doc = m31a::document::MarkdownDocument::new(
        m31a::document::DocumentKind::Config,
        1,
        serde_yaml::Value::Mapping(data_map),
        body_raw.to_string(),
    );
    let serialized = m31a::document::serialize_markdown_document(&updated_doc).unwrap();
    m31a::document::atomic_write_document(&config_file, &serialized).unwrap();

    // Reload and verify
    let updated_resolved = m31a::config::ResolvedConfiguration::for_workspace(ws).unwrap();
    assert_eq!(updated_resolved.app_config.runtime.concurrency_limit, 8);

    // Verify body preserved
    let file_on_disk = fs::read_to_string(&config_file).unwrap();
    assert!(file_on_disk.contains("Human-Readable Custom Comments"));
    assert!(file_on_disk.contains("This documentation should NOT be lost"));
}

#[test]
fn test_null_preservation_and_settings_compatibility() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();

    // Write settings.md with kind: settings and explicit null profile
    let settings_file = m31a_dir.join("settings.md");
    let content = r#"---
kind: settings
schema_version: 1
data:
  profile: null
  runtime:
    concurrency_limit: 4
---

# Workspace Settings
"#;
    fs::write(&settings_file, content).unwrap();

    // Verify it parses via parse_and_validate_config
    let parsed = m31a::config::schema::parse_and_validate_config(content).expect("parse settings");
    assert_eq!(parsed.profile, None);
    assert_eq!(parsed.runtime.concurrency_limit, 4);

    // Verify format-neutral JSON parser preserves null without crashing or pruning
    let json_val = m31a::config::resolved::parse_config_value_json(content).expect("parse json");
    assert_eq!(json_val.get("profile"), Some(&serde_json::Value::Null));

    // Verify workspace resolves using settings.md when config.md is absent
    let resolved =
        m31a::config::ResolvedConfiguration::for_workspace(ws).expect("resolve workspace");
    assert_eq!(resolved.app_config.runtime.concurrency_limit, 4);
    assert_eq!(resolved.active_profile, None);
}

#[test]
fn test_policy_markdown_precedence_and_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();

    let md_policy = m31a_dir.join("policy.md");
    let toml_policy = m31a_dir.join("policy.toml");

    let md_content = r#"---
kind: policy
schema_version: 1
data:
  version: "1.0"
  name: "markdown-workspace-policy"
  rules:
    - id: "deny-tool-from-md"
      description: "Deny tool from MD"
      decision: "deny"
      tools: ["tool_markdown"]
---

# Policy Notes
"#;

    let toml_content = r#"
version = "1.0"
name = "toml-workspace-policy"

[[rules]]
id = "deny-tool-from-toml"
description = "Deny tool from TOML"
decision = "deny"
tools = ["tool_toml"]
"#;

    fs::write(&md_policy, md_content).unwrap();
    fs::write(&toml_policy, toml_content).unwrap();

    // Load effective policy for workspace: Markdown must take precedence over legacy TOML
    let policy =
        m31a::policy::effective::EffectivePolicy::standard(ws).expect("load standard policy");
    let ctx_md =
        m31a::policy::matcher::PolicyEvaluationContext::new("tool_markdown", ws.to_path_buf());
    let (dec_md, rec_md) = policy.evaluate_request(&ctx_md);
    assert_eq!(dec_md, m31a::kernel::seams::policy::PolicyDecision::Deny);
    assert_eq!(rec_md.matched_rule_id.as_deref(), Some("deny-tool-from-md"));

    // Tool from TOML should NOT match because Markdown took precedence
    let ctx_toml =
        m31a::policy::matcher::PolicyEvaluationContext::new("tool_toml", ws.to_path_buf());
    let (dec_toml, _) = policy.evaluate_request(&ctx_toml);
    assert_ne!(dec_toml, m31a::kernel::seams::policy::PolicyDecision::Deny);
}

#[test]
fn test_malformed_canonical_policy_fails_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();

    let md_policy = m31a_dir.join("policy.md");
    let broken_md = r#"---
kind: policy
schema_version: 1
data: [broken yaml :::
---
"#;
    fs::write(&md_policy, broken_md).unwrap();

    let res = m31a::policy::effective::EffectivePolicy::standard(ws);
    assert!(
        res.is_err(),
        "Malformed canonical markdown policy must fail closed"
    );
}

#[test]
fn test_directory_migration_preserves_repo_legacy_fixtures() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = repo_root.join("tests/fixtures/legacy_toml");

    let expected_fixtures = [
        "adapter.toml",
        "command.toml",
        "config.example.toml",
        "decompose.v1.toml",
        "discovery.v1.toml",
        "greenfield.toml",
        "organization.toml",
        "profile.toml",
        "safety.v1.toml",
        "skill.toml",
    ];

    // Dry-run migration targeting repository root
    let _dry_report = m31a::config::migrate::migrate_path(&repo_root, true, false);

    // Verify all fixtures still exist as TOML and NO sibling .md files exist
    for fixture in &expected_fixtures {
        let p = fixtures_dir.join(fixture);
        if p.exists() {
            let md_sibling = fixtures_dir.join(format!("{}.md", fixture.trim_end_matches(".toml")));
            assert!(
                !md_sibling.exists(),
                "sibling md {} must not exist",
                md_sibling.display()
            );
        }
    }

    // Now test recursive in-place in a temp directory copy containing a tests/fixtures directory
    let tmp = tempfile::tempdir().unwrap();
    let nested_fixtures = tmp.path().join("tests/fixtures/legacy_toml");
    fs::create_dir_all(&nested_fixtures).unwrap();
    for fixture in &expected_fixtures {
        let src = fixtures_dir.join(fixture);
        if src.exists() {
            fs::copy(&src, nested_fixtures.join(fixture)).unwrap();
        }
    }

    let _run_report = m31a::config::migrate::migrate_path(tmp.path(), false, true);

    // Fixtures in nested tests/fixtures must remain completely untouched even with in_place = true
    for fixture in &expected_fixtures {
        let p = nested_fixtures.join(fixture);
        if fixtures_dir.join(fixture).exists() {
            assert!(
                p.exists(),
                "fixture {} in copy must remain untouched",
                fixture
            );
            let md_sibling =
                nested_fixtures.join(format!("{}.md", fixture.trim_end_matches(".toml")));
            assert!(!md_sibling.exists(), "sibling md must not be created");
        }
    }
}

#[test]
fn test_explicit_targeting_of_ineligible_paths_is_rejected_or_skipped() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = repo_root.join("tests/fixtures/legacy_toml/discovery.v1.toml");
    let cargo_toml = repo_root.join("Cargo.toml");

    let mut report = m31a::config::migrate::MigrationReport::default();
    m31a::config::migrate::migrate_single_file(&fixture_path, false, false, &mut report);
    assert_eq!(report.migrated.len(), 0);
    assert!(report.skipped.iter().any(|s| s.contains("ineligible path")));

    let mut report2 = m31a::config::migrate::MigrationReport::default();
    m31a::config::migrate::migrate_single_file(&cargo_toml, false, false, &mut report2);
    assert_eq!(report2.migrated.len(), 0);
    assert!(
        report2
            .skipped
            .iter()
            .any(|s| s.contains("toolchain manifest") || s.contains("ineligible"))
    );
}

#[test]
fn test_symlink_escaping_migration_root_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("migration_root");
    let outside = tmp.path().join("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();

    let outside_target = outside.join("target_dir");
    fs::create_dir_all(&outside_target).unwrap();
    let outside_toml = outside_target.join("config.toml");
    fs::write(&outside_toml, "profile = \"safe\"\n").unwrap();

    // Create symlink inside root pointing to outside_target
    let symlink_path = root.join("symlinked_dir");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_target, &symlink_path).unwrap();
    let explicit_file_link = root.join("explicit-config.toml");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_toml, &explicit_file_link).unwrap();

    #[cfg(unix)]
    {
        let report = m31a::config::migrate::migrate_path(&root, false, true);
        assert_eq!(report.migrated.len(), 0);
        assert!(
            report
                .skipped
                .iter()
                .any(|s| s.contains("outside migration root") || s.contains("symlink"))
        );
        assert!(
            outside_toml.exists(),
            "outside toml must not be deleted or modified"
        );
        assert!(!outside_target.join("config.md").exists());

        // An explicitly supplied symlink is rejected too; explicit paths do
        // not bypass the recursive scanner's symlink protections.
        let explicit_report =
            m31a::config::migrate::migrate_path(&explicit_file_link, false, true);
        assert!(explicit_report.migrated.is_empty());
        assert!(explicit_report.skipped.iter().any(|s| s.contains("symlink targets")));
        assert!(outside_toml.exists());
        assert!(!outside_target.join("config.md").exists());
    }
}

#[test]
fn test_dry_run_validation_failure_matches_real_run() {
    let tmp = tempfile::tempdir().unwrap();
    let m31a_dir = tmp.path().join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();
    let invalid_toml = m31a_dir.join("config.toml");
    // Invalid config (concurrency_limit = 0 violates validation)
    fs::write(
        &invalid_toml,
        "profile = \"safe\"\n[runtime]\nconcurrency_limit = 0\n",
    )
    .unwrap();

    let dry_report = m31a::config::migrate::migrate_path(&invalid_toml, true, false);
    // When semantic verification fails, dry-run must report it as an error, NOT planned
    assert_eq!(dry_report.migrated.len(), 0);
    assert_eq!(dry_report.errors.len(), 1);
    assert!(
        dry_report.errors[0]
            .reason
            .as_ref()
            .unwrap()
            .contains("Semantic verification failed")
            || dry_report.errors[0]
                .reason
                .as_ref()
                .unwrap()
                .contains("verification")
    );
    assert!(!m31a_dir.join("config.md").exists());

    let live_report = m31a::config::migrate::migrate_path(&invalid_toml, false, false);
    assert_eq!(live_report.migrated.len(), 0);
    assert_eq!(live_report.errors.len(), 1);
    assert!(!m31a_dir.join("config.md").exists());
}

#[test]
fn test_config_set_preserves_settings_md() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();

    // Create a populated settings.md with existing values and custom Markdown comments
    let settings_md = m31a_dir.join("settings.md");
    let settings_content = r#"---
kind: settings
schema_version: 1
data:
  profile: safe
  runtime:
    concurrency_limit: 8
    timeout_secs: 450
---
# Custom Organization Settings
# Note: do not reduce timeout_secs below 400!
Authoritative organizational guidelines for this repository.
"#;
    fs::write(&settings_md, settings_content).unwrap();

    // Mutate a key (e.g. runtime.timeout_secs = 600)
    let res = m31a::config::schema::mutate_and_persist_workspace_config(
        ws,
        "runtime.timeout_secs",
        "600",
    )
    .expect("mutate_and_persist_workspace_config should succeed");

    assert!(res.canonical_path.exists());
    assert_eq!(res.key, "runtime.timeout_secs");
    assert_eq!(res.value, "600");

    // Read the newly created canonical config.md
    let canonical_content = fs::read_to_string(&res.canonical_path).unwrap();
    assert!(canonical_content.contains("timeout_secs: 600"));
    // Pre-existing settings from settings.md must be preserved!
    assert!(canonical_content.contains("concurrency_limit: 8"));
    assert!(canonical_content.contains("profile: safe"));
    // Markdown comments from settings.md must be preserved!
    assert!(canonical_content.contains("Custom Organization Settings"));
    assert!(canonical_content.contains("do not reduce timeout_secs below 400!"));

    // Original settings.md must remain intact
    assert!(settings_md.exists());
}

#[test]
fn test_config_set_rejects_non_mapping_document_without_modifying_it() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();

    let config_file = m31a_dir.join("config.md");
    let original = r#"---
kind: config
schema_version: 1
data:
  - this-is-not-a-mapping
---
# Preserve this document
Do not replace invalid data with empty defaults.
"#;
    fs::write(&config_file, original).unwrap();

    let result = m31a::config::schema::mutate_and_persist_workspace_config(
        ws,
        "runtime.concurrency_limit",
        "4",
    );
    assert!(result.is_err(), "non-mapping data must reject mutation");
    assert!(result.unwrap_err().contains("non-mapping front matter data"));
    assert_eq!(
        fs::read_to_string(&config_file).unwrap(),
        original,
        "rejected mutations must leave the original document byte-for-byte unchanged"
    );
}

#[test]
fn test_config_set_rejects_settings_and_legacy_toml_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();

    let settings_file = m31a_dir.join("settings.md");
    fs::write(
        &settings_file,
        r#"---
kind: settings
schema_version: 1
data:
  runtime:
    concurrency_limit: 7
---
# Existing settings
"#,
    )
    .unwrap();

    let legacy_file = m31a_dir.join("config.toml");
    fs::write(&legacy_file, "profile = \"safe\"\n").unwrap();

    let result = m31a::config::schema::mutate_and_persist_workspace_config(
        ws,
        "runtime.concurrency_limit",
        "9",
    );
    assert!(result.is_err(), "conflicting sources must not be silently ignored");
    assert!(result.unwrap_err().contains("Conflicting workspace configuration sources"));
    assert!(settings_file.exists());
    assert!(legacy_file.exists());
    assert!(!m31a_dir.join("config.md").exists());
}

#[test]
fn test_config_set_imports_retired_dot_m31_path_into_canonical_dot_m31a_config() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let retired_dir = ws.join(".m31");
    fs::create_dir_all(&retired_dir).unwrap();
    let legacy_file = retired_dir.join("config.toml");
    fs::write(
        &legacy_file,
        "profile = \"safe\"\n[runtime]\nconcurrency_limit = 3\ntimeout_secs = 120\n",
    )
    .unwrap();

    let result = m31a::config::schema::mutate_and_persist_workspace_config(
        ws,
        "runtime.concurrency_limit",
        "5",
    )
    .expect("legacy .m31 config should import before mutation");

    let canonical = ws.join(".m31a/config.md");
    assert_eq!(result.canonical_path, canonical);
    let resolved = m31a::config::ResolvedConfiguration::for_workspace(ws).unwrap();
    assert_eq!(resolved.app_config.runtime.concurrency_limit, 5);
    assert_eq!(resolved.app_config.runtime.timeout_secs, 120);
    assert_eq!(resolved.app_config.profile.as_deref(), Some("safe"));
    assert!(legacy_file.exists(), "non-in-place import must preserve legacy source");
}

#[test]
fn test_legacy_planning_workflow_migrates_to_runtime_canonical_path() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let legacy_dir = workspace.join(".planning/workflows");
    fs::create_dir_all(&legacy_dir).unwrap();

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/legacy_toml/greenfield.toml");
    let legacy_file = legacy_dir.join("greenfield.toml");
    fs::copy(&fixture, &legacy_file).unwrap();

    // Scan the workspace root to prove the hidden legacy directory is discovered.
    let report = m31a::config::migrate::migrate_path(workspace, false, false);
    assert_eq!(report.migrated.len(), 1, "{report:?}");
    assert!(legacy_file.exists(), "non-in-place import preserves the legacy source");

    let canonical = workspace.join(".m31a/project/workflows/greenfield.md");
    assert!(canonical.exists(), "migrated workflow must be available to resume/approval");
    assert!(
        !legacy_dir.join("greenfield.md").exists(),
        "do not create a second manifest beside the retired workflow"
    );

    let manifest = m31a::workflow::manifest::WorkflowManifest::from_file(
        &canonical,
        Some(workspace),
    )
    .expect("canonical migrated workflow must be loadable by runtime");
    assert_eq!(manifest.workflow.id, "greenfield");
}

#[test]
fn test_workflow_migration_rejects_semantically_invalid_candidate() {
    let tmp = tempfile::tempdir().unwrap();
    let legacy_dir = tmp.path().join(".planning/workflows");
    fs::create_dir_all(&legacy_dir).unwrap();
    let legacy_file = legacy_dir.join("greenfield.toml");
    let original = r#"
manifest_version = 1

[workflow]
id = ""
name = "Invalid Workflow"
version = 1
default_recovery_strategy = { type = "fail" }

[[steps]]
key = "step"
name = "Step"
role = "researcher"
prompt = "test.v1"
timeout_secs = 60
"#;
    fs::write(&legacy_file, original).unwrap();

    let report = m31a::config::migrate::migrate_path(&legacy_file, false, true);
    assert!(report.migrated.is_empty());
    assert_eq!(report.errors.len(), 1);
    assert!(
        report.errors[0]
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("workflow semantic validation error")
    );
    assert_eq!(fs::read_to_string(&legacy_file).unwrap(), original);
    assert!(!tmp.path().join(".m31a/project/workflows/greenfield.md").exists());
}

#[test]
fn test_policy_load_fails_closed_when_only_legacy_toml_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();

    let legacy_policy = m31a_dir.join("policy.toml");
    fs::write(&legacy_policy, "version = 1\nname = \"legacy\"\n").unwrap();

    let res = m31a::policy::effective::EffectivePolicy::standard(ws);
    assert!(
        res.is_err(),
        "Loading policy when only legacy TOML exists must fail closed"
    );
    let err = res.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("migration to canonical Markdown is required")
            || err_str.contains("LegacyMigrationRequired"),
        "Error message was: {err_str}"
    );
}
