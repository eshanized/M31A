//! Runtime invariant assertions (KRN-05)
//!
//! Per D-18, kernel/ owns only cross-cutting runtime invariants.
//! Per KRN-05, runtime invariants are enforced and directly tested.

use crate::state_machine::{MissionState, TaskState};

/// RuntimeInvariants bundles all invariant assertion methods for convenient use.
#[derive(Debug, Clone, Copy, Default)]
pub struct RuntimeInvariants;

impl RuntimeInvariants {
    /// Assert that a task is not in a blocked state.
    /// Panics if task status is Blocked.
    pub fn assert_task_not_blocked(&self, task_status: &TaskState) {
        assert!(
            !matches!(task_status, TaskState::Blocked),
            "Invariant violation: Blocked task cannot execute (status: {:?})",
            task_status
        );
    }

    /// Assert that a task has not already succeeded.
    /// Panics if task status is Succeeded.
    pub fn assert_task_not_succeeded(&self, task_status: &TaskState) {
        assert!(
            !matches!(task_status, TaskState::Succeeded),
            "Invariant violation: Already succeeded task cannot execute again (status: {:?})",
            task_status
        );
    }

    /// Assert that a mission is not completed.
    /// Panics if mission status is Completed.
    pub fn assert_mission_not_completed(&self, mission_status: &MissionState) {
        assert!(
            !matches!(mission_status, MissionState::Completed),
            "Invariant violation: Completed mission cannot be modified (status: {:?})",
            mission_status
        );
    }

    /// Assert that a task is not in a terminal state.
    /// Panics if task status is Succeeded, Failed, Skipped, or Cancelled.
    pub fn assert_not_terminal(&self, task_status: &TaskState) {
        assert!(
            !task_status.is_terminal(),
            "Invariant violation: Terminal task cannot transition (status: {:?})",
            task_status
        );
    }

    /// Assert that a side effect has a policy decision.
    /// Panics if policy_decided is false.
    pub fn assert_side_effect_has_policy(&self, tool_name: &str, policy_decided: bool) {
        assert!(
            policy_decided,
            "Invariant violation: Side effect '{}' requires policy decision",
            tool_name
        );
    }
}

/// Standalone assertion functions for direct use without RuntimeInvariants struct.
/// Assert that a task is not in a blocked state.
pub fn assert_task_not_blocked(task_status: &TaskState) {
    RuntimeInvariants.assert_task_not_blocked(task_status)
}

/// Assert that a task has not already succeeded.
pub fn assert_task_not_succeeded(task_status: &TaskState) {
    RuntimeInvariants.assert_task_not_succeeded(task_status)
}

/// Assert that a mission is not completed.
pub fn assert_mission_not_completed(mission_status: &MissionState) {
    RuntimeInvariants.assert_mission_not_completed(mission_status)
}

/// Assert that a task is not in a terminal state.
pub fn assert_not_terminal(task_status: &TaskState) {
    RuntimeInvariants.assert_not_terminal(task_status)
}

/// Assert that a side effect has a policy decision.
pub fn assert_side_effect_has_policy(tool_name: &str, policy_decided: bool) {
    RuntimeInvariants.assert_side_effect_has_policy(tool_name, policy_decided)
}

/// Returns true if a path component is protected runtime or repository metadata (`.git` or `.m31a`).
pub fn is_protected_component(comp: &str) -> bool {
    let c = comp.trim();
    c.eq_ignore_ascii_case(".git") || c.eq_ignore_ascii_case(".m31a")
}

/// Checks if any component in the given path matches a protected component name.
pub fn contains_protected_component(path: &std::path::Path) -> bool {
    for comp in path.components() {
        if matches!(comp, std::path::Component::Normal(c) if is_protected_component(&c.to_string_lossy()))
        {
            return true;
        }
    }
    false
}

/// Returns true if a path targets sensitive credentials, private keys, environment files, or system security configs.
pub fn is_sensitive_or_credential_path(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();
    let lower_path = path_str.to_lowercase().replace('\\', "/");

    // Check individual components
    for comp in path.components() {
        if let std::path::Component::Normal(c) = comp {
            let name = c.to_string_lossy();
            let lower_name = name.to_lowercase();

            // Credential / secret directories
            if lower_name == ".ssh"
                || lower_name == ".aws"
                || lower_name == ".gnupg"
                || lower_name == ".pki"
            {
                return true;
            }

            // Environment files
            if lower_name == ".env"
                || lower_name.starts_with(".env.")
                || lower_name.starts_with(".env_")
                || lower_name.starts_with(".envrc")
            {
                return true;
            }

            // Private keys & certificates
            if lower_name.contains("id_rsa")
                || lower_name.contains("id_ed25519")
                || lower_name.contains("id_ecdsa")
                || lower_name.contains("id_dsa")
            {
                return true;
            }

            // Secret / token credential files
            if lower_name.contains("token")
                || lower_name.contains("secret")
                || lower_name == "credentials.json"
                || lower_name == "credentials-dev.json"
            {
                return true;
            }

            // Shell startup profile tampering
            if lower_name == ".bashrc"
                || lower_name == ".zshrc"
                || lower_name == ".profile"
                || lower_name == ".bash_profile"
                || lower_name == "profile.ps1"
                || lower_name == "microsoft.powershell_profile.ps1"
                || lower_name == "microsoft.vscode_profile.ps1"
            {
                return true;
            }
        }
    }

    // Check system policy and credential paths
    if lower_path.contains("/etc/shadow")
        || lower_path.contains("/etc/gshadow")
        || lower_path.contains("/etc/sudoers")
        || lower_path.contains("/etc/passwd")
        || lower_path.contains("/etc/profile")
        || lower_path.contains("/etc/m31")
        || lower_path.contains("system32/config/sam")
        || lower_path.contains("system32/config/system")
        || lower_path.contains("system32/config/security")
        || lower_path.contains("system32/drivers/etc/hosts")
    {
        return true;
    }

    // Check if filename starts with .env (e.g. unparsed or relative "./.env")
    let file_name = path.file_name().map(|f| f.to_string_lossy().to_lowercase());
    if let Some(ref fname) = file_name {
        if fname.starts_with(".env") {
            return true;
        }
    }

    false
}

/// Returns true if a path targets protected metadata (.git, .m31a) OR sensitive credentials/secrets.
pub fn is_protected_or_sensitive_path(path: &std::path::Path) -> bool {
    contains_protected_component(path) || is_sensitive_or_credential_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_machine::{MissionState, TaskState};
    use std::panic;

    #[test]
    fn test_assert_task_not_blocked_passes_for_valid_states() {
        let valid_states = [
            TaskState::Pending,
            TaskState::Ready,
            TaskState::Running,
            TaskState::Succeeded,
            TaskState::Failed,
            TaskState::Skipped,
            TaskState::Cancelled,
            TaskState::NeedsReview,
        ];
        for state in valid_states {
            // Should not panic
            assert_task_not_blocked(&state);
        }
    }

    #[test]
    fn test_assert_task_not_blocked_panics_for_blocked() {
        let result = panic::catch_unwind(|| {
            assert_task_not_blocked(&TaskState::Blocked);
        });
        assert!(result.is_err(), "Should panic for Blocked state");
    }

    #[test]
    fn test_assert_task_not_succeeded_passes_for_valid_states() {
        let valid_states = [
            TaskState::Pending,
            TaskState::Blocked,
            TaskState::Ready,
            TaskState::Running,
            TaskState::Failed,
            TaskState::Skipped,
            TaskState::Cancelled,
            TaskState::NeedsReview,
        ];
        for state in valid_states {
            // Should not panic
            assert_task_not_succeeded(&state);
        }
    }

    #[test]
    fn test_assert_task_not_succeeded_panics_for_succeeded() {
        let result = panic::catch_unwind(|| {
            assert_task_not_succeeded(&TaskState::Succeeded);
        });
        assert!(result.is_err(), "Should panic for Succeeded state");
    }

    #[test]
    fn test_assert_mission_not_completed_passes_for_valid_states() {
        let valid_states = [
            MissionState::Created,
            MissionState::Understanding,
            MissionState::Researching,
            MissionState::Planning,
            MissionState::Scheduled,
            MissionState::Executing,
            MissionState::Verifying,
            MissionState::Reviewing,
            MissionState::Integrating,
            MissionState::Shipping,
            MissionState::Paused,
            MissionState::Blocked,
            MissionState::Failed,
            MissionState::Cancelled,
        ];
        for state in valid_states {
            // Should not panic
            assert_mission_not_completed(&state);
        }
    }

    #[test]
    fn test_assert_mission_not_completed_panics_for_completed() {
        let result = panic::catch_unwind(|| {
            assert_mission_not_completed(&MissionState::Completed);
        });
        assert!(result.is_err(), "Should panic for Completed state");
    }

    #[test]
    fn test_assert_not_terminal_passes_for_non_terminal_states() {
        let non_terminal = [
            TaskState::Pending,
            TaskState::Blocked,
            TaskState::Ready,
            TaskState::Running,
            TaskState::NeedsReview,
        ];
        for state in non_terminal {
            assert_not_terminal(&state);
        }
    }

    #[test]
    fn test_assert_not_terminal_panics_for_terminal_states() {
        let terminal_states = [
            TaskState::Succeeded,
            TaskState::Failed,
            TaskState::Skipped,
            TaskState::Cancelled,
        ];
        for state in terminal_states {
            let result = panic::catch_unwind(|| {
                assert_not_terminal(&state);
            });
            assert!(
                result.is_err(),
                "Should panic for terminal state {:?}",
                state
            );
        }
    }

    #[test]
    fn test_assert_side_effect_has_policy_passes_when_true() {
        // Should not panic
        assert_side_effect_has_policy("shell", true);
        assert_side_effect_has_policy("file_write", true);
    }

    #[test]
    fn test_assert_side_effect_has_policy_panics_when_false() {
        let result = panic::catch_unwind(|| {
            assert_side_effect_has_policy("shell", false);
        });
        assert!(result.is_err(), "Should panic when policy not decided");
    }

    #[test]
    fn test_runtime_invariants_struct() {
        let invariants = RuntimeInvariants;

        // All should pass
        invariants.assert_task_not_blocked(&TaskState::Running);
        invariants.assert_task_not_succeeded(&TaskState::Running);
        invariants.assert_mission_not_completed(&MissionState::Executing);
        invariants.assert_not_terminal(&TaskState::Running);
        invariants.assert_side_effect_has_policy("test", true);

        // These should panic
        let result = panic::catch_unwind(|| {
            invariants.assert_task_not_blocked(&TaskState::Blocked);
        });
        assert!(result.is_err());

        let result = panic::catch_unwind(|| {
            invariants.assert_task_not_succeeded(&TaskState::Succeeded);
        });
        assert!(result.is_err());

        let result = panic::catch_unwind(|| {
            invariants.assert_mission_not_completed(&MissionState::Completed);
        });
        assert!(result.is_err());

        let result = panic::catch_unwind(|| {
            invariants.assert_not_terminal(&TaskState::Succeeded);
        });
        assert!(result.is_err());

        let result = panic::catch_unwind(|| {
            invariants.assert_side_effect_has_policy("test", false);
        });
        assert!(result.is_err());
    }
}
