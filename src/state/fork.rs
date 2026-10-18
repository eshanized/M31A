//! Shared fork-input resolution for Mission and ExecutionRun.

use std::path::{Path, PathBuf};

use crate::state::intake::{AutonomyMode, NormalizedIntake};
use crate::state::policy_context::PolicyContext;

/// Immutable parent specification used when resolving fork overrides.
pub(crate) struct ForkParent<'a> {
    pub(crate) objective: &'a str,
    pub(crate) constraints: &'a [String],
    pub(crate) requirements: &'a [String],
    pub(crate) success_criteria: &'a [String],
    pub(crate) workspace_root: &'a Path,
    pub(crate) mode: AutonomyMode,
    pub(crate) policy_context: &'a PolicyContext,
}

/// Fully resolved specification for a new independent execution fork.
pub(crate) struct ForkInputs {
    pub(crate) objective: String,
    pub(crate) constraints: Vec<String>,
    pub(crate) requirements: Vec<String>,
    pub(crate) success_criteria: Vec<String>,
    pub(crate) workspace_root: PathBuf,
    pub(crate) mode: AutonomyMode,
    pub(crate) policy_context: PolicyContext,
}

/// Apply the shared fork-override policy without mutating the parent.
///
/// Empty constraints, requirements, and success criteria inherit from the
/// parent. The normalized intake remains authoritative for objective, workspace,
/// autonomy mode, and policy context, matching the pre-refactor behavior.
pub(crate) fn resolve_fork_inputs(
    parent: ForkParent<'_>,
    overrides: Option<NormalizedIntake>,
) -> ForkInputs {
    match overrides {
        Some(normalized) => ForkInputs {
            objective: normalized.objective,
            constraints: if normalized.constraints.is_empty() {
                parent.constraints.to_vec()
            } else {
                normalized.constraints
            },
            requirements: if normalized.requirements.is_empty() {
                parent.requirements.to_vec()
            } else {
                normalized.requirements
            },
            success_criteria: if normalized.success_criteria.is_empty() {
                parent.success_criteria.to_vec()
            } else {
                normalized.success_criteria
            },
            workspace_root: normalized.workspace_root,
            mode: normalized.mode,
            policy_context: normalized.policy_context,
        },
        None => ForkInputs {
            objective: parent.objective.to_owned(),
            constraints: parent.constraints.to_vec(),
            requirements: parent.requirements.to_vec(),
            success_criteria: parent.success_criteria.to_vec(),
            workspace_root: parent.workspace_root.to_path_buf(),
            mode: parent.mode,
            policy_context: parent.policy_context.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent<'a>(
        constraints: &'a [String],
        requirements: &'a [String],
        success_criteria: &'a [String],
        policy_context: &'a PolicyContext,
    ) -> ForkParent<'a> {
        ForkParent {
            objective: "parent objective",
            constraints,
            requirements,
            success_criteria,
            workspace_root: Path::new("/workspace/parent"),
            mode: AutonomyMode::Assisted,
            policy_context,
        }
    }

    fn normalized(
        objective: &str,
        constraints: Vec<String>,
        requirements: Vec<String>,
        success_criteria: Vec<String>,
    ) -> NormalizedIntake {
        NormalizedIntake {
            objective: objective.to_owned(),
            constraints,
            requirements,
            success_criteria,
            workspace_root: PathBuf::from("/workspace/child"),
            repository_path: None,
            mode: AutonomyMode::Autonomous,
            policy_context: PolicyContext::standard(),
            target_branch: None,
            target_commit: None,
            model_profile: None,
        }
    }

    #[test]
    fn fork_without_overrides_copies_parent_specification() {
        let constraints = vec!["parent constraint".to_owned()];
        let requirements = vec!["parent requirement".to_owned()];
        let success_criteria = vec!["parent criterion".to_owned()];
        let policy_context = PolicyContext {
            profile_name: "parent-profile".to_owned(),
            security_level: 2,
            allowed_capabilities: vec!["workspace_fs_read".to_owned()],
        };
        let inputs = resolve_fork_inputs(
            parent(&constraints, &requirements, &success_criteria, &policy_context),
            None,
        );

        assert_eq!(inputs.objective, "parent objective");
        assert_eq!(inputs.constraints, ["parent constraint"]);
        assert_eq!(inputs.requirements, ["parent requirement"]);
        assert_eq!(inputs.success_criteria, ["parent criterion"]);
        assert_eq!(inputs.workspace_root, PathBuf::from("/workspace/parent"));
        assert_eq!(inputs.mode, AutonomyMode::Assisted);
        assert_eq!(inputs.policy_context.profile_name, "parent-profile");
    }

    #[test]
    fn empty_override_lists_inherit_while_nonempty_lists_replace() {
        let constraints = vec!["parent constraint".to_owned()];
        let requirements = vec!["parent requirement".to_owned()];
        let success_criteria = vec!["parent criterion".to_owned()];
        let policy_context = PolicyContext {
            profile_name: "parent-profile".to_owned(),
            security_level: 2,
            allowed_capabilities: vec!["workspace_fs_read".to_owned()],
        };
        let inputs = resolve_fork_inputs(
            parent(&constraints, &requirements, &success_criteria, &policy_context),
            Some(normalized(
                "child objective",
                Vec::new(),
                Vec::new(),
                vec!["child criterion".to_owned()],
            )),
        );

        assert_eq!(inputs.objective, "child objective");
        assert_eq!(inputs.constraints, ["parent constraint"]);
        assert_eq!(inputs.requirements, ["parent requirement"]);
        assert_eq!(inputs.success_criteria, ["child criterion"]);
        assert_eq!(inputs.workspace_root, PathBuf::from("/workspace/child"));
        assert_eq!(inputs.mode, AutonomyMode::Autonomous);
        assert_eq!(inputs.policy_context.profile_name, "standard");
    }
}
