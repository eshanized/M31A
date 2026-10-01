//! Integration and security test suite for PromptContracts, MiniJinja rendering, and PromptCatalog (Package 2).

use m31a::context::priority::ContextPriority;
use m31a::context::tokenizer::TokenizerAdapter;
use m31a::prompt::{
    InMemoryPromptCatalog, PromptCatalog, PromptComposition, PromptContract, PromptError,
    PromptParameter, PromptReference, RUNTIME_SAFETY_INVARIANTS, render_prompt,
};
use m31a::state_machine::agent::AgentRole;
use std::collections::BTreeMap;
use tempfile::tempdir;

#[test]
fn test_prompt_reference_parsing() {
    assert_eq!(
        PromptReference::parse("discovery.v1").unwrap(),
        PromptReference::new("discovery", 1)
    );
    assert_eq!(
        PromptReference::parse("genesis.discovery.v2").unwrap(),
        PromptReference::new("genesis.discovery", 2)
    );
    assert_eq!(
        PromptReference::parse("discovery:v3").unwrap(),
        PromptReference::new("discovery", 3)
    );
    assert_eq!(
        PromptReference::parse("discovery@4").unwrap(),
        PromptReference::new("discovery", 4)
    );

    // Missing version must fail
    assert!(PromptReference::parse("discovery").is_err());
    assert!(PromptReference::parse("   ").is_err());
}

#[test]
fn test_prompt_contract_deterministic_hashing() {
    let p1 = PromptParameter {
        name: "target".to_string(),
        description: "Target".to_string(),
        is_required: true,
        default_value: None,
    };
    let p2 = PromptParameter {
        name: "mode".to_string(),
        description: "Mode".to_string(),
        is_required: false,
        default_value: Some("fast".to_string()),
    };

    // Parameters order swapped in constructor: hash must be identical due to sorting
    let c1 = PromptContract::new(
        "test.prompt",
        1,
        AgentRole::researcher(),
        "Description",
        vec![p1.clone(), p2.clone()],
        "Hello {{ target }} (mode: {{ mode }})",
        Some("markdown".to_string()),
    )
    .unwrap();

    let c2 = PromptContract::new(
        "test.prompt",
        1,
        AgentRole::researcher(),
        "Description",
        vec![p2, p1],
        "Hello {{ target }} (mode: {{ mode }})",
        Some("markdown".to_string()),
    )
    .unwrap();

    assert_eq!(c1.content_hash, c2.content_hash);

    // Altered template body must alter hash
    let c3 = PromptContract::new(
        "test.prompt",
        1,
        AgentRole::researcher(),
        "Description",
        vec![],
        "Different body",
        Some("markdown".to_string()),
    )
    .unwrap();

    assert_ne!(c1.content_hash, c3.content_hash);
}

#[test]
fn test_catalog_immutability_and_duplicate_rejection() {
    let mut catalog = InMemoryPromptCatalog::new();
    let contract = PromptContract::new(
        "test.contract",
        1,
        AgentRole::planner(),
        "Original",
        vec![],
        "Body 1",
        None,
    )
    .unwrap();

    catalog.register(contract.clone()).unwrap();

    // Idempotent re-registration of exact contract succeeds
    catalog.register(contract).unwrap();

    // Conflicting contract with same id + version but different body must fail
    let conflicting = PromptContract::new(
        "test.contract",
        1,
        AgentRole::planner(),
        "Modified",
        vec![],
        "Body 2 modified",
        None,
    )
    .unwrap();

    let err = catalog.register(conflicting).unwrap_err();
    assert!(
        matches!(err, PromptError::PromptDuplicate { id, version, .. } if id == "test.contract" && version == 1)
    );
}

#[test]
fn test_built_in_catalog_coverage() {
    let catalog = InMemoryPromptCatalog::with_builtins();
    let list = catalog.list();
    assert!(list.len() >= 7, "must have all representative built-ins");

    // Exact version lookup
    let disc = catalog
        .get("genesis.discovery", 1)
        .expect("discovery v1 exists");
    assert_eq!(disc.role, AgentRole::researcher());
    assert_eq!(disc.version, 1);

    let arch = catalog
        .get("genesis.architecture", 1)
        .expect("architecture v1 exists");
    assert_eq!(arch.role, AgentRole::architect());

    // Missing version returns PromptNotFound
    let err = catalog.get("genesis.discovery", 99).unwrap_err();
    assert!(matches!(err, PromptError::PromptNotFound { version, .. } if version == 99));
}

#[test]
fn test_deterministic_minijinja_rendering() {
    let contract = PromptContract::new(
        "render.test",
        1,
        AgentRole::implementer(),
        "Test Render",
        vec![
            PromptParameter {
                name: "project".to_string(),
                description: "Name".to_string(),
                is_required: true,
                default_value: None,
            },
            PromptParameter {
                name: "edition".to_string(),
                description: "Edition".to_string(),
                is_required: false,
                default_value: Some("2024".to_string()),
            },
        ],
        "Building {{ project }} in Rust {{ edition }}.",
        None,
    )
    .unwrap();

    // 1. Successful render with default
    let mut params = BTreeMap::new();
    params.insert("project".to_string(), "M31A".to_string());
    let rendered = render_prompt(&contract, &params, true).unwrap();
    assert_eq!(rendered.rendered_text, "Building M31A in Rust 2024.");

    // 2. Override default
    params.insert("edition".to_string(), "2021".to_string());
    let rendered2 = render_prompt(&contract, &params, true).unwrap();
    assert_eq!(rendered2.rendered_text, "Building M31A in Rust 2021.");

    // 3. Missing required parameter fails
    let empty_params = BTreeMap::new();
    let err = render_prompt(&contract, &empty_params, true).unwrap_err();
    assert!(
        matches!(err, PromptError::PromptParameterMissing { parameter, .. } if parameter == "project")
    );

    // 4. Strict mode rejects unknown parameters
    params.insert("unrelated_hack".to_string(), "payload".to_string());
    let err2 = render_prompt(&contract, &params, true).unwrap_err();
    assert!(
        matches!(err2, PromptError::PromptParameterInvalid { parameter, .. } if parameter == "unrelated_hack")
    );
}

#[test]
fn test_rendering_memory_bounds_protection() {
    let contract = PromptContract::new(
        "render.bounds",
        1,
        AgentRole::implementer(),
        "Bounds",
        vec![PromptParameter {
            name: "payload".to_string(),
            description: "Payload".to_string(),
            is_required: true,
            default_value: None,
        }],
        "{{ payload }}",
        None,
    )
    .unwrap();

    let mut params = BTreeMap::new();
    // 2.5 MB payload exceeding MAX_PARAMETER_BYTES (2 MB)
    let giant_payload = "A".repeat(2_500_000);
    params.insert("payload".to_string(), giant_payload);

    let err = render_prompt(&contract, &params, false).unwrap_err();
    assert!(
        matches!(err, PromptError::PromptRenderFailure { reason, .. } if reason.contains("exceeds maximum permitted bound"))
    );
}

#[test]
fn test_seven_layer_prompt_composition() {
    let comp = PromptComposition::new(
        "You are an expert systems programmer.",
        "Implement SQLite transactional persistence layer.",
        vec![
            "All transactions must use BEGIN IMMEDIATE.".to_string(),
            "Foreign keys must be enabled.".to_string(),
        ],
    )
    .with_charter("Project M31A Autonomous Runtime")
    .with_upstream_artifact("discovery", "PROJECT.md", "Vision and invariants")
    .with_repo_context("Cargo workspace: 1 crate; Edition 2024");

    // Assert Layer 0 runtime safety is uncompromised
    assert!(comp.verify_safety_invariants());
    assert!(comp.layer_0_safety.contains(RUNTIME_SAFETY_INVARIANTS));

    // Full text rendering
    let full_text = comp.to_full_prompt();
    assert!(full_text.contains("## SYSTEM INVARIANTS (P0)"));
    assert!(full_text.contains("## AGENT ROLE & PROFILE (P1)"));
    assert!(full_text.contains("## WORKFLOW STEP OBJECTIVE & CONTRACT (P1)"));
    assert!(full_text.contains("## PROJECT CHARTER & BOUNDARIES (P2)"));
    assert!(full_text.contains("## UPSTREAM ARTIFACT EVIDENCE (P2/P3)"));
    assert!(full_text.contains("## REPOSITORY CONSTRAINTS & CONTEXT (P3/P4)"));
    assert!(full_text.contains("## QUALITY GATE VERIFICATION CHECKLIST (P1)"));

    // Context section priority mapping
    let tokenizer = TokenizerAdapter::conservative();
    let sections = comp.to_context_sections(&tokenizer);
    assert_eq!(sections[0].priority, ContextPriority::P0RuntimeSafety);
    assert_eq!(sections[1].priority, ContextPriority::P1TaskCompletion);
    assert_eq!(sections[2].priority, ContextPriority::P1TaskCompletion);
    assert_eq!(sections[3].priority, ContextPriority::P2RequiredEvidence);
    assert_eq!(sections[4].priority, ContextPriority::P2RequiredEvidence);
    assert_eq!(
        sections[5].priority,
        ContextPriority::P3ActiveWorkingContext
    );
    assert_eq!(sections[6].priority, ContextPriority::P1TaskCompletion);
}

#[test]
fn test_prompt_directory_loader_and_security() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Valid prompt file in root
    let prompt_file = root.join("valid.v1.toml");
    std::fs::write(
        &prompt_file,
        r#"
[prompt]
id = "custom.discovery"
version = 1
role = "researcher"
description = "Custom test prompt"

[[parameters]]
name = "idea"
description = "User idea"
is_required = true

[template]
body = "Custom discovery: {{ idea }}"
"#,
    )
    .unwrap();

    let mut catalog = InMemoryPromptCatalog::new();
    let loaded = catalog.load_from_dir(root, Some(root)).unwrap();
    assert_eq!(loaded, 1);
    assert!(catalog.contains("custom.discovery", 1));

    // Refuse path with ..
    let bad_path = root.join("../escape");
    let err = catalog.load_from_dir(&bad_path, Some(root)).unwrap_err();
    assert!(matches!(err, PromptError::PathViolation { .. }));

    // Refuse path in .git
    let git_dir = root.join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    let err2 = catalog.load_from_dir(&git_dir, Some(root)).unwrap_err();
    assert!(matches!(err2, PromptError::PathViolation { .. }));
}

#[test]
fn test_discovery_v1_toml_hash() {
    let toml = include_str!("../prompts/genesis/discovery.v1.toml");
    let contract = PromptContract::from_toml_str(toml).unwrap();
    assert_eq!(contract.id, "genesis.discovery");
    assert_eq!(contract.version, 1);
    assert_eq!(
        contract.content_hash,
        "c85ba2ad5e8010bcbba8c86db940bdbb871ba69dd81c4e84147ff7fc3aeb11d8"
    );

    let plan_toml = include_str!("../prompts/planning/decompose.v1.toml");
    let plan_contract = PromptContract::from_toml_str(plan_toml).unwrap();
    assert_eq!(plan_contract.id, "planning.decompose");
    assert_eq!(
        plan_contract.content_hash,
        "dbf36e52487567be4746d000af09840a42ff1ac1d2abf864b2094073580310b0"
    );

    let safety_toml = include_str!("../prompts/core/safety.v1.toml");
    let safety_contract = PromptContract::from_toml_str(safety_toml).unwrap();
    assert_eq!(safety_contract.id, "runtime.safety_invariants");
    assert_eq!(
        safety_contract.content_hash,
        "af1f9a98cea885e8577fbd6ee15e0720a8129ca224874bd7c8545057e4aa77b9"
    );
}

#[test]
fn test_all_25_builtins_hash_stability() {
    let expected_hashes: [(&str, u32, &str); 26] = [
        (
            "diagnose",
            1,
            "4554796b2c74569b29e950fcac4ba9c99c38068b2f08415434b7a7cd8e90d2f8",
        ),
        (
            "execution.diagnostician",
            1,
            "54b56d733b5534bcdc2ef6363b4ee210c0bb5604c5ce4e482d519cd505f7f250",
        ),
        (
            "execution.implementer",
            1,
            "8c70f2947d6773b1c9f1de03698041457e3f222c098a94be7b35c80225dd6e52",
        ),
        (
            "execution.reviewer",
            1,
            "dd79e65a115a1bc4a877bef626a434776bca795735973733611cb470c96a4330",
        ),
        (
            "execution.verifier",
            1,
            "acccde8a167bbdaf1f4982910c8ff0100c5caea56eb136e4a6a80e4390178ca2",
        ),
        (
            "genesis.adr",
            1,
            "09f654d84218ef77eef95f24822b44c70d0995fba02ac229a086640253c2b5cf",
        ),
        (
            "genesis.architecture",
            1,
            "197abafbc35b597ebd452137db5d8f119bba5dc2fbe8fb3e72ee65ee78c50d72",
        ),
        (
            "genesis.charter",
            1,
            "7ac13839718bff9cfa21cc3d841493535826a05bfb0d275def23faba63c37f32",
        ),
        (
            "genesis.discovery",
            1,
            "c85ba2ad5e8010bcbba8c86db940bdbb871ba69dd81c4e84147ff7fc3aeb11d8",
        ),
        (
            "genesis.requirements",
            1,
            "61f7ece0f00779b2fdbc7f107a8d7b1f871c4410bbffaa78cd1830f6a5943456",
        ),
        (
            "genesis.research_architecture",
            1,
            "4a7cc72d24bd9afbd69f8101fec33623db7c95b961aed1a8f9ccc325b6ecfd2e",
        ),
        (
            "genesis.research_deployment",
            1,
            "33bbe70e8297937c2f7fb715afc29de50adc331ad38531cce1f619a856f938fb",
        ),
        (
            "genesis.research_features",
            1,
            "53000eb0c041f2f340d3f312c83ea9035da7cd903fd9194a7cfccda5bda00d15",
        ),
        (
            "genesis.research_pitfalls",
            1,
            "1bd7be1efe10e105f42a69b4820113e3bb94c62c367dfa9fa4990eb62630b39c",
        ),
        (
            "genesis.research_security",
            1,
            "e2958ca25e6f858366c06d8e62cbfe2c19ffa234bbb1840b2fdfe9e6e2f903b8",
        ),
        (
            "genesis.research_stack",
            1,
            "cfba34e0113f769e1a4184d83cc07486e3e9be48cf2d1a6ba8c0231cec7a5959",
        ),
        (
            "genesis.research_synthesis",
            1,
            "6e5b47c4bd362c3fa55ea76278b52aa9e1fca1e100faf096b8d9d64e4cb39f2d",
        ),
        (
            "genesis.risks",
            1,
            "3e469a8aa7d14866682f201bf1cc164550aa7c9598ba7895da607ae6acf8e26e",
        ),
        (
            "genesis.roadmap",
            1,
            "75fdca742cf1e1d0928378c67b7ef3e44baa94612cbc0c402486d8d41deccfae",
        ),
        (
            "genesis.synthesis",
            1,
            "6f42a21591aae882b167de1c3d631e0ec32cf74a8b6ad2e737832cf946cb74e2",
        ),
        (
            "implement",
            1,
            "fc424e8908c73304be17061e54cfaf4c6d966fedc88037eb7874306420355a31",
        ),
        (
            "planning.decompose",
            1,
            "dbf36e52487567be4746d000af09840a42ff1ac1d2abf864b2094073580310b0",
        ),
        (
            "review",
            1,
            "6cde08d5ba2224ee2165d14731857d7f87f09061e797dafd54d53369afdee52c",
        ),
        (
            "runtime.safety_invariants",
            1,
            "af1f9a98cea885e8577fbd6ee15e0720a8129ca224874bd7c8545057e4aa77b9",
        ),
        (
            "skill.in_task_guidance",
            1,
            "57740fea36015f5a436e49640b097b23a0fe5dfcf79d86f3856bd1e6eaaed053",
        ),
        (
            "verify",
            1,
            "5cbe1f1a9c9f4c1f73fb1de89601da08f8db4153b8b253fb5269fc7aba73b1c2",
        ),
    ];

    let contracts = m31a::prompt::builtins::load_builtin_contracts().expect("all builtins parse");
    assert_eq!(contracts.len(), 46, "must have all 46 built-in contracts");

    for (expected_id, expected_version, expected_hash) in expected_hashes {
        let found = contracts
            .iter()
            .find(|c| c.id == expected_id && c.version == expected_version)
            .unwrap_or_else(|| panic!("missing builtin {} v{}", expected_id, expected_version));
        assert_eq!(
            found.content_hash, expected_hash,
            "hash mismatch for {} v{}: expected {}, found {}",
            expected_id, expected_version, expected_hash, found.content_hash
        );
    }
}
