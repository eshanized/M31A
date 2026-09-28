//! Quality gates for Requirements, Architecture, and Roadmap synthesis.

use super::adr::AdrRegistry;
use super::architecture::ArchitectureDocument;
use super::requirements::RequirementsDocument;
use super::roadmap::Roadmap;
use crate::planning::requirements::{EpistemicStatus, RequirementPriority};
use crate::workflow::quality_gate::{QualityGateResult, QualityGateTier, QualityGateViolation};
use std::collections::HashSet;

/// Planning Quality Gate engine evaluating Tier 1 and Tier 2 gates across planning artifacts.
pub struct PlanningQualityGates;

impl PlanningQualityGates {
    /// Evaluate Requirements Tier 1 Gate (Section 13, 40).
    pub fn evaluate_requirements_tier1(reqs: &RequirementsDocument) -> QualityGateResult {
        let mut violations = Vec::new();

        if reqs.requirements.is_empty() {
            violations.push(QualityGateViolation::error(
                "REQ_T1_NON_EMPTY",
                "Requirements document contains zero requirements",
                Some("requirements".to_string()),
                Some("REQUIREMENTS.md".to_string()),
            ));
            return QualityGateResult::fail(QualityGateTier::Tier1SyntaxAndArtifacts, violations);
        }

        let mut seen_keys = HashSet::new();
        for r in &reqs.requirements {
            // Check ID non-empty
            if r.key.as_str().trim().is_empty() {
                violations.push(QualityGateViolation::error(
                    "REQ_T1_ID_NON_EMPTY",
                    "Requirement has empty ID key",
                    Some("requirements".to_string()),
                    Some("REQUIREMENTS.md".to_string()),
                ));
            }

            // Check duplicate IDs
            if !seen_keys.insert(r.key.clone()) {
                violations.push(QualityGateViolation::error(
                    "REQ_T1_UNIQUE_ID",
                    format!("Duplicate requirement ID '{}'", r.key),
                    Some("requirements".to_string()),
                    Some("REQUIREMENTS.md".to_string()),
                ));
            }

            // Check non-empty statement
            if r.statement().trim().is_empty() {
                violations.push(QualityGateViolation::error(
                    "REQ_T1_NON_EMPTY_STATEMENT",
                    format!("Requirement '{}' has empty description/statement", r.key),
                    Some("requirements".to_string()),
                    Some("REQUIREMENTS.md".to_string()),
                ));
            }

            // Check acceptance criteria for executable (non-unknown) requirements
            if r.current_status != EpistemicStatus::Unknown
                && r.priority != RequirementPriority::Deferred
                && r.satisfaction_criteria.is_empty()
            {
                violations.push(QualityGateViolation::error(
                    "REQ_T1_ACCEPTANCE_CRITERIA",
                    format!(
                        "Active requirement '{}' lacks acceptance/satisfaction criteria",
                        r.key
                    ),
                    Some("requirements".to_string()),
                    Some("REQUIREMENTS.md".to_string()),
                ));
            }
        }

        if violations.is_empty() {
            QualityGateResult::pass(QualityGateTier::Tier1SyntaxAndArtifacts)
        } else {
            QualityGateResult::fail(QualityGateTier::Tier1SyntaxAndArtifacts, violations)
        }
    }

    /// Evaluate Requirements Tier 2 Gate (Section 13, 40).
    pub fn evaluate_requirements_tier2(reqs: &RequirementsDocument) -> QualityGateResult {
        let mut violations = Vec::new();
        let all_keys: HashSet<_> = reqs.requirements.iter().map(|r| r.key.clone()).collect();

        for r in &reqs.requirements {
            // Source provenance exists
            if r.provenance.actor.trim().is_empty() {
                violations.push(QualityGateViolation::error(
                    "REQ_T2_PROVENANCE_ACTOR",
                    format!("Requirement '{}' has empty provenance actor", r.key),
                    Some("requirements".to_string()),
                    Some("REQUIREMENTS.md".to_string()),
                ));
            }

            // Validate declared dependencies reference real requirements
            for dep in &r.dependencies {
                if !all_keys.contains(dep) {
                    violations.push(QualityGateViolation::error(
                        "REQ_T2_DEPENDENCY_EXISTS",
                        format!(
                            "Requirement '{}' depends on non-existent requirement '{}'",
                            r.key, dep
                        ),
                        Some("requirements".to_string()),
                        Some("REQUIREMENTS.md".to_string()),
                    ));
                }
            }
        }

        if violations.is_empty() {
            QualityGateResult::pass(QualityGateTier::Tier2RelationalConsistency)
        } else {
            QualityGateResult::fail(QualityGateTier::Tier2RelationalConsistency, violations)
        }
    }

    /// Evaluate Architecture Tier 1 Gate (Section 40).
    pub fn evaluate_architecture_tier1(arch: &ArchitectureDocument) -> QualityGateResult {
        let mut violations = Vec::new();

        if arch.components.is_empty() {
            violations.push(QualityGateViolation::error(
                "ARCH_T1_NON_EMPTY",
                "Architecture document contains zero components",
                Some("architecture".to_string()),
                Some("ARCHITECTURE.md".to_string()),
            ));
            return QualityGateResult::fail(QualityGateTier::Tier1SyntaxAndArtifacts, violations);
        }

        let mut seen_ids = HashSet::new();
        for comp in &arch.components {
            if comp.id.trim().is_empty() {
                violations.push(QualityGateViolation::error(
                    "ARCH_T1_ID_NON_EMPTY",
                    "Architecture component has empty ID",
                    Some("architecture".to_string()),
                    Some("ARCHITECTURE.md".to_string()),
                ));
            }
            if !seen_ids.insert(comp.id.clone()) {
                violations.push(QualityGateViolation::error(
                    "ARCH_T1_UNIQUE_ID",
                    format!("Duplicate architecture component ID '{}'", comp.id),
                    Some("architecture".to_string()),
                    Some("ARCHITECTURE.md".to_string()),
                ));
            }

            // No self-dependency
            for dep in &comp.depends_on {
                if dep == &comp.id {
                    violations.push(QualityGateViolation::error(
                        "ARCH_T1_SELF_DEPENDENCY",
                        format!("Component '{}' cannot depend on itself", comp.id),
                        Some("architecture".to_string()),
                        Some("ARCHITECTURE.md".to_string()),
                    ));
                }
            }
        }

        // Interface provider validity
        for iface in &arch.interfaces {
            if !seen_ids.contains(&iface.provider_component) {
                violations.push(QualityGateViolation::error(
                    "ARCH_T1_IFACE_PROVIDER",
                    format!(
                        "Interface '{}' references non-existent provider component '{}'",
                        iface.id, iface.provider_component
                    ),
                    Some("architecture".to_string()),
                    Some("ARCHITECTURE.md".to_string()),
                ));
            }
        }

        if violations.is_empty() {
            QualityGateResult::pass(QualityGateTier::Tier1SyntaxAndArtifacts)
        } else {
            QualityGateResult::fail(QualityGateTier::Tier1SyntaxAndArtifacts, violations)
        }
    }

    /// Evaluate Architecture Tier 2 Gate (Section 40).
    pub fn evaluate_architecture_tier2(
        arch: &ArchitectureDocument,
        reqs: &RequirementsDocument,
        adrs: &AdrRegistry,
    ) -> QualityGateResult {
        let mut violations = Vec::new();
        let req_keys: HashSet<_> = reqs.requirements.iter().map(|r| r.key.clone()).collect();

        // Check requirement references in components
        for comp in &arch.components {
            for r in &comp.requirement_refs {
                if !req_keys.contains(r) {
                    violations.push(QualityGateViolation::error(
                        "ARCH_T2_REQ_REF_EXISTS",
                        format!(
                            "Component '{}' references non-existent requirement '{}'",
                            comp.id, r
                        ),
                        Some("architecture".to_string()),
                        Some("ARCHITECTURE.md".to_string()),
                    ));
                }
            }
        }

        // Check ADR references
        for adr_ref in &arch.adr_refs {
            let adr_id = crate::workflow::planning::adr::AdrId::new(adr_ref);
            if adrs.get(&adr_id).is_none() {
                violations.push(QualityGateViolation::error(
                    "ARCH_T2_ADR_REF_EXISTS",
                    format!("Architecture references non-existent ADR '{}'", adr_ref),
                    Some("architecture".to_string()),
                    Some("ARCHITECTURE.md".to_string()),
                ));
            }
        }

        // Security trust boundary completeness
        if arch.trust_boundaries.is_empty() {
            violations.push(QualityGateViolation::error(
                "ARCH_T2_TRUST_BOUNDARY",
                "Architecture lacks explicit security trust boundaries",
                Some("architecture".to_string()),
                Some("ARCHITECTURE.md".to_string()),
            ));
        }

        if violations.is_empty() {
            QualityGateResult::pass(QualityGateTier::Tier2RelationalConsistency)
        } else {
            QualityGateResult::fail(QualityGateTier::Tier2RelationalConsistency, violations)
        }
    }

    /// Evaluate Roadmap Tier 1 Gate (Section 40).
    pub fn evaluate_roadmap_tier1(roadmap: &Roadmap) -> QualityGateResult {
        let mut violations = Vec::new();

        if roadmap.phases.is_empty() {
            violations.push(QualityGateViolation::error(
                "ROADMAP_T1_NON_EMPTY",
                "Roadmap contains zero phases",
                Some("roadmap".to_string()),
                Some("ROADMAP.md".to_string()),
            ));
            return QualityGateResult::fail(QualityGateTier::Tier1SyntaxAndArtifacts, violations);
        }

        // DAG validation (cycles, self dependencies, unique IDs)
        if let Err(e) = roadmap.validate_dag() {
            violations.push(QualityGateViolation::error(
                "ROADMAP_T1_DAG_INVALID",
                format!("Roadmap DAG validation failed: {}", e),
                Some("roadmap".to_string()),
                Some("ROADMAP.md".to_string()),
            ));
        }

        for p in &roadmap.phases {
            if p.exit_criteria.is_empty() {
                violations.push(QualityGateViolation::error(
                    "ROADMAP_T1_EXIT_CRITERIA",
                    format!("Roadmap phase '{}' lacks exit criteria", p.id),
                    Some("roadmap".to_string()),
                    Some("ROADMAP.md".to_string()),
                ));
            }
        }

        if violations.is_empty() {
            QualityGateResult::pass(QualityGateTier::Tier1SyntaxAndArtifacts)
        } else {
            QualityGateResult::fail(QualityGateTier::Tier1SyntaxAndArtifacts, violations)
        }
    }

    /// Evaluate Roadmap Tier 2 Gate (Section 40).
    pub fn evaluate_roadmap_tier2(
        roadmap: &Roadmap,
        reqs: &RequirementsDocument,
        arch: &ArchitectureDocument,
    ) -> QualityGateResult {
        let mut violations = Vec::new();
        let mapped_reqs: HashSet<_> = roadmap
            .phases
            .iter()
            .flat_map(|p| p.requirement_refs.iter().cloned())
            .collect();

        // 100% of non-deferred mandatory requirements must be covered by at least one phase
        for r in &reqs.requirements {
            if r.priority == RequirementPriority::Must && !mapped_reqs.contains(&r.key) {
                violations.push(QualityGateViolation::error(
                    "ROADMAP_T2_REQ_COVERAGE",
                    format!(
                        "Mandatory requirement '{}' is not scheduled in any roadmap phase",
                        r.key
                    ),
                    Some("roadmap".to_string()),
                    Some("ROADMAP.md".to_string()),
                ));
            }
        }

        // Validate architecture references exist
        let comp_ids: HashSet<_> = arch.components.iter().map(|c| c.id.clone()).collect();
        let sub_ids: HashSet<_> = arch.subsystems.iter().map(|s| s.id.clone()).collect();

        for p in &roadmap.phases {
            for aref in &p.architecture_refs {
                if !comp_ids.contains(aref) && !sub_ids.contains(aref) {
                    violations.push(QualityGateViolation::error(
                        "ROADMAP_T2_ARCH_REF_EXISTS",
                        format!(
                            "Phase '{}' references non-existent architecture component/subsystem '{}'",
                            p.id, aref
                        ),
                        Some("roadmap".to_string()),
                        Some("ROADMAP.md".to_string()),
                    ));
                }
            }

            if p.verification_strategy.trim().is_empty() {
                violations.push(QualityGateViolation::error(
                    "ROADMAP_T2_VERIFICATION",
                    format!("Phase '{}' lacks verification strategy", p.id),
                    Some("roadmap".to_string()),
                    Some("ROADMAP.md".to_string()),
                ));
            }
        }

        if violations.is_empty() {
            QualityGateResult::pass(QualityGateTier::Tier2RelationalConsistency)
        } else {
            QualityGateResult::fail(QualityGateTier::Tier2RelationalConsistency, violations)
        }
    }
}
