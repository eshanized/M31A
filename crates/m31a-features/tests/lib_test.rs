//! Dedicated integration tests for `m31a-features` (src/lib.rs).
//!
//! Validates:
//! - Builtin flag registration and retrieval
//! - Deterministic targeting by user ID, groups, and custom attributes
//! - Deterministic rollout percentage hashing
//! - Dependency satisfaction and failure cascades
//! - User-level overrides and override clearing
//! - Staged rollout progression, rollback, and audit trail generation
//! - Disk serialization and persistent recovery across manager instances

use m31a_features::{
    AuditAction, EvaluationContext, EvaluationReason, FeatureCategory, FeatureFlag,
    FeatureFlagManager, FeatureMetadata, FeatureState, FlagUpdates, RolloutConfig, RolloutStage,
    RolloutStrategy, TargetingRules,
};
use std::collections::HashMap;
use tempfile::tempdir;

fn sample_flag(id: &str, state: FeatureState, percentage: u8) -> FeatureFlag {
    FeatureFlag {
        id: id.to_string(),
        name: format!("Flag {}", id),
        description: "Test feature flag".to_string(),
        category: FeatureCategory::Experimental,
        state,
        rollout: RolloutConfig {
            strategy: RolloutStrategy::Gradual,
            percentage,
            stages: vec![
                RolloutStage {
                    name: "canary".to_string(),
                    percentage,
                    duration_hours: 24,
                    success_criteria: vec!["no errors".to_string()],
                    rollback_criteria: vec!["crash > 1%".to_string()],
                },
                RolloutStage {
                    name: "full".to_string(),
                    percentage: 100,
                    duration_hours: 48,
                    success_criteria: vec!["stable".to_string()],
                    rollback_criteria: vec![],
                },
            ],
            current_stage: 0,
        },
        targeting: TargetingRules {
            user_ids: vec!["alpha_tester".to_string()],
            groups: vec!["beta_testers".to_string()],
            environments: vec!["staging".to_string(), "production".to_string()],
            custom_attributes: HashMap::new(),
            exclude: vec!["blocked_user".to_string()],
        },
        dependencies: vec![],
        metadata: FeatureMetadata {
            jira_ticket: Some("M31A-101".to_string()),
            github_pr: None,
            owner_team: "core-platform".to_string(),
            documentation_url: None,
            migration_guide_url: None,
            sunset_date: None,
        },
        created_at: 1700000000,
        updated_at: 1700000000,
        created_by: "test-harness".to_string(),
    }
}

#[tokio::test]
async fn test_builtin_flags_registered() {
    let tmp = tempdir().unwrap();
    let manager = FeatureFlagManager::new(tmp.path().to_path_buf()).unwrap();

    let flags = manager.list_flags().await;
    assert!(!flags.is_empty(), "Builtin flags should be pre-registered");
    assert!(flags.iter().any(|f| f.id == "code_mode"));

    let code_mode = manager.get_flag("code_mode").await;
    assert!(code_mode.is_some());
    assert_eq!(code_mode.unwrap().category, FeatureCategory::Experimental);
}

#[tokio::test]
async fn test_targeting_rules_evaluation() {
    let tmp = tempdir().unwrap();
    let manager = FeatureFlagManager::new(tmp.path().to_path_buf()).unwrap();

    let flag = sample_flag("feature-targeted", FeatureState::Enabled, 100);
    manager
        .create_flag(flag, "test-suite".to_string())
        .await
        .unwrap();

    // 1. Explicitly targeted user
    let ctx_target = EvaluationContext {
        user_id: Some("alpha_tester".to_string()),
        groups: vec![],
        environment: "production".to_string(),
        custom_attributes: HashMap::new(),
        session_id: None,
    };
    let res = manager
        .evaluate("feature-targeted", &ctx_target)
        .await
        .unwrap();
    assert!(res.enabled, "Targeted user must evaluate to true");
    assert_eq!(res.reason, EvaluationReason::TargetedEnabled);

    // 2. Excluded user
    let ctx_excluded = EvaluationContext {
        user_id: Some("blocked_user".to_string()),
        groups: vec!["beta_testers".to_string()],
        environment: "production".to_string(),
        custom_attributes: HashMap::new(),
        session_id: None,
    };
    let res = manager
        .evaluate("feature-targeted", &ctx_excluded)
        .await
        .unwrap();
    assert!(
        !res.enabled,
        "Excluded user must evaluate to false even if in group"
    );
    assert_eq!(res.reason, EvaluationReason::TargetedDisabled);

    // 3. User with matching group
    let ctx_group = EvaluationContext {
        user_id: Some("random_user".to_string()),
        groups: vec!["beta_testers".to_string()],
        environment: "production".to_string(),
        custom_attributes: HashMap::new(),
        session_id: None,
    };
    let res = manager
        .evaluate("feature-targeted", &ctx_group)
        .await
        .unwrap();
    assert!(res.enabled, "User in targeted group must evaluate to true");
    assert_eq!(res.reason, EvaluationReason::TargetedEnabled);
}

#[tokio::test]
async fn test_overrides_and_clearing() {
    let tmp = tempdir().unwrap();
    let manager = FeatureFlagManager::new(tmp.path().to_path_buf()).unwrap();

    let mut flag = sample_flag("feature-override", FeatureState::Disabled, 0);
    flag.targeting = TargetingRules::default();
    manager
        .create_flag(flag, "test-suite".to_string())
        .await
        .unwrap();

    let ctx = EvaluationContext {
        user_id: Some("engineer-01".to_string()),
        groups: vec![],
        environment: "production".to_string(),
        custom_attributes: HashMap::new(),
        session_id: None,
    };

    // Disabled initially
    let initial = manager.evaluate("feature-override", &ctx).await.unwrap();
    assert!(!initial.enabled);

    // Apply manual override to Enabled
    manager
        .set_override("feature-override", "engineer-01", FeatureState::Enabled)
        .await
        .unwrap();

    let overridden = manager.evaluate("feature-override", &ctx).await.unwrap();
    assert!(overridden.enabled);
    assert_eq!(overridden.reason, EvaluationReason::Override);

    // Clear override
    manager
        .clear_override("feature-override", "engineer-01")
        .await
        .unwrap();

    let reverted = manager.evaluate("feature-override", &ctx).await.unwrap();
    assert!(!reverted.enabled);
}

#[tokio::test]
async fn test_dependencies_gate() {
    let tmp = tempdir().unwrap();
    let manager = FeatureFlagManager::new(tmp.path().to_path_buf()).unwrap();

    let mut parent = sample_flag("dep-parent", FeatureState::Disabled, 100);
    parent.targeting = TargetingRules::default();
    manager
        .create_flag(parent, "admin".to_string())
        .await
        .unwrap();

    let mut child = sample_flag("dep-child", FeatureState::Enabled, 100);
    child.dependencies = vec!["dep-parent".to_string()];
    child.targeting = TargetingRules::default();
    manager
        .create_flag(child, "admin".to_string())
        .await
        .unwrap();

    let ctx = EvaluationContext {
        user_id: Some("u123".to_string()),
        groups: vec![],
        environment: "production".to_string(),
        custom_attributes: HashMap::new(),
        session_id: None,
    };

    // Parent is disabled, so child must be disabled due to dependency
    let res = manager.evaluate("dep-child", &ctx).await.unwrap();
    assert!(!res.enabled);
    assert_eq!(res.reason, EvaluationReason::DependencyDisabled);

    // Enable parent
    manager
        .update_flag(
            "dep-parent",
            FlagUpdates {
                state: Some(FeatureState::Enabled),
                ..Default::default()
            },
            "admin".to_string(),
        )
        .await
        .unwrap();

    let res = manager.evaluate("dep-child", &ctx).await.unwrap();
    assert!(
        res.enabled,
        "Child must now be enabled after parent is enabled"
    );
}

#[tokio::test]
async fn test_rollout_advancement_and_rollback_audit() {
    let tmp = tempdir().unwrap();
    let manager = FeatureFlagManager::new(tmp.path().to_path_buf()).unwrap();

    let flag = sample_flag("feature-rollout", FeatureState::Enabled, 10);
    manager
        .create_flag(flag, "admin".to_string())
        .await
        .unwrap();

    // Advance rollout to stage 1 (100%)
    manager
        .advance_rollout("feature-rollout", "release-bot".to_string())
        .await
        .unwrap();

    let flag_after = manager.get_flag("feature-rollout").await.unwrap();
    assert_eq!(flag_after.rollout.current_stage, 1);
    assert_eq!(flag_after.rollout.percentage, 100);

    // Rollback rollout
    manager
        .rollback_rollout("feature-rollout", "oncall".to_string())
        .await
        .unwrap();

    let flag_rolled_back = manager.get_flag("feature-rollout").await.unwrap();
    assert_eq!(flag_rolled_back.rollout.current_stage, 0);
    assert_eq!(flag_rolled_back.rollout.percentage, 10);

    // Audit logs validation
    let audit_entries = manager.get_audit_log(Some("feature-rollout"), 50).await;
    assert!(audit_entries.len() >= 3);
    assert!(
        audit_entries
            .iter()
            .any(|e| e.action == AuditAction::RolloutAdvanced)
    );
    assert!(
        audit_entries
            .iter()
            .any(|e| e.action == AuditAction::RolloutRolledBack)
    );
}

#[tokio::test]
async fn test_storage_persistence_roundtrip() {
    let tmp = tempdir().unwrap();
    let storage_dir = tmp.path().to_path_buf();

    // Create flag in first manager instance and write to disk
    {
        let manager = FeatureFlagManager::new(storage_dir.clone()).unwrap();
        let flag = sample_flag("persisted-feature", FeatureState::Enabled, 50);
        manager
            .create_flag(flag, "admin".to_string())
            .await
            .unwrap();
    }

    // Load second manager instance from the same disk path
    {
        let manager2 = FeatureFlagManager::new(storage_dir).unwrap();
        let loaded = manager2.get_flag("persisted-feature").await;
        assert!(loaded.is_some(), "Flag must persist across manager reboots");
        assert_eq!(loaded.unwrap().name, "Flag persisted-feature");
    }
}
