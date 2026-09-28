//! Upstream planning, specifications, ADRs, risk register, and phased roadmap.
//!
//! # Architecture Invariant
//!
//! ```text
//! Project Charter (PROJECT.md) + Research Summary (SUMMARY.md)
//!        ↓
//! Requirements Synthesizer (REQUIREMENTS.md)
//!        ↓
//! System Architect (ARCHITECTURE.md + DECISIONS.md)
//!        ↓
//! Risk Register (RISKS.md)
//!        ↓
//! Roadmap Architect (ROADMAP.md)
//!        ↓
//! Planning State (STATE.md)
//!        ↓
//! Human Approval Gate
//!        ↓
//! Implementation Handoff (ImplementationReady)
//! ```
//!
//! "The model proposes. The runtime decides."

pub mod adr;
pub mod architecture;
pub mod architecture_synthesizer;
pub mod decision;
pub mod invalidation;
pub mod requirements;
pub mod requirements_synthesizer;
pub mod risks;
pub mod roadmap;
pub mod state;
pub mod traceability;
pub mod validation;

// Re-export primary types for callers
pub use adr::{AdrId, AdrRegistry, ArchitectureDecisionRecord};
pub use architecture::{
    ArchitectureComponent, ArchitectureDataStore, ArchitectureDocument, ArchitectureInterface,
    ArchitectureSubsystem, ComponentStatus, DeploymentBoundary, TrustBoundary,
};
pub use architecture_synthesizer::ArchitectureSynthesizer;
pub use decision::DecisionStatus;
pub use invalidation::{PlanningChangeSet, PlanningInvalidator};
pub use requirements::{
    DomainEngineeringRequirement, DomainEpistemicRevision, DomainEpistemicStatus, DomainProvenance,
    DomainProvenanceSourceType, DomainRequirementCategory, DomainRequirementKey,
    DomainRequirementPriority, DomainTrustLevel, RequirementsDocument,
};
pub use requirements_synthesizer::RequirementsSynthesizer;
pub use risks::{
    Criticality, QualitativeImpact, QualitativeLikelihood, RiskEntry, RiskRegister, RiskStatus,
};
pub use roadmap::{
    Roadmap, RoadmapDependency, RoadmapPhase, RoadmapPhaseStatus, RoadmapSynthesizer,
};
pub use state::{PlanningLifecycleState, PlanningState, PlanningStateMachine};
pub use traceability::{TraceLink, TraceabilityMatrix};
pub use validation::PlanningQualityGates;

use crate::workflow::error::WorkflowError;
use crate::workflow::genesis::brownfield::BrownfieldMap;
use crate::workflow::genesis::environment::WorkspaceEnvironment;
use crate::workflow::genesis::intake::GenesisOptions;
use crate::workflow::genesis::project::ProjectCharter;
use crate::workflow::genesis::provenance::ResearchFinding;
use crate::workflow::genesis::synthesis::ResearchSummary;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Outcome of the comprehensive planning synthesis pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningPipelineOutcome {
    pub requirements: RequirementsDocument,
    pub requirements_path: PathBuf,
    pub architecture: ArchitectureDocument,
    pub architecture_path: PathBuf,
    pub adrs: AdrRegistry,
    pub decisions_path: PathBuf,
    pub risks: RiskRegister,
    pub risks_path: PathBuf,
    pub roadmap: Roadmap,
    pub roadmap_path: PathBuf,
    pub traceability: TraceabilityMatrix,
    pub state: PlanningState,
    pub state_path: PathBuf,
}

impl PlanningPipelineOutcome {
    /// Register all durable planning artifacts in the canonical `ArtifactService`, linking provenance.
    pub async fn register_artifacts(
        &self,
        artifact_service: &crate::persistence::artifacts::ArtifactService,
        parent_id: Option<crate::ids::ArtifactId>,
    ) -> Result<
        Vec<crate::persistence::artifacts::ArtifactRecord>,
        crate::persistence::artifacts::ArtifactError,
    > {
        let mut records = Vec::new();

        // 1. Requirements
        let req_rec = artifact_service
            .register_genesis_artifact(
                "REQUIREMENTS.md",
                &self.requirements_path,
                "genesis.requirements",
                parent_id,
            )
            .await?;
        let req_id = req_rec.id;
        records.push(req_rec);

        // 2. Architecture
        let arch_rec = artifact_service
            .register_genesis_artifact(
                "ARCHITECTURE.md",
                &self.architecture_path,
                "genesis.architecture",
                Some(req_id),
            )
            .await?;
        let arch_id = arch_rec.id;
        records.push(arch_rec);

        // 3. ADRs (Decisions)
        let adr_rec = artifact_service
            .register_genesis_artifact(
                "DECISIONS.md",
                &self.decisions_path,
                "genesis.adr",
                Some(arch_id),
            )
            .await?;
        records.push(adr_rec);

        // 4. Risks
        let risk_rec = artifact_service
            .register_genesis_artifact("RISKS.md", &self.risks_path, "genesis.risks", Some(req_id))
            .await?;
        records.push(risk_rec);

        // 5. Roadmap
        let road_rec = artifact_service
            .register_genesis_artifact(
                "ROADMAP.md",
                &self.roadmap_path,
                "genesis.roadmap",
                Some(arch_id),
            )
            .await?;
        let road_id = road_rec.id;
        records.push(road_rec);

        // 6. State
        let state_rec = artifact_service
            .register_genesis_artifact("STATE.md", &self.state_path, "genesis.state", Some(road_id))
            .await?;
        records.push(state_rec);

        Ok(records)
    }
}

/// Orchestrates the upstream requirements, architecture, ADR, risk, and roadmap synthesis pipeline.
pub struct PlanningCoordinator;

impl PlanningCoordinator {
    /// Execute the complete planning pipeline from Genesis inputs to verified planning artifacts on disk.
    pub fn run_planning(
        charter: &ProjectCharter,
        summary: Option<&ResearchSummary>,
        findings: &[ResearchFinding],
        brownfield: Option<&BrownfieldMap>,
        env: Option<&WorkspaceEnvironment>,
        options: &GenesisOptions,
        workspace_root: &Path,
    ) -> Result<PlanningPipelineOutcome, WorkflowError> {
        let proj_dir = &options.projection_dir;

        // 1. Requirements Synthesis
        let reqs =
            RequirementsSynthesizer::synthesize(charter, summary, findings, brownfield, env)?;
        let t1_req = PlanningQualityGates::evaluate_requirements_tier1(&reqs);
        if !t1_req.is_passed {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: Some("requirements_tier1".to_string()),
                field: None,
                reason: format!("Requirements Tier 1 gate failed: {:?}", t1_req.violations),
            });
        }
        let t2_req = PlanningQualityGates::evaluate_requirements_tier2(&reqs);
        if !t2_req.is_passed {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: Some("requirements_tier2".to_string()),
                field: None,
                reason: format!("Requirements Tier 2 gate failed: {:?}", t2_req.violations),
            });
        }
        let requirements_path = reqs.save_to_dir(workspace_root, proj_dir)?;

        // 2. Architecture & Candidate ADR Synthesis
        let (arch, adrs) =
            ArchitectureSynthesizer::synthesize(&reqs, charter, summary, findings, brownfield)?;
        let t1_arch = PlanningQualityGates::evaluate_architecture_tier1(&arch);
        if !t1_arch.is_passed {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: Some("architecture_tier1".to_string()),
                field: None,
                reason: format!("Architecture Tier 1 gate failed: {:?}", t1_arch.violations),
            });
        }
        let t2_arch = PlanningQualityGates::evaluate_architecture_tier2(&arch, &reqs, &adrs);
        if !t2_arch.is_passed {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: Some("architecture_tier2".to_string()),
                field: None,
                reason: format!("Architecture Tier 2 gate failed: {:?}", t2_arch.violations),
            });
        }
        let architecture_path = arch.save_to_dir(workspace_root, proj_dir)?;
        let decisions_path = adrs.save_to_dir(workspace_root, proj_dir)?;

        // 3. Risk Register Synthesis
        let mut risks = RiskRegister::new();
        let prov = crate::planning::requirements::Provenance::new(
            crate::planning::requirements::ProvenanceSourceType::RepositoryFile,
            crate::planning::requirements::TrustLevel::VerifiedRepository,
            "risk_synthesizer",
        );

        if let Some(sum) = summary {
            for (idx, unk) in sum.open_unknowns.iter().enumerate() {
                let rsk_id = format!("RSK-{:02}", idx + 1);
                let entry = RiskEntry::new(
                    &rsk_id,
                    format!("Risk in {}: {}", unk.area, unk.mitigation_strategy),
                    "research/SUMMARY.md",
                    QualitativeLikelihood::Medium,
                    QualitativeImpact::Medium,
                    unk.mitigation_strategy.clone(),
                    "Targeted verification suite in roadmap phase",
                    prov.clone(),
                );
                risks.add_risk(entry);
            }
        }
        risks.sort_deterministic();
        let risks_path = risks.save_to_dir(workspace_root, proj_dir)?;

        // 4. Roadmap Synthesis
        let roadmap =
            RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, charter, brownfield)?;
        let t1_road = PlanningQualityGates::evaluate_roadmap_tier1(&roadmap);
        if !t1_road.is_passed {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: Some("roadmap_tier1".to_string()),
                field: None,
                reason: format!("Roadmap Tier 1 gate failed: {:?}", t1_road.violations),
            });
        }
        let t2_road = PlanningQualityGates::evaluate_roadmap_tier2(&roadmap, &reqs, &arch);
        if !t2_road.is_passed {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: Some("roadmap_tier2".to_string()),
                field: None,
                reason: format!("Roadmap Tier 2 gate failed: {:?}", t2_road.violations),
            });
        }
        let roadmap_path = roadmap.save_to_dir(workspace_root, proj_dir)?;

        // 5. Traceability Matrix Compilation
        let traceability = TraceabilityMatrix::build(&reqs, &arch, &adrs, &roadmap, &risks);
        if let Err(errs) = traceability.validate() {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: Some("traceability_validation".to_string()),
                field: None,
                reason: format!("Traceability matrix validation failed: {:?}", errs),
            });
        }

        // 6. Planning State & State Machine Lifecycle
        let mut sm = PlanningStateMachine::new();
        sm.transition_to(
            PlanningLifecycleState::RequirementsApproved,
            "Tier 1 & Tier 2 quality gates passed",
        )?;
        sm.transition_to(
            PlanningLifecycleState::ArchitectureDraft,
            "Beginning architecture synthesis",
        )?;
        sm.transition_to(
            PlanningLifecycleState::ArchitectureApproved,
            "Architecture validation passed",
        )?;
        sm.transition_to(
            PlanningLifecycleState::RoadmapDraft,
            "Beginning roadmap synthesis",
        )?;
        sm.transition_to(
            PlanningLifecycleState::RoadmapApproved,
            "Roadmap DAG validation passed",
        )?;
        sm.transition_to(
            PlanningLifecycleState::ImplementationReady,
            "All planning layers synthesized and validated",
        )?;

        let mut state = PlanningState::new(
            &charter.project_name,
            PlanningLifecycleState::ImplementationReady,
        );
        state.requirements_status = "Approved".to_string();
        state.requirements_count = reqs.requirements.len();
        state.architecture_status = "Approved".to_string();
        state.component_count = arch.components.len();
        state.adr_status = "Proposals Registered".to_string();
        state.adr_count = adrs.adrs.len();
        state.roadmap_status = "Approved DAG".to_string();
        state.phase_count = roadmap.phases.len();
        state.approval_status =
            "Synthesized and Verified (Ready for Implementation Handoff)".to_string();
        state.current_phase = roadmap.phases.first().map(|p| p.id.clone());
        state.next_authorized_transition = "Handoff to Phase Plan Decomposition".to_string();

        let state_path = state.save_to_dir(workspace_root, proj_dir)?;

        Ok(PlanningPipelineOutcome {
            requirements: reqs,
            requirements_path,
            architecture: arch,
            architecture_path,
            adrs,
            decisions_path,
            risks,
            risks_path,
            roadmap,
            roadmap_path,
            traceability,
            state,
            state_path,
        })
    }
}
