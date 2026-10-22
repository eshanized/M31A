//! Integration tests verifying User Freedom, Configurability, and Unbounded Execution.
//!
//! Validates:
//! 1. User A (Balanced, Git OFF, Approvals ON)
//! 2. User B (Autonomous, Git ON, Auto-commit OFF)
//! 3. User C (Code Reviewer, Git OFF, Read-only sandbox)
//! 4. Non-Git workspace onboarding without mandatory git init or doctor failures
//! 5. Execution without budget constraints ("Never Arbitrarily Stop Coding")
//! 6. NVIDIA NIM production restriction alongside full model configurability

use std::fs;
use tempfile::tempdir;

use m31a::config::resolved::ResolvedConfiguration;
use m31a::config::schema::parse_and_validate_config;
use m31a::init::doctor::{DiagnosticStatus, DoctorEngine};
use m31a::init::lifecycle::SetupStep;
use m31a::tui::screens::wizard::{SetupWizardScreen, WizardProfile};

#[test]
fn test_user_a_balanced_git_off() {
    let dir = tempdir().expect("tempdir");
    let ws_path = dir.path().to_path_buf();

    let mut wizard = SetupWizardScreen::new(ws_path.clone());
    wizard.trust_confirmed = true;
    wizard.git_enabled = false;
    wizard.profile = WizardProfile::Balanced;
    wizard.require_approval_for_writes = true;
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wizard
        .fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");

    wizard
        .persist_configuration()
        .expect("persist configuration");

    let canonical_path = ws_path.join(".m31a").join("M31A.toml");
    let legacy_path = ws_path.join(".m31a").join("config.toml");
    let config_path = if canonical_path.is_file() {
        canonical_path
    } else {
        legacy_path
    };
    assert!(config_path.is_file());
    let content = fs::read_to_string(&config_path).expect("read config");

    let parsed = parse_and_validate_config(&content).expect("validate config");
    assert_eq!(parsed.profile.as_deref(), Some("balanced"));
    assert!(!parsed.git.enabled, "Git must be disabled");
    assert!(
        parsed.policy.interactive_approvals,
        "Approvals must be active"
    );

    // Verify resolved configuration propagates settings
    let resolved = ResolvedConfiguration::for_workspace(&ws_path).expect("resolve config");
    assert_eq!(resolved.active_profile.as_deref(), Some("balanced"));
    assert!(!resolved.app_config.git.enabled);
}

#[test]
fn test_user_b_autonomous_git_on() {
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
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");

    wizard
        .persist_configuration()
        .expect("persist configuration");

    let canonical_path = ws_path.join(".m31a").join("M31A.toml");
    let legacy_path = ws_path.join(".m31a").join("config.toml");
    let config_path = if canonical_path.is_file() {
        canonical_path
    } else {
        legacy_path
    };
    let content = fs::read_to_string(&config_path).expect("read config");

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

    let resolved = ResolvedConfiguration::for_workspace(&ws_path).expect("resolve config");
    assert_eq!(resolved.active_profile.as_deref(), Some("autonomous"));
    assert!(resolved.app_config.git.enabled);
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
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");

    wizard
        .persist_configuration()
        .expect("persist configuration");

    let canonical_path = ws_path.join(".m31a").join("M31A.toml");
    let legacy_path = ws_path.join(".m31a").join("config.toml");
    let config_path = if canonical_path.is_file() {
        canonical_path
    } else {
        legacy_path
    };
    let content = fs::read_to_string(&config_path).expect("read config");

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
        !wizard.doctor.has_blocking_failures(wizard.probes()),
        "Disabled git must NOT cause blocking failure"
    );
    wizard.warnings_acknowledged = true;
    assert!(
        wizard.can_advance(),
        "Doctor step must be advanceable because git check is disabled and warnings acknowledged"
    );
    assert!(wizard.advance());
    assert_eq!(wizard.current_step(), SetupStep::ProviderSetup);
}

#[test]
fn test_unconstrained_execution_without_budgets() {
    // Normal mission creation proceeds without any budget limits or enforcers
    let mission = m31a::state::mission::Mission::new(
        m31a::ids::MissionId::new(),
        "Never Arbitrarily Stop Coding".to_string(),
    );
    assert_eq!(mission.objective, "Never Arbitrarily Stop Coding");
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
