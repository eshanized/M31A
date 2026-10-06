//! Integration tests verifying User Freedom, Configurability, and Unbounded Execution.
//!
//! Validates:
//! 1. User A (Balanced, Git OFF, Approvals ON, Unlimited budget)
//! 2. User B (Autonomous, Git ON, Auto-commit OFF, Custom budget)
//! 3. User C (Code Reviewer, Git OFF, Read-only sandbox)
//! 4. Non-Git workspace onboarding without mandatory git init or doctor failures
//! 5. Unlimited resource budget end-to-end ("Never Arbitrarily Stop Coding")
//! 6. NVIDIA NIM production restriction alongside full model configurability

use std::fs;
use tempfile::tempdir;

use m31a::budget::{ActualUsage, BudgetEnforcer, TaskEstimates};
use m31a::config::resolved::ResolvedConfiguration;
use m31a::config::schema::parse_and_validate_config;
use m31a::init::doctor::{DiagnosticStatus, DoctorEngine};
use m31a::init::lifecycle::SetupStep;
use m31a::state::budget::ResourceBudget;
use m31a::tui::screens::wizard::{SetupWizardScreen, WizardProfile};

#[test]
fn test_user_a_balanced_git_off_unlimited_budget() {
    let dir = tempdir().expect("tempdir");
    let ws_path = dir.path().to_path_buf();

    let mut wizard = SetupWizardScreen::new(ws_path.clone());
    wizard.trust_confirmed = true;
    wizard.git_enabled = false;
    wizard.profile = WizardProfile::Balanced;
    wizard.require_approval_for_writes = true;
    wizard.unlimited_budget = true;
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wizard
        .fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");

    wizard
        .persist_configuration()
        .expect("persist configuration");

    let config_path = ws_path.join(".m31a").join("config.toml");
    assert!(config_path.is_file());
    let content = fs::read_to_string(&config_path).expect("read config.toml");

    let parsed = parse_and_validate_config(&content).expect("validate config");
    assert_eq!(parsed.profile.as_deref(), Some("balanced"));
    assert!(!parsed.git.enabled, "Git must be disabled");
    assert!(
        parsed.policy.interactive_approvals,
        "Approvals must be active"
    );
    assert!(
        parsed.budget.max_cost_usd.is_none(),
        "Cost budget must be unbounded"
    );
    assert!(
        parsed.budget.max_agent_steps.is_none(),
        "Steps budget must be unbounded"
    );
    assert!(
        parsed.budget.max_tokens.is_none(),
        "Token budget must be unbounded"
    );

    // Verify resolved configuration propagates settings
    let resolved = ResolvedConfiguration::for_workspace(&ws_path).expect("resolve config");
    assert_eq!(resolved.active_profile.as_deref(), Some("balanced"));
    assert!(!resolved.app_config.git.enabled);
    assert!(resolved.app_config.budget.max_cost_usd.is_none());
}

#[test]
fn test_user_b_autonomous_git_on_custom_budget() {
    let dir = tempdir().expect("tempdir");
    let ws_path = dir.path().to_path_buf();

    let mut wizard = SetupWizardScreen::new(ws_path.clone());
    wizard.trust_confirmed = true;
    wizard.git_enabled = true;
    wizard.git_auto_commit = false;
    wizard.git_push_policy = m31a::config::schema::GitPushPolicy::Deny;
    wizard.git_execution_isolation = m31a::config::schema::GitExecutionIsolation::BestEffort;
    wizard.profile = WizardProfile::Autonomous;
    wizard.require_approval_for_writes = false;
    wizard.unlimited_budget = false;
    wizard.max_budget_dollars = 50;
    wizard.max_agent_steps = Some(100);
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");

    wizard
        .persist_configuration()
        .expect("persist configuration");

    let config_path = ws_path.join(".m31a").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config.toml");

    let parsed = parse_and_validate_config(&content).expect("validate config");
    assert_eq!(parsed.profile.as_deref(), Some("autonomous"));
    assert!(parsed.git.enabled, "Git must be enabled");
    assert!(!parsed.git.auto_commit, "Auto commit must be disabled");
    assert_eq!(
        parsed.git.push_policy,
        m31a::config::schema::GitPushPolicy::Deny
    );
    assert_eq!(
        parsed.git.execution_isolation,
        m31a::config::schema::GitExecutionIsolation::BestEffort
    );
    assert!(!parsed.policy.interactive_approvals);
    assert_eq!(parsed.budget.max_cost_usd, Some(50.0));
    assert_eq!(parsed.budget.max_agent_steps, Some(100));

    let resolved = ResolvedConfiguration::for_workspace(&ws_path).expect("resolve config");
    assert_eq!(resolved.active_profile.as_deref(), Some("autonomous"));
    assert!(resolved.app_config.git.enabled);
    assert_eq!(resolved.app_config.budget.max_cost_usd, Some(50.0));
    assert_eq!(resolved.app_config.budget.max_agent_steps, Some(100));
}

#[test]
fn test_user_c_code_reviewer_git_off_read_only() {
    let dir = tempdir().expect("tempdir");
    let ws_path = dir.path().to_path_buf();

    let mut wizard = SetupWizardScreen::new(ws_path.clone());
    wizard.trust_confirmed = true;
    wizard.git_enabled = false;
    wizard.profile = WizardProfile::CodeReviewer;
    wizard.sandbox_mode = "strict".to_string();
    wizard.require_approval_for_writes = true;
    wizard.unlimited_budget = true;
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");

    wizard
        .persist_configuration()
        .expect("persist configuration");

    let config_path = ws_path.join(".m31a").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config.toml");

    let parsed = parse_and_validate_config(&content).expect("validate config");
    assert_eq!(parsed.profile.as_deref(), Some("code_reviewer"));
    assert!(!parsed.git.enabled);
    assert_eq!(parsed.policy.sandbox_mode.as_deref(), Some("strict"));

    let resolved = ResolvedConfiguration::for_workspace(&ws_path).expect("resolve config");
    assert_eq!(resolved.active_profile.as_deref(), Some("code_reviewer"));
    assert_eq!(
        resolved.app_config.policy.sandbox_mode.as_deref(),
        Some("strict")
    );
}

#[test]
fn test_workspace_without_git_repository_diagnostics_and_onboarding() {
    let dir = tempdir().expect("tempdir");
    let ws_path = dir.path().to_path_buf();
    assert!(!ws_path.join(".git").exists());

    // 1. DoctorEngine diagnostic check with Git disabled
    let doctor = DoctorEngine::new();
    let probes = doctor.run_all_with_git_enabled(&ws_path, false);

    let git_probe = probes
        .iter()
        .find(|p| p.id == "git_installed")
        .expect("git_installed probe");
    let repo_probe = probes
        .iter()
        .find(|p| p.id == "git_repository")
        .expect("git_repository probe");

    assert_eq!(
        git_probe.status,
        DiagnosticStatus::Disabled,
        "Git probe must report Disabled when git.enabled is false"
    );
    assert_eq!(
        repo_probe.status,
        DiagnosticStatus::Disabled,
        "Repo probe must report Disabled when git.enabled is false"
    );
    assert!(
        !doctor.has_blocking_failures(&probes),
        "Disabled git must NOT cause blocking failure"
    );

    // 2. SetupWizardScreen onboarding in non-git directory
    let mut wizard = SetupWizardScreen::new(ws_path);
    assert!(!wizard.git_info.is_git_repo);
    assert!(!wizard.git_enabled, "Should default to false when no .git");

    // Advance Step 1 (Trust)
    wizard.trust_confirmed = true;
    assert!(
        wizard.advance(),
        "Should advance from Step 1 without requiring git"
    );
    assert_eq!(wizard.current_step(), SetupStep::DoctorDiagnostics);

    // Advance Step 2 (Doctor)
    assert!(
        wizard.can_advance(),
        "Doctor step must be advanceable because git check is disabled"
    );
    assert!(wizard.advance());
    assert_eq!(wizard.current_step(), SetupStep::ProviderSetup);
}

#[test]
fn test_unlimited_resource_budget_never_halts() {
    let budget = ResourceBudget::unbounded();
    assert!(budget.max_cost_usd.is_none());
    assert!(budget.max_agent_steps.is_none());
    assert!(budget.max_tokens.is_none());
    assert!(budget.max_model_calls.is_none());
    assert!(budget.max_wall_clock_seconds.is_none());
    assert!(budget.max_retries.is_none());
    assert!(budget.max_concurrent_agents.is_none());

    let enforcer = BudgetEnforcer::new(budget);

    // Reserve massive workload exceeding typical fixed caps ($500 cost, 100M tokens)
    let estimate = TaskEstimates {
        estimated_tokens: 100_000_000,
        estimated_cost_usd: 500.0,
        requires_worker: true,
        estimated_artifact_bytes: 1_000_000_000,
    };

    let receipt = enforcer
        .reserve(&estimate, false)
        .expect("Reservation must succeed under unbounded budget");

    // Settle large actual usage
    let actual = ActualUsage {
        steps: 1500,
        calls: 800,
        tokens: 120_000_000,
        cost_usd: 650.0,
        artifact_bytes: 800_000_000,
        retries: 0,
    };

    enforcer.settle(&receipt, &actual);
    let snapshot = enforcer.snapshot();

    assert_eq!(snapshot.tokens_consumed, 120_000_000);
    assert!(snapshot.cost_consumed_usd >= 649.99);
}

#[test]
fn test_nvidia_nim_production_restriction_and_model_freedom() {
    // 1. Mandatory provider restriction: other providers must fail validation
    let bad_config = r#"
[provider]
default = "openai"

[agents]
default_model = "gpt-4o"
"#;
    let res = parse_and_validate_config(bad_config);
    assert!(
        res.is_err(),
        "Non-NVIDIA provider selection must be rejected"
    );

    // 2. NVIDIA NIM is accepted with custom user-selected model
    let good_config = r#"
[provider]
default = "nvidia_nim"

[agents]
default_model = "custom/any-specialized-developer-model"
fast_auxiliary_model = "custom/my-fast-linter"
"#;
    let validated = parse_and_validate_config(good_config).expect("valid config");
    assert_eq!(validated.provider.default, "nvidia_nim");
    assert_eq!(
        validated.agents.default_model,
        "custom/any-specialized-developer-model"
    );
    assert_eq!(
        validated.agents.fast_auxiliary_model.as_deref(),
        Some("custom/my-fast-linter")
    );
}
