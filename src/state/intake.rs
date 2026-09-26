//! MissionIntake contract and JSON Schema generator (D-05, D-06, D-08, D-09).

use crate::state::budget::ResourceBudget;
use crate::state::policy_context::PolicyContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

pub use crate::state_machine::AutonomyMode;

/// External mission intake contract.
///
/// Per D-09, rejects unknown fields with serde deny_unknown_fields.
/// Per D-06, only non-empty objective is required; all other fields are optional.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MissionIntake {
    /// Non-empty natural language objective.
    pub objective: String,
    /// Optional path to requirements markdown or text file.
    pub requirements_file: Option<PathBuf>,
    /// Optional explicit constraints.
    pub constraints: Option<Vec<String>>,
    /// Optional success criteria.
    pub success_criteria: Option<Vec<String>>,
    /// Optional named policy profile.
    pub policy_profile: Option<String>,
    /// Optional execution resource budget.
    pub budget: Option<ResourceBudget>,
    /// Optional target git branch.
    pub target_branch: Option<String>,
    /// Optional target git commit SHA.
    pub target_commit: Option<String>,
    /// Optional repository path.
    pub repository_path: Option<PathBuf>,
    /// Optional workspace root.
    pub workspace_root: Option<PathBuf>,
    /// Optional model profile name.
    pub model_profile: Option<String>,
    /// Optional autonomy mode.
    pub mode: Option<AutonomyMode>,
}

/// Validation error during intake checking.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntakeValidationError {
    #[error("Mission objective cannot be empty")]
    EmptyObjective,
    #[error("Workspace path contains forbidden path traversal: {0}")]
    PathTraversal(String),
}

/// Fully normalized intake ready for domain Mission construction.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedIntake {
    pub objective: String,
    pub constraints: Vec<String>,
    pub requirements: Vec<String>,
    pub success_criteria: Vec<String>,
    pub workspace_root: PathBuf,
    pub repository_path: Option<PathBuf>,
    pub mode: AutonomyMode,
    pub policy_context: PolicyContext,
    pub budget: ResourceBudget,
    pub target_branch: Option<String>,
    pub target_commit: Option<String>,
    pub model_profile: Option<String>,
}

impl MissionIntake {
    /// Create a minimal intake with only an objective.
    pub fn new(objective: impl Into<String>) -> Self {
        Self {
            objective: objective.into(),
            requirements_file: None,
            constraints: None,
            success_criteria: None,
            policy_profile: None,
            budget: None,
            target_branch: None,
            target_commit: None,
            repository_path: None,
            workspace_root: None,
            model_profile: None,
            mode: None,
        }
    }

    /// Validate intake fields against security and semantic constraints.
    pub fn validate(&self) -> Result<(), IntakeValidationError> {
        if self.objective.trim().is_empty() {
            return Err(IntakeValidationError::EmptyObjective);
        }

        if let Some(ref path) = self.workspace_root {
            Self::validate_path_safety(path)?;
        }

        if let Some(ref path) = self.repository_path {
            Self::validate_path_safety(path)?;
        }

        if let Some(ref path) = self.requirements_file {
            Self::validate_path_safety(path)?;
        }

        Ok(())
    }

    fn validate_path_safety(path: &Path) -> Result<(), IntakeValidationError> {
        for comp in path.components() {
            if comp == Component::ParentDir {
                return Err(IntakeValidationError::PathTraversal(
                    path.display().to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Normalize the intake with deterministic defaults against a base workspace.
    pub fn normalize(
        &self,
        default_workspace: &Path,
    ) -> Result<NormalizedIntake, IntakeValidationError> {
        self.validate()?;

        let workspace_root = match &self.workspace_root {
            Some(w) => {
                if w.is_absolute() {
                    w.clone()
                } else {
                    default_workspace.join(w)
                }
            }
            None => default_workspace.to_path_buf(),
        };

        let mode = self.mode.unwrap_or(AutonomyMode::Safe);

        let policy_context = match &self.policy_profile {
            Some(profile) => PolicyContext {
                profile_name: profile.clone(),
                security_level: 1,
                allowed_capabilities: vec!["fs:read".to_string(), "git".to_string()],
            },
            None => PolicyContext::standard(),
        };

        let budget = self.budget.clone().unwrap_or_default();

        Ok(NormalizedIntake {
            objective: self.objective.trim().to_string(),
            constraints: self.constraints.clone().unwrap_or_default(),
            requirements: Vec::new(),
            success_criteria: self.success_criteria.clone().unwrap_or_default(),
            workspace_root,
            repository_path: self.repository_path.clone(),
            mode,
            policy_context,
            budget,
            target_branch: self.target_branch.clone(),
            target_commit: self.target_commit.clone(),
            model_profile: self.model_profile.clone(),
        })
    }
}

/// Generates a canonical JSON Schema for MissionIntake using schemars (D-05).
pub fn generate_mission_intake_schema() -> serde_json::Value {
    let schema = schemars::schema_for!(MissionIntake);
    serde_json::to_value(&schema).expect("Failed to serialize JSON schema to Value")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intake_minimal_valid() {
        let json = r#"{"objective": "Fix memory leak in parser"}"#;
        let intake: MissionIntake = serde_json::from_str(json).unwrap();
        assert_eq!(intake.objective, "Fix memory leak in parser");
        assert!(intake.validate().is_ok());
    }

    #[test]
    fn test_intake_rejects_unknown_fields() {
        let json = r#"{"objective": "Test", "unrecognized_field": 123}"#;
        let result: Result<MissionIntake, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_intake_rejects_empty_objective() {
        let intake = MissionIntake::new("   ");
        assert_eq!(
            intake.validate().unwrap_err(),
            IntakeValidationError::EmptyObjective
        );
    }

    #[test]
    fn test_intake_rejects_path_traversal() {
        let mut intake = MissionIntake::new("Valid objective");
        intake.workspace_root = Some(PathBuf::from("../../etc/passwd"));
        assert!(matches!(
            intake.validate(),
            Err(IntakeValidationError::PathTraversal(_))
        ));
    }

    #[test]
    fn test_intake_normalize() {
        let intake = MissionIntake::new("Build feature");
        let default_ws = Path::new("/tmp/workspace");
        let normalized = intake.normalize(default_ws).unwrap();
        assert_eq!(normalized.objective, "Build feature");
        assert_eq!(normalized.workspace_root, PathBuf::from("/tmp/workspace"));
        assert_eq!(normalized.mode, AutonomyMode::Safe);
    }

    #[test]
    fn test_generate_mission_intake_schema() {
        let schema = generate_mission_intake_schema();
        assert!(schema.is_object());
        let schema_str = serde_json::to_string(&schema).unwrap();
        assert!(schema_str.contains("objective"));
    }
}
