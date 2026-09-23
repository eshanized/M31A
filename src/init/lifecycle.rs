//! Durable Initialization Lifecycle State Machine (FRX-01, FRX-02, D-01).
//!
//! Tracks cold startup through UNINITIALIZED, CHECKING, CONFIGURING, VERIFYING,
//! READY, and ONBOARDED states with bootstrap sentinel and SQLite backing.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Linear 7-step onboarding wizard progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SetupStep {
    /// Step 1: Explicit operator confirmation of repository trust and safety boundaries.
    WorkspaceTrust,
    /// Step 2: System diagnostic checks (git, disk space, SQLite, sandbox).
    DoctorDiagnostics,
    /// Step 3: LLM provider selection and credential entry.
    ProviderSetup,
    /// Step 4: Model selection and reasoning budget configuration.
    ModelSetup,
    /// Step 5: Agent execution profile selection (Autonomous, Balanced, etc.).
    ProfileSelection,
    /// Step 6: Autonomy limits and human approval policies.
    AutonomySafety,
    /// Step 7: Final verification probe and confirmation.
    FinalVerification,
}

impl SetupStep {
    /// Return the 1-based index of this step (1 through 7).
    pub fn step_number(&self) -> usize {
        match self {
            Self::WorkspaceTrust => 1,
            Self::DoctorDiagnostics => 2,
            Self::ProviderSetup => 3,
            Self::ModelSetup => 4,
            Self::ProfileSelection => 5,
            Self::AutonomySafety => 6,
            Self::FinalVerification => 7,
        }
    }

    /// Total number of setup wizard steps.
    pub fn total_steps() -> usize {
        7
    }

    /// Advance to next sequential step.
    pub fn next(&self) -> Option<Self> {
        match self {
            Self::WorkspaceTrust => Some(Self::DoctorDiagnostics),
            Self::DoctorDiagnostics => Some(Self::ProviderSetup),
            Self::ProviderSetup => Some(Self::ModelSetup),
            Self::ModelSetup => Some(Self::ProfileSelection),
            Self::ProfileSelection => Some(Self::AutonomySafety),
            Self::AutonomySafety => Some(Self::FinalVerification),
            Self::FinalVerification => None,
        }
    }

    /// Return previous sequential step.
    pub fn prev(&self) -> Option<Self> {
        match self {
            Self::WorkspaceTrust => None,
            Self::DoctorDiagnostics => Some(Self::WorkspaceTrust),
            Self::ProviderSetup => Some(Self::DoctorDiagnostics),
            Self::ModelSetup => Some(Self::ProviderSetup),
            Self::ProfileSelection => Some(Self::ModelSetup),
            Self::AutonomySafety => Some(Self::ProfileSelection),
            Self::FinalVerification => Some(Self::AutonomySafety),
        }
    }

    /// Human-readable title of this setup step.
    pub fn title(&self) -> &'static str {
        match self {
            Self::WorkspaceTrust => "Workspace Trust",
            Self::DoctorDiagnostics => "Doctor Diagnostics",
            Self::ProviderSetup => "Provider Setup",
            Self::ModelSetup => "Model Setup",
            Self::ProfileSelection => "Profile Selection",
            Self::AutonomySafety => "Autonomy & Safety",
            Self::FinalVerification => "Final Verification",
        }
    }
}

/// Durable initialization lifecycle state (D-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitState {
    /// Brand new installation, no sentinel or state initialized.
    Uninitialized,
    /// Inspecting environment and workspace preconditions.
    Checking,
    /// In the middle of guided onboarding wizard steps (Step 1 to 7).
    Configuring(SetupStep),
    /// Validating saved configuration via test connections and doctor probes.
    Verifying,
    /// Initialization verified and storage schema intact.
    Ready,
    /// Operator has completed onboarding and acknowledged workspace setup.
    Onboarded,
}

impl InitState {
    /// Return human-readable label for status bars and logging.
    pub fn label(&self) -> String {
        match self {
            Self::Uninitialized => "UNINITIALIZED".to_string(),
            Self::Checking => "CHECKING".to_string(),
            Self::Configuring(step) => {
                format!(
                    "CONFIGURING ({}/{}: {})",
                    step.step_number(),
                    SetupStep::total_steps(),
                    step.title()
                )
            }
            Self::Verifying => "VERIFYING".to_string(),
            Self::Ready => "READY".to_string(),
            Self::Onboarded => "ONBOARDED".to_string(),
        }
    }
}

/// Initialization lifecycle errors.
#[derive(Debug, Error)]
pub enum InitError {
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: InitState, to: InitState },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Missing prerequisite data: {0}")]
    MissingPrerequisite(String),
}

/// Pre-SQLite bootstrap sentinel payload persisted in `.m31a/init.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitSentinel {
    pub state: InitState,
    pub step_data: HashMap<String, serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for InitSentinel {
    fn default() -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            state: InitState::Uninitialized,
            step_data: HashMap::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// Durable initialization manager with sentinel file and database transition.
#[derive(Debug)]
pub struct InitManager {
    workspace_root: PathBuf,
    sentinel_path: PathBuf,
    sentinel: InitSentinel,
}

impl InitManager {
    pub const SENTINEL_DIR: &'static str = ".m31a";
    pub const SENTINEL_FILENAME: &'static str = "init.json";

    /// Initialize manager for a workspace, loading existing sentinel file if present.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Result<Self, InitError> {
        let workspace_root = workspace_root.into();
        let sentinel_path = workspace_root
            .join(Self::SENTINEL_DIR)
            .join(Self::SENTINEL_FILENAME);

        let sentinel = if sentinel_path.exists() {
            let data = fs::read_to_string(&sentinel_path)?;
            serde_json::from_str(&data)?
        } else {
            InitSentinel::default()
        };

        Ok(Self {
            workspace_root,
            sentinel_path,
            sentinel,
        })
    }

    /// Check if sentinel file exists on disk.
    pub fn is_sentinel_present(&self) -> bool {
        self.sentinel_path.exists()
    }

    /// Get current lifecycle state.
    pub fn current_state(&self) -> &InitState {
        &self.sentinel.state
    }

    /// Check if the system has completed onboarding and is fully operational.
    pub fn is_onboarded(&self) -> bool {
        self.sentinel.state == InitState::Onboarded
    }

    /// Save state and step data to the sentinel file on disk.
    pub fn persist_sentinel(&mut self) -> Result<(), InitError> {
        if let Some(parent) = self.sentinel_path.parent() {
            fs::create_dir_all(parent)?;
        }
        self.sentinel.updated_at = Utc::now().to_rfc3339();
        let json = serde_json::to_string_pretty(&self.sentinel)?;
        fs::write(&self.sentinel_path, json)?;
        Ok(())
    }

    /// Attempt state transition following strict lifecycle constraints (D-01).
    pub fn transition_to(&mut self, next: InitState) -> Result<(), InitError> {
        let valid = match (&self.sentinel.state, &next) {
            // Uninitialized can only begin checking or directly configure step 1
            (InitState::Uninitialized, InitState::Checking) => true,
            (InitState::Uninitialized, InitState::Configuring(SetupStep::WorkspaceTrust)) => true,

            // Checking can move to step 1 or back to uninitialized
            (InitState::Checking, InitState::Configuring(SetupStep::WorkspaceTrust)) => true,
            (InitState::Checking, InitState::Uninitialized) => true,

            // Configuring can navigate between steps, cancel to Checking, or advance to Verifying if at FinalVerification
            (InitState::Configuring(_), InitState::Configuring(_)) => true,
            (InitState::Configuring(SetupStep::FinalVerification), InitState::Verifying) => true,
            (InitState::Configuring(_), InitState::Checking) => true,

            // Verifying can succeed to Ready or fall back to Configuring if verification fails
            (InitState::Verifying, InitState::Ready) => true,
            (InitState::Verifying, InitState::Configuring(_)) => true,

            // Ready can advance to Onboarded
            (InitState::Ready, InitState::Onboarded) => true,

            // Onboarded can re-enter Configuring if explicitly re-configuring
            (InitState::Onboarded, InitState::Configuring(SetupStep::WorkspaceTrust)) => true,

            // Same state transition is a no-op / valid
            (a, b) if a == b => true,

            _ => false,
        };

        if !valid {
            return Err(InitError::InvalidTransition {
                from: self.sentinel.state.clone(),
                to: next,
            });
        }

        self.sentinel.state = next;
        self.persist_sentinel()?;
        Ok(())
    }

    /// Save step-specific configuration data into durable sentinel storage.
    pub fn save_step_data(
        &mut self,
        step: SetupStep,
        data: serde_json::Value,
    ) -> Result<(), InitError> {
        let key = format!("{:?}", step);
        self.sentinel.step_data.insert(key, data);
        self.persist_sentinel()?;
        Ok(())
    }

    /// Retrieve step-specific configuration data.
    pub fn get_step_data(&self, step: SetupStep) -> Option<&serde_json::Value> {
        let key = format!("{:?}", step);
        self.sentinel.step_data.get(&key)
    }

    /// Retrieve workspace root path.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Complete onboarding and migrate state records to SQLite `system_state` table.
    pub async fn complete_and_migrate_to_db(
        &mut self,
        pool: &crate::persistence::sqlite::SqlitePool,
    ) -> Result<(), InitError> {
        // Ensure state is Ready before finishing
        if self.sentinel.state != InitState::Ready && self.sentinel.state != InitState::Onboarded {
            self.transition_to(InitState::Ready)?;
        }
        self.transition_to(InitState::Onboarded)?;

        // Persist init_state and onboarding metadata into SQLite via canonical repository
        let repo = crate::persistence::sqlite::repositories::SqliteSystemStateRepository::new(
            pool.clone(),
        );
        let payload = serde_json::to_string(&self.sentinel)?;
        repo.record_onboarding(&payload, "Onboarded")
            .await
            .map_err(|e| InitError::Database(e.to_string()))?;

        Ok(())
    }
}
