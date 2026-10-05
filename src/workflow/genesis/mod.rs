//! Project Genesis: Discovery, Environment Probing, and Research Orchestration.
//!
//! # Architecture Invariant
//!
//! ```text
//! Genesis Controller
//!       ↓
//! WorkflowEngine
//!       ↓
//! WorkflowStep
//!       ↓
//! Mission
//!       ↓
//! TaskGraph
//!       ↓
//! SchedulerEngine
//!       ↓
//! AgentRuntime
//! ```
//!
//! "The model proposes. The runtime decides."

pub mod brownfield;
pub mod dimension_registry;
pub mod discovery;
pub mod environment;
pub mod errors;
pub mod intake;
pub mod project;
pub mod provenance;
pub mod research_decision;
pub mod researcher;
pub mod synthesis;

// Re-export primary domain types
pub use brownfield::{BrownfieldMap, CodebaseTopology};
pub use dimension_registry::{
    DimensionError, ResearchDimensionDefinition, ResearchDimensionRegistry,
};
pub use discovery::{
    AnswerValidationError, ConvergenceReason, DiscoveryFact, DiscoveryPillar, DiscoverySession,
    DiscoveryTurn, DynamicQuestion, QuestionValidationError, apply_question_answer,
    is_discovery_converged, validate_dynamic_question, validate_question_answer,
};
pub use environment::{
    Confidence, EnvironmentCategory, EnvironmentFact, FactSource, WorkspaceEnvironment,
    WorkspaceMode,
};
pub use errors::GenesisError;
pub use intake::{GenesisMode, GenesisOptions, GenesisRequest};
pub use project::{
    AmbiguityAssessment, OperationalInvariants, ProjectBoundaries, ProjectCharter,
    TargetDomainModel, TechnicalPreferences, WorkflowTier,
};
pub use provenance::{ResearchEvidence, ResearchFinding, ResearchSourceType};
pub use research_decision::{ResearchDecision, ResearchDimension, evaluate_research_decision};
pub use researcher::ResearchOrchestrator;
pub use synthesis::{
    ConsensusPoint, ContradictionResolution, OpenUnknown, RejectedAlternative, ResearchSummary,
    ResearchSynthesizer, TradeoffAnalysis,
};

use crate::workflow::engine::WorkflowEngine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Outcome of running the Genesis intake, discovery, and research pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenesisPipelineOutcome {
    pub environment: WorkspaceEnvironment,
    pub charter: ProjectCharter,
    pub charter_path: PathBuf,
    pub research_decision: ResearchDecision,
    pub summary: Option<ResearchSummary>,
    pub summary_path: Option<PathBuf>,
}

impl GenesisPipelineOutcome {
    /// Register all durable Genesis discovery and research artifacts in the canonical `ArtifactService`.
    pub async fn register_artifacts(
        &self,
        artifact_service: &crate::persistence::artifacts::ArtifactService,
    ) -> Result<
        Vec<crate::persistence::artifacts::ArtifactRecord>,
        crate::persistence::artifacts::ArtifactError,
    > {
        let mut records = Vec::new();

        // 1. Register charter
        let charter_rec = artifact_service
            .register_genesis_artifact("PROJECT.md", &self.charter_path, "genesis.discovery", None)
            .await?;
        let charter_id = charter_rec.id;
        records.push(charter_rec);

        // 2. Register summary if present
        if let Some(ref sum_path) = self.summary_path
            && sum_path.exists()
        {
            let sum_rec = artifact_service
                .register_genesis_artifact(
                    "SUMMARY.md",
                    sum_path,
                    "genesis.research_synthesis",
                    Some(charter_id),
                )
                .await?;
            records.push(sum_rec);
        }

        Ok(records)
    }
}

/// Orchestrates upstream Project Genesis from raw intake through discovery and research.
pub struct GenesisController;

impl GenesisController {
    /// Probe the workspace environment deterministically.
    pub fn probe(workspace_root: &Path) -> Result<WorkspaceEnvironment, GenesisError> {
        WorkspaceEnvironment::probe(workspace_root)
    }

    /// Execute deterministic brownfield codebase mapping.
    pub fn map_brownfield(
        workspace_root: &Path,
        projection_dir: &str,
    ) -> Result<BrownfieldMap, GenesisError> {
        let bmap = BrownfieldMap::scan(workspace_root)?;
        bmap.save_to_dir(workspace_root, projection_dir)?;
        Ok(bmap)
    }

    /// Run discovery session through completion using script or operator inputs, and save PROJECT.md.
    pub fn run_discovery(
        session: &mut DiscoverySession,
        workspace_root: &Path,
        projection_dir: &str,
    ) -> Result<ProjectCharter, GenesisError> {
        let charter = session.synthesize_charter()?;
        charter.validate()?;
        charter.save_to_dir(workspace_root, projection_dir)?;
        Ok(charter)
    }

    /// Evaluate research decision against charter and environment.
    pub fn decide_research(
        charter: &ProjectCharter,
        env: &WorkspaceEnvironment,
        options: &GenesisOptions,
    ) -> ResearchDecision {
        evaluate_research_decision(charter, env, options)
    }

    /// Execute bounded parallel research via WorkflowEngine and materialize SUMMARY.md.
    pub async fn execute_research(
        engine: &WorkflowEngine,
        charter: &ProjectCharter,
        decision: &ResearchDecision,
        options: &GenesisOptions,
        workspace_root: &Path,
    ) -> Result<(Option<ResearchSummary>, Vec<ResearchFinding>), GenesisError> {
        if !decision.execute_research {
            return Ok((None, Vec::new()));
        }

        let (summary, findings) =
            ResearchOrchestrator::execute(engine, charter, decision, options, workspace_root)
                .await?;
        if let Some(ref s) = summary {
            s.validate()?;
            s.save_to_dir(workspace_root, &options.projection_dir)?;
        }
        Ok((summary, findings))
    }

    /// Execute comprehensive requirements, architecture, ADR, risk, and roadmap synthesis.
    pub fn run_planning(
        charter: &ProjectCharter,
        summary: Option<&ResearchSummary>,
        findings: &[ResearchFinding],
        brownfield: Option<&BrownfieldMap>,
        env: Option<&WorkspaceEnvironment>,
        options: &GenesisOptions,
        workspace_root: &Path,
    ) -> Result<crate::workflow::planning::PlanningPipelineOutcome, GenesisError> {
        crate::workflow::planning::PlanningCoordinator::run_planning(
            charter,
            summary,
            findings,
            brownfield,
            env,
            options,
            workspace_root,
        )
        .map_err(GenesisError::Workflow)
    }
}
