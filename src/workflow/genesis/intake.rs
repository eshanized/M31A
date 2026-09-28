//! Genesis intake types, mode definition, and request aggregates.

use crate::config::ResolvedConfiguration;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Mode of Project Genesis initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenesisMode {
    /// Create a brand new project from scratch in an empty directory.
    Greenfield,
    /// Ingest and extend an existing codebase with existing code or Git history.
    Brownfield,
    /// Automatically probe the target workspace to determine Greenfield vs Brownfield.
    #[default]
    AutoDetect,
}

impl std::fmt::Display for GenesisMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Greenfield => write!(f, "greenfield"),
            Self::Brownfield => write!(f, "brownfield"),
            Self::AutoDetect => write!(f, "auto_detect"),
        }
    }
}

/// Runtime configuration options for Project Genesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenesisOptions {
    /// Maximum Socratic discovery interview turns (default 4).
    pub max_discovery_turns: usize,
    /// Ambiguity threshold percentage (1..=100) to conclude discovery (default 15).
    pub ambiguity_threshold_percent: u8,
    /// Whether domain research is enabled before planning (default true).
    pub enable_research: bool,
    /// Maximum concurrent parallel research dimensions (default 4, 1..=8).
    pub research_concurrency: usize,
    /// Require human approval before finalizing Project Charter (default true).
    pub require_charter_approval: bool,
    /// Projection directory relative to workspace root (default ".planning").
    pub projection_dir: String,
}

impl Default for GenesisOptions {
    fn default() -> Self {
        Self {
            max_discovery_turns: 4,
            ambiguity_threshold_percent: 15,
            enable_research: true,
            research_concurrency: 4,
            require_charter_approval: true,
            projection_dir: ".planning".to_string(),
        }
    }
}

impl GenesisOptions {
    /// Construct options from a resolved M31A configuration.
    pub fn from_config(config: &ResolvedConfiguration) -> Self {
        Self {
            max_discovery_turns: config.app_config.workflow.max_discovery_turns,
            ambiguity_threshold_percent: config.app_config.workflow.ambiguity_threshold_percent,
            enable_research: config.app_config.workflow.research,
            research_concurrency: config.app_config.workflow.research_concurrency,
            require_charter_approval: config.app_config.workflow.require_charter_approval,
            projection_dir: config.app_config.workflow.projection_dir.clone(),
        }
    }
}

/// Project Genesis initial intake request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenesisRequest {
    /// Unique identifier for this genesis request.
    pub request_id: String,
    /// Optional mission ID linking this genesis run to a runtime mission for
    /// assumption/decision provenance tracking.
    pub mission_id: Option<crate::ids::MissionId>,
    /// User's initial project concept, vision, or feature request.
    pub prompt: String,
    /// Genesis operational mode.
    pub mode: GenesisMode,
    /// Target filesystem workspace root directory.
    pub workspace_root: PathBuf,
    /// Genesis runtime options.
    pub options: GenesisOptions,
}

impl GenesisRequest {
    pub fn new(prompt: impl Into<String>, workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            request_id: uuid::Uuid::now_v7().to_string(),
            mission_id: None,
            prompt: prompt.into(),
            mode: GenesisMode::AutoDetect,
            workspace_root: workspace_root.into(),
            options: GenesisOptions::default(),
        }
    }

    pub fn with_mission_id(mut self, mission_id: crate::ids::MissionId) -> Self {
        self.mission_id = Some(mission_id);
        self
    }

    pub fn with_mode(mut self, mode: GenesisMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_options(mut self, options: GenesisOptions) -> Self {
        self.options = options;
        self
    }
}
