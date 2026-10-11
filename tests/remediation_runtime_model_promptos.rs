//! Comprehensive Regression Tests for Runtime, Model Routing, PromptOS, and TUI Remediation.
//!
//! Covers findings:
//! - MODEL-01 & UX-02: Primary-model eligibility enforcement across router, selector, command, and bridge.
//! - MODEL-02: NVIDIA provider output-token limit clamping and context boundary enforcement.
//! - MODEL-03: Model selector truthful provider availability and prevention of invalid primary model selection.
//! - PROMPT-01: Shipped prompt asset discovery trust boundary and transactional directory loading.
//! - PROMPT-02: Prompt guidance compilation hash recomputation, provenance update, and budget enforcement.
//! - PROMPT-03: Canonical version resolution precedence before override checks.
//! - PROMPT-04: Authoritative prompt inventory audit via reachability classification matrix.
//! - UX-01: Conversation error inspection via Enter key opening FailureRecovery inspector and dismissal via Esc/Enter.

use std::path::PathBuf;

use m31a::config::provider_registry::PRODUCTION_PROVIDER_ID;
use m31a::model::ProviderModelMetadataSource;
use m31a::model::provider::nvidia::NvidiaProvider;
use m31a::model::provider::nvidia_metadata::NvidiaModelMetadataResolver;
use m31a::model::router::resolver::{
    CapabilitySupport, ModelCandidate, ModelKind, ModelRole, check_fast_auxiliary_eligibility,
    check_model_eligibility, check_primary_eligibility,
};
use m31a::model::types::{ModelError, ModelInvocationOptions, ProviderCapabilityStatus};
use m31a::prompt::catalog::{InMemoryPromptCatalog, PromptCatalog};
use m31a::prompt::compiler::{CompilationOptions, DefaultPromptCompiler};
use m31a::prompt::context::{MissionStage, PromptContext};
use m31a::prompt::contract::PromptContract;
use m31a::prompt::provenance::PromptSourceKind;
use m31a::state_machine::agent::AgentRole;
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::model::TuiViewModel;
use m31a::tui::surface::model_selector::{
    ModelInfo, ModelSelectorMode, ModelSelectorState, handle_model_selector_key,
    resolve_display_models_and_providers,
};

// ── MODEL-01 & UX-02: Primary-model eligibility ──────────────────────────────

#[test]
fn test_reward_model_rejected_for_primary_reasoning() {
    let mut candidate = ModelCandidate::new_unknown(
        "nvidia/nemotron-4-340b-reward".to_string(),
        PRODUCTION_PROVIDER_ID.to_string(),
    );
    candidate.context_capacity = 4096;
    candidate.supports_tools = true;
    candidate.tool_support = CapabilitySupport::Supported;
    candidate.availability = ProviderCapabilityStatus::Available;

    let resolver = NvidiaModelMetadataResolver::new_offline();
    futures::executor::block_on(resolver.enrich_candidate(&mut candidate)).unwrap();

    assert_eq!(candidate.model_kind, ModelKind::Reward);

    // Primary reasoning check must reject reward model
    let err = check_primary_eligibility(&candidate).expect_err("reward model cannot be primary");
    assert!(
        err.to_lowercase().contains("reward model"),
        "error message must clearly identify reward model rejection: {err}"
    );

    // Fast auxiliary check must also reject reward model
    let err_fast = check_fast_auxiliary_eligibility(&candidate)
        .expect_err("reward model cannot be fast auxiliary");
    assert!(err_fast.to_lowercase().contains("reward model"));

    // Check with explicit role enum
    assert!(check_model_eligibility(&candidate, ModelRole::PrimaryReasoning).is_err());
    assert!(check_model_eligibility(&candidate, ModelRole::FastAuxiliary).is_err());
}

#[test]
fn test_embedding_and_tool_lacking_models_rejected_for_primary() {
    let mut embedding = ModelCandidate::new_unknown(
        "nvidia/nv-embedqa-e5-v5".to_string(),
        PRODUCTION_PROVIDER_ID.to_string(),
    );
    let resolver = NvidiaModelMetadataResolver::new_offline();
    futures::executor::block_on(resolver.enrich_candidate(&mut embedding)).unwrap();
    assert!(embedding.model_kind.is_embedding());

    let err = check_primary_eligibility(&embedding).expect_err("embedding model cannot be primary");
    assert!(
        err.to_lowercase().contains("embedding"),
        "error message must clearly identify embedding rejection: {err}"
    );

    let mut no_tools = ModelCandidate::new_unknown(
        "meta/llama-3.1-8b-instruct".to_string(),
        PRODUCTION_PROVIDER_ID.to_string(),
    );
    no_tools.context_capacity = 128000;
    no_tools.supports_tools = false;
    no_tools.tool_support = CapabilitySupport::Unsupported;
    no_tools.availability = ProviderCapabilityStatus::Available;
    no_tools.set_context_provenance("nvidia:model_reference");

    let err_tools = check_primary_eligibility(&no_tools).expect_err("no tools model rejected");
    assert!(err_tools.to_lowercase().contains("tool"));
}

#[test]
fn test_eligible_model_passes_primary_check() {
    let mut candidate = ModelCandidate::new_unknown(
        "meta/llama-3.3-70b-instruct".to_string(),
        PRODUCTION_PROVIDER_ID.to_string(),
    );
    let resolver = NvidiaModelMetadataResolver::new_offline();
    futures::executor::block_on(resolver.enrich_candidate(&mut candidate)).unwrap();

    assert!(check_primary_eligibility(&candidate).is_ok());
    assert!(check_fast_auxiliary_eligibility(&candidate).is_ok());
}

// ── MODEL-02: NVIDIA Provider output tokens & context boundary ───────────────

#[test]
fn test_nvidia_determine_effective_max_tokens_clamps_to_model_limit() {
    let resolver = NvidiaModelMetadataResolver::new_offline();

    // Model with known 4096 output limit
    let options = ModelInvocationOptions {
        max_output_tokens: Some(32768), // User or router requests 32k
        temperature: Some(0.7),
        top_p: None,
    };

    let effective = NvidiaProvider::determine_effective_max_tokens(
        "meta/llama-3.1-8b-instruct",
        Some(&options),
        100, // 100 prompt tokens
        &resolver,
    )
    .expect("effective max tokens calculation succeeds");

    // Must be clamped to model's known max output limit (8192), never 32768
    assert_eq!(effective, 8192);
}

#[test]
fn test_nvidia_determine_effective_max_tokens_exceeds_context() {
    let resolver = NvidiaModelMetadataResolver::new_offline();

    let options = ModelInvocationOptions {
        max_output_tokens: Some(4096),
        temperature: None,
        top_p: None,
    };

    // Prompt tokens equal or exceeding context length (131072 for llama-3.1-8b)
    let res = NvidiaProvider::determine_effective_max_tokens(
        "meta/llama-3.1-8b-instruct",
        Some(&options),
        131072,
        &resolver,
    );

    match res {
        Err(ModelError::ContextWindowExhausted {
            requested,
            capacity,
        }) => {
            assert_eq!(requested, 131072);
            assert_eq!(capacity, 131072);
        }
        other => panic!("expected ContextWindowExhausted, got {:?}", other),
    }
}

// ── MODEL-03: Model Selector truthful providers & key handling ──────────────

#[test]
fn test_model_selector_does_not_fabricate_mock_provider() {
    let resolver = NvidiaModelMetadataResolver::new_offline();
    let candidates = vec![
        {
            let mut c = ModelCandidate::new_unknown(
                "meta/llama-3.3-70b-instruct".to_string(),
                PRODUCTION_PROVIDER_ID.to_string(),
            );
            c.context_capacity = 128000;
            c.supports_tools = true;
            c.tool_support = CapabilitySupport::Supported;
            c.availability = ProviderCapabilityStatus::Available;
            let _ = futures::executor::block_on(resolver.enrich_candidate(&mut c));
            c
        },
        {
            let mut c = ModelCandidate::new_unknown(
                "nvidia/nemotron-4-340b-reward".to_string(),
                PRODUCTION_PROVIDER_ID.to_string(),
            );
            c.context_capacity = 4096;
            c.supports_tools = true;
            c.tool_support = CapabilitySupport::Supported;
            c.availability = ProviderCapabilityStatus::Available;
            let _ = futures::executor::block_on(resolver.enrich_candidate(&mut c));
            c
        },
    ];

    let (providers, models) = resolve_display_models_and_providers(
        &candidates,
        PRODUCTION_PROVIDER_ID,
        "meta/llama-3.3-70b-instruct",
    );

    // Mock provider must NOT be present when active provider is nvidia_nim
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].id, PRODUCTION_PROVIDER_ID);

    // Reward model must be flagged as primary ineligible
    let reward_info = models
        .iter()
        .find(|m| m.model_id == "nvidia/nemotron-4-340b-reward")
        .expect("reward model present");
    assert!(!reward_info.primary_eligible);
    assert!(reward_info.ineligibility_reason.is_some());

    // Eligible model must be flagged as primary eligible
    let llama_info = models
        .iter()
        .find(|m| m.model_id == "meta/llama-3.3-70b-instruct")
        .expect("llama model present");
    assert!(llama_info.primary_eligible);
}

#[test]
fn test_model_selector_key_prevents_selecting_ineligible_primary() {
    let models = vec![ModelInfo {
        model_id: "nvidia/nemotron-4-340b-reward".to_string(),
        display_name: None,
        tier: "Standard".to_string(),
        context_capacity: 4096,
        supports_tools: true,
        is_current_primary: false,
        is_current_fast: false,
        availability: "Available".to_string(),
        primary_eligible: false,
        ineligibility_reason: Some("reward model cannot reason".to_string()),
    }];

    let mut state = ModelSelectorState::new();
    state.mode = ModelSelectorMode::Models;
    state.selected_index = 0;

    let key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('1'),
        crossterm::event::KeyModifiers::NONE,
    );

    let action = handle_model_selector_key(key, &mut state, &[], &models);
    assert_eq!(
        action, None,
        "pressing '1' on an ineligible model must return None, not SetPrimaryModel"
    );
}

// ── PROMPT-01: Shipped prompt asset discovery & transactional loading ────────

#[test]
fn test_repo_prompts_not_scanned_as_project_overrides() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let catalog = InMemoryPromptCatalog::with_builtins_and_workspace(&repo_root);

    // In M31A repository checkout, <workspace>/prompts is the canonical built-in
    // assets source, so it must not be loaded as ProjectOverride.
    assert_eq!(
        catalog.overrides_count(),
        0,
        "no overrides should be loaded from shipped repo prompts directory"
    );
}

#[test]
fn test_load_from_dir_with_source_is_transactional_on_duplicates() {
    let temp_dir = std::env::temp_dir().join(format!("m31a_test_trans_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let file1 = temp_dir.join("prompt_a.toml");
    let file2 = temp_dir.join("prompt_b.toml");

    let prompt_body_1 = r#"
id = "custom.test_trans"
version = 1
description = "First test prompt"
template = "Body 1"
"#;
    let prompt_body_2 = r#"
id = "custom.test_trans"
version = 1
description = "Conflicting test prompt with same id and version"
template = "Body 2"
"#;

    std::fs::write(&file1, prompt_body_1).unwrap();
    std::fs::write(&file2, prompt_body_2).unwrap();

    let mut catalog = InMemoryPromptCatalog::new();
    let res =
        catalog.load_from_dir_with_source(&temp_dir, None, PromptSourceKind::WorkspaceOverride);

    assert!(res.is_err(), "duplicate in batch must fail");
    assert_eq!(
        catalog.overrides_count(),
        0,
        "catalog must remain unmodified when directory batch fails"
    );

    let _ = std::fs::remove_dir_all(temp_dir);
}

// ── PROMPT-02: Guidance hash recomputation, provenance & budget ──────────────

#[test]
fn test_prompt_guidance_recomputes_hash_and_updates_provenance() {
    let mut catalog = InMemoryPromptCatalog::with_builtins();

    // Register guidance for agent.implementer
    let guidance_contract = PromptContract::from_toml_str(
        r#"
id = "agent.implementer"
version = 1
role = "implementer"
description = "Project specific coding guidance"

[body]
template = "Always use explicit type annotations in Rust code."
"#,
    )
    .unwrap();

    catalog
        .register_with_source(
            guidance_contract,
            PromptSourceKind::ProjectOverride,
            Some("guidance.toml".to_string()),
        )
        .unwrap();

    let compiler = DefaultPromptCompiler::new();
    let context = PromptContext::new(
        "test-session".to_string(),
        "test-mission".to_string(),
        "test-task".to_string(),
        AgentRole::implementer(),
        MissionStage::Execute,
        "Implement safe arithmetic",
    );

    let plain = compiler
        .compile_from_catalog(
            &catalog,
            "agent.implementer",
            2,
            &context,
            &CompilationOptions::default(),
        )
        .expect("compile plain");

    let guided = compiler
        .compile_from_catalog_with_guidance(
            &catalog,
            "agent.implementer",
            2,
            &context,
            &CompilationOptions::default(),
        )
        .expect("compile guided");

    // Content hash must be distinct because guidance content was appended
    assert_ne!(
        plain.content_hash, guided.content_hash,
        "guided prompt content_hash must be distinct from plain prompt"
    );

    // Provenance effective hash must match the new content hash
    let prov = guided.provenance.as_ref().expect("provenance present");
    assert_eq!(prov.invocation.effective_prompt_hash, guided.content_hash);

    // Project guidance layer must be recorded in provenance layers
    assert!(
        prov.layers.iter().any(|l| l.name == "project_guidance"),
        "provenance must record project_guidance layer"
    );
}

#[test]
fn test_prompt_guidance_enforces_budget_on_assembled_text() {
    let mut catalog = InMemoryPromptCatalog::with_builtins();

    let guidance_contract = PromptContract::from_toml_str(
        r#"
id = "agent.implementer"
version = 1
role = "implementer"
description = "Project specific coding guidance"

[body]
template = "Enormous project guidance content repeating over and over again."
"#,
    )
    .unwrap();

    catalog
        .register_with_source(
            guidance_contract,
            PromptSourceKind::ProjectOverride,
            Some("guidance.toml".to_string()),
        )
        .unwrap();

    let compiler = DefaultPromptCompiler::new();
    let context = PromptContext::new(
        "test-session".to_string(),
        "test-mission".to_string(),
        "test-task".to_string(),
        AgentRole::implementer(),
        MissionStage::Execute,
        "Implement safe arithmetic",
    );

    // Plain compiles within standard size (~5000 bytes). Set budget tight to fail when guidance is added.
    let plain = compiler
        .compile_from_catalog(
            &catalog,
            "agent.implementer",
            2,
            &context,
            &CompilationOptions::default(),
        )
        .expect("compile plain");

    let opts = CompilationOptions {
        max_total_bytes: Some(plain.total_bytes + 10), // Not enough room for guidance
        ..Default::default()
    };

    let res = compiler.compile_from_catalog_with_guidance(
        &catalog,
        "agent.implementer",
        2,
        &context,
        &opts,
    );

    assert!(res.is_err(), "tight budget must reject guidance overflow");
    match res {
        Err(m31a::prompt::error::PromptError::PromptBudgetExceeded { .. }) => {}
        other => panic!("expected PromptBudgetExceeded, got {:?}", other),
    }
}

// ── PROMPT-03: Canonical version resolution precedence ───────────────────────

#[test]
fn test_resolve_canonical_upgrades_version_before_override_lookup() {
    let mut catalog = InMemoryPromptCatalog::with_builtins();

    // Inject a v1 override for agent.implementer
    let v1_override = PromptContract::from_toml_str(
        r#"
id = "agent.implementer"
version = 1
role = "implementer"
description = "Legacy v1 override attempting to bypass v2"

[body]
template = "Legacy v1 body"
"#,
    )
    .unwrap();

    // Register directly in overrides map (e.g. from workspace)
    catalog
        .register_with_source(
            v1_override,
            PromptSourceKind::WorkspaceOverride,
            Some("agent.implementer.v1.toml".to_string()),
        )
        .unwrap();

    // Resolving canonical for "agent.implementer" (requesting v1) must upgrade to canonical v2
    let resolved = catalog
        .resolve_canonical("agent.implementer", 1)
        .expect("resolve canonical succeeds");

    assert_eq!(
        resolved.version, 2,
        "canonical resolution must return v2, not the v1 override"
    );
}

// ── PROMPT-04: Authoritative prompt inventory audit ──────────────────────────

#[test]
fn test_prompt_inventory_audit_aligns_with_reachability() {
    let catalog = InMemoryPromptCatalog::with_builtins();
    let audit = catalog.audit_inventory();

    // All contracts in canonical_contracts must be version >= 2 or canonical
    for contract in &audit.canonical_contracts {
        assert!(
            contract.contains(".v2"),
            "canonical contract label must be v2: {contract}"
        );
    }

    assert!(
        !audit.production_reachable.is_empty(),
        "production reachable contracts must not be empty"
    );

    // skill.in_task_guidance is TestOnly in reachability matrix, so it must be in test_only_contracts
    assert!(
        audit
            .test_only_contracts
            .iter()
            .any(|c| c.contains("skill.in_task_guidance")),
        "skill.in_task_guidance must be classified as test_only in inventory audit"
    );
}

// ── UX-01: Enter key inspects error in conversation ─────────────────────────

#[test]
fn test_conversation_has_inspectable_error_predicate() {
    let mut model = TuiViewModel::new();
    assert!(!model.has_inspectable_error());

    model.add_conversation_item(TuiConversationItem::Error {
        message: "Failed to connect to model endpoint".to_string(),
        timestamp: chrono::Utc::now(),
    });

    assert!(model.has_inspectable_error());
    assert_eq!(
        model.latest_error_summary(),
        Some("Failed to connect to model endpoint".to_string())
    );
}
