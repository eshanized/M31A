//! Test suite for PromptOS Phase 2: Embedded TOML Builtin Catalog & Schema Validation.

use m31a::prompt::builtins::load_all_builtin_contracts;
use m31a::prompt::catalog::{InMemoryPromptCatalog, PromptCatalog};
use m31a::prompt::contract::PromptContract;
use m31a::prompt::error::PromptError;
use m31a::prompt::renderer::render_prompt;
use std::collections::{BTreeMap, BTreeSet};

const EXPECTED_BUILTIN_IDS: &[(&str, u32)] = &[
    ("agent.architect", 1),
    ("agent.architecture_researcher", 1),
    ("agent.auditor", 2),
    ("agent.deployment_researcher", 1),
    ("agent.diagnostician", 1),
    ("agent.diagnostician", 2),
    ("agent.discovery_analyst", 1),
    ("agent.features_researcher", 1),
    ("agent.implementer", 1),
    ("agent.implementer", 2),
    ("agent.integrator", 1),
    ("agent.pitfalls_researcher", 1),
    ("agent.planner", 1),
    ("agent.release_certifier", 2),
    ("agent.researcher", 1),
    ("agent.reviewer", 1),
    ("agent.reviewer", 2),
    ("agent.security_researcher", 1),
    ("agent.stack_researcher", 1),
    ("agent.synthesizer", 1),
    ("agent.verifier", 1),
    ("agent.verifier", 2),
    ("core.safety", 2),
    ("diagnose", 1),
    ("execution.authorization_explanation", 1),
    ("execution.diagnostician", 1),
    ("execution.implementer", 1),
    ("execution.implementer", 2),
    ("execution.reviewer", 1),
    ("execution.verifier", 1),
    ("genesis.adr", 1),
    ("genesis.architecture", 1),
    ("genesis.charter", 1),
    ("genesis.discovery", 1),
    ("genesis.dynamic_questions", 1),
    ("genesis.requirements", 1),
    ("genesis.research_architecture", 1),
    ("genesis.research_deployment", 1),
    ("genesis.research_features", 1),
    ("genesis.research_pitfalls", 1),
    ("genesis.research_security", 1),
    ("genesis.research_stack", 1),
    ("genesis.research_synthesis", 1),
    ("genesis.risks", 1),
    ("genesis.roadmap", 1),
    ("genesis.synthesis", 1),
    ("implement", 1),
    ("planning.decompose", 1),
    ("planning.decompose", 2),
    ("planning.revision", 1),
    ("planning.task_revision", 1),
    ("recovery.diagnostician", 2),
    ("review", 1),
    ("runtime.safety_invariants", 1),
    ("skill.in_task_guidance", 1),
    ("verification.reviewer", 2),
    ("verification.task", 2),
    ("verify", 1),
];

#[test]
fn test_catalog_completeness_and_exact_inventory() {
    let catalog = InMemoryPromptCatalog::with_builtins();
    let registered = catalog.list();
    assert_eq!(
        registered.len(),
        EXPECTED_BUILTIN_IDS.len(),
        "catalog must contain expected built-in contracts"
    );

    let registered_set: BTreeSet<(String, u32)> =
        registered.into_iter().map(|m| (m.id, m.version)).collect();

    for (expected_id, expected_version) in EXPECTED_BUILTIN_IDS {
        assert!(
            registered_set.contains(&(expected_id.to_string(), *expected_version)),
            "missing expected builtin: {} v{}",
            expected_id,
            expected_version
        );

        assert!(catalog.contains(expected_id, *expected_version));
        let contract = catalog
            .get(expected_id, *expected_version)
            .expect("contract exists");
        assert_eq!(contract.id, *expected_id);
        assert_eq!(contract.version, *expected_version);
        assert!(!contract.description.is_empty());
        assert!(!contract.template_body.is_empty());
        assert!(!contract.content_hash.is_empty());
    }
}

#[test]
fn test_golden_rendering_behavior() {
    let catalog = InMemoryPromptCatalog::with_builtins();

    // 1. genesis.discovery
    let disc = catalog.get("genesis.discovery", 1).unwrap();
    let mut disc_params = BTreeMap::new();
    disc_params.insert(
        "user_intent".to_string(),
        "Build a secure microkernel".to_string(),
    );
    disc_params.insert("turn_number".to_string(), "3".to_string());
    let rendered_disc = render_prompt(disc, &disc_params, true).unwrap();
    let expected_disc = "You are the Lead Discovery Analyst for M31A.\n\
                         Lead a structured Socratic interview for:\n\
                         Build a secure microkernel\n\
                         Turn: 3\n\
                         Clarify vision, boundaries, constraints, and non-goals.";
    assert_eq!(rendered_disc.rendered_text, expected_disc);

    // 2. execution.implementer
    let impl_contract = catalog.get("execution.implementer", 1).unwrap();
    let mut impl_params = BTreeMap::new();
    impl_params.insert(
        "task_spec".to_string(),
        "Implement AES-GCM encryption".to_string(),
    );
    let rendered_impl = render_prompt(impl_contract, &impl_params, true).unwrap();
    let expected_impl = "You are the M31A Implementer.\n\
                         Execute the following task specification:\n\
                         Implement AES-GCM encryption\n\
                         Modify files using tools and verify correctness with tests.";
    assert_eq!(rendered_impl.rendered_text, expected_impl);

    // 3. runtime.safety_invariants
    let safety = catalog.get("runtime.safety_invariants", 1).unwrap();
    let mut safety_params = BTreeMap::new();
    safety_params.insert("mission_id".to_string(), "MISSION-42".to_string());
    let rendered_safety = render_prompt(safety, &safety_params, true).unwrap();
    assert!(
        rendered_safety
            .rendered_text
            .contains("Mission ID: MISSION-42")
    );
    assert!(
        rendered_safety
            .rendered_text
            .contains("1. The model proposes. The runtime decides.")
    );
    assert!(
        rendered_safety
            .rendered_text
            .contains("CRITICAL MANDATE: You MUST make code modifications")
    );
}

#[test]
fn test_validation_fails_closed_on_invalid_toml() {
    // Malformed TOML syntax
    assert!(PromptContract::from_toml_str("this is not valid toml [[[}}").is_err());

    // Missing id
    let no_id = r#"
version = 1
role = "implementer"
description = "Test"
[body]
template = "Hello"
"#;
    assert!(matches!(
        PromptContract::from_toml_str(no_id),
        Err(PromptError::PromptInvalid { .. })
    ));

    // Version 0 rejected
    let v0 = r#"
id = "test.zero"
version = 0
role = "implementer"
description = "Test"
[body]
template = "Hello"
"#;
    assert!(matches!(
        PromptContract::from_toml_str(v0),
        Err(PromptError::PromptInvalid { .. })
    ));

    // Unknown-but-wellformed role: parsing succeeds (identity is open),
    // but existence is enforced explicitly at validation boundaries —
    // the role registry must not contain it.
    let bad_role = r#"
id = "test.badrole"
version = 1
role = "nonexistent_role_xyz"
description = "Test"
[body]
template = "Hello"
"#;
    let parsed = PromptContract::from_toml_str(bad_role).expect("well-formed role id must parse");
    assert_eq!(parsed.role.as_str(), "nonexistent_role_xyz");
    let guard = m31a::agent::registry::RoleRegistry::global()
        .read()
        .expect("registry readable");
    assert!(!guard.contains(&parsed.role));

    // Duplicate parameter names rejected
    let dup_params = r#"
id = "test.dup"
version = 1
role = "implementer"
description = "Test"
[inputs]
required = [
    { name = "param1", description = "first" },
    { name = "param1", description = "duplicate" },
]
[body]
template = "Hello"
"#;
    assert!(matches!(
        PromptContract::from_toml_str(dup_params),
        Err(PromptError::PromptInvalid { .. })
    ));

    // Parameter in both required and optional rejected
    let conflict_params = r#"
id = "test.conflict"
version = 1
role = "implementer"
description = "Test"
[inputs]
required = [
    { name = "param1", description = "required" },
]
optional = [
    { name = "param1", description = "optional", default = "" },
]
[body]
template = "Hello"
"#;
    assert!(matches!(
        PromptContract::from_toml_str(conflict_params),
        Err(PromptError::PromptInvalid { .. })
    ));

    // Invalid MiniJinja template syntax rejected
    let bad_jinja = r#"
id = "test.badjinja"
version = 1
role = "implementer"
description = "Test"
[body]
template = "Hello {{ unclosed"
"#;
    assert!(matches!(
        PromptContract::from_toml_str(bad_jinja),
        Err(PromptError::PromptInvalid { .. })
    ));

    // Template referencing undeclared variable rejected
    let undeclared_var = r#"
id = "test.undeclared"
version = 1
role = "implementer"
description = "Test"
[inputs]
required = []
[body]
template = "Hello {{ undeclared_parameter }}"
"#;
    assert!(matches!(
        PromptContract::from_toml_str(undeclared_var),
        Err(PromptError::PromptInvalid { .. })
    ));

    // False security.runtime_authority rejected
    let false_sec = r#"
id = "test.falsesec"
version = 1
role = "implementer"
description = "Test"
[security]
runtime_authority = false
[body]
template = "Hello"
"#;
    assert!(matches!(
        PromptContract::from_toml_str(false_sec),
        Err(PromptError::PromptInvalid { .. })
    ));
}

#[test]
fn test_embedded_builtins_load_and_deduplicate() {
    let contracts = load_all_builtin_contracts().expect("builtins parse cleanly");
    assert_eq!(contracts.len(), EXPECTED_BUILTIN_IDS.len());

    let mut catalog = InMemoryPromptCatalog::new();
    for contract in contracts {
        // First registration succeeds
        catalog.register(contract.clone()).unwrap();
        // Idempotent duplicate registration succeeds
        catalog.register(contract).unwrap();
    }

    assert_eq!(catalog.list().len(), EXPECTED_BUILTIN_IDS.len());
}
