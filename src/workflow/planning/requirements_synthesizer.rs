//! Requirements Synthesizer extracting EngineeringRequirements from Genesis artifacts.

use super::requirements::RequirementsDocument;
use crate::planning::requirements::{
    EngineeringRequirement, EpistemicStatus, Provenance, ProvenanceSourceType, RequirementCategory,
    RequirementKey, RequirementPriority, TrustLevel,
};
use crate::workflow::error::WorkflowError;
use crate::workflow::genesis::brownfield::BrownfieldMap;
use crate::workflow::genesis::environment::WorkspaceEnvironment;
use crate::workflow::genesis::project::ProjectCharter;
use crate::workflow::genesis::provenance::ResearchFinding;
use crate::workflow::genesis::synthesis::ResearchSummary;

/// Autonomous synthesizer deriving authoritative requirements from Project Genesis evidence.
pub struct RequirementsSynthesizer;

impl RequirementsSynthesizer {
    /// Synthesize formal requirements specification from charter, research summary, and brownfield facts.
    pub fn synthesize(
        charter: &ProjectCharter,
        summary: Option<&ResearchSummary>,
        findings: &[ResearchFinding],
        brownfield: Option<&BrownfieldMap>,
        _env: Option<&WorkspaceEnvironment>,
    ) -> Result<RequirementsDocument, WorkflowError> {
        let mut doc = RequirementsDocument::new(&charter.project_name, &charter.overview);

        // Unified per-prefix requirement counters (REQ-{PREFIX}-{NN}).
        // A single map keeps numbering collision-free across charter-,
        // finding-, and brownfield-derived requirements sharing a prefix.
        let mut prefix_counters: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        let next_key = |prefix: &str, counters: &mut std::collections::HashMap<String, u32>| {
            let idx = counters.entry(prefix.to_string()).or_insert(1);
            let key = RequirementKey::new(format!("REQ-{}-{:02}", prefix, *idx));
            *idx += 1;
            key
        };

        // 1. Extract Core Functional Requirements from ProjectCharter in_scope
        for (i, feature) in charter.boundaries.in_scope.iter().enumerate() {
            let key = next_key("FUNC", &mut prefix_counters);

            let prov = Provenance::new(
                ProvenanceSourceType::RepositoryFile,
                TrustLevel::VerifiedRepository,
                "genesis_charter",
            )
            .with_location(format!("PROJECT.md:boundaries.in_scope[{}]", i))
            .with_reason("Extracted from Project Charter in-scope features");

            let req = EngineeringRequirement::new(
                key,
                feature.clone(),
                EpistemicStatus::ExplicitUserRequirement,
                prov,
                vec![format!("Verify functionality of: {}", feature)],
            )
            .with_title(format!("Scope: {}", feature))
            .with_category(RequirementCategory::Functional)
            .with_priority(RequirementPriority::Must)
            .with_rationale("Mandated by project charter scope");

            doc.add_requirement(req)?;
        }

        // 2. Extract Security Requirements from Charter Security Invariants
        for (i, sec_req) in charter
            .operational_invariants
            .security_requirements
            .iter()
            .enumerate()
        {
            let key = next_key("SEC", &mut prefix_counters);

            let prov = Provenance::new(
                ProvenanceSourceType::RepositoryFile,
                TrustLevel::VerifiedRepository,
                "genesis_charter_security",
            )
            .with_location(format!("PROJECT.md:security_requirements[{}]", i))
            .with_reason("Extracted from operational security requirements");

            let req = EngineeringRequirement::new(
                key,
                sec_req.clone(),
                EpistemicStatus::ExplicitUserRequirement,
                prov,
                vec![format!("Security policy enforcement for: {}", sec_req)],
            )
            .with_title(format!("Security Policy: {}", sec_req))
            .with_category(RequirementCategory::Security)
            .with_priority(RequirementPriority::Must)
            .with_rationale("Mandatory security invariant");

            doc.add_requirement(req)?;
        }

        // Performance targets from charter
        for (i, perf) in charter
            .operational_invariants
            .performance_targets
            .iter()
            .enumerate()
        {
            let key = next_key("NF", &mut prefix_counters);

            let prov = Provenance::new(
                ProvenanceSourceType::RepositoryFile,
                TrustLevel::VerifiedRepository,
                "genesis_charter_performance",
            )
            .with_location(format!("PROJECT.md:performance_targets[{}]", i))
            .with_reason("Extracted from operational performance targets");

            let req = EngineeringRequirement::new(
                key,
                perf.clone(),
                EpistemicStatus::ExplicitUserRequirement,
                prov,
                vec![format!("Validate performance target: {}", perf)],
            )
            .with_title(format!("Performance: {}", perf))
            .with_category(RequirementCategory::Performance)
            .with_priority(RequirementPriority::Must)
            .with_rationale("Operational performance target");

            doc.add_requirement(req)?;
        }

        // Extract requirements from findings via their registered dimension
        // definitions. One generic path serves every registered dimension —
        // including dimensions added after this code was written. An unregistered
        // dimension id fails explicitly here: silently dropping a researched
        // dimension is never acceptable. Finding-derived requirements carry
        // `InferredFact` status: research summaries are model-derived evidence,
        // never verified facts.
        let dim_registry =
            crate::workflow::genesis::dimension_registry::ResearchDimensionRegistry::global()
                .read()
                .map_err(|_| {
                    WorkflowError::InvalidDefinition("dimension registry lock poisoned".to_string())
                })?;
        for finding in findings {
            let def = dim_registry.resolve(&finding.dimension).ok_or_else(|| {
                WorkflowError::InvalidDefinition(format!(
                    "unknown research dimension '{}': finding cannot be synthesized without a registered dimension definition",
                    finding.dimension.as_str()
                ))
            })?;
            let key = next_key(&def.requirement_prefix, &mut prefix_counters);
            let prov = Provenance::new(
                ProvenanceSourceType::RepositoryFile,
                TrustLevel::VerifiedRepository,
                def.agent_role.as_str(),
            )
            .with_location(format!("research/{}", def.artifact_filename))
            .with_reason(format!(
                "Extracted from {} research dimension",
                def.display_name
            ));
            let satisfaction = if finding.recommendations.is_empty() {
                vec![format!(
                    "Verify {} finding: {}",
                    def.display_name, finding.topic
                )]
            } else {
                finding.recommendations.clone()
            };
            let req = EngineeringRequirement::new(
                key,
                finding.summary.clone(),
                EpistemicStatus::InferredFact,
                prov,
                satisfaction,
            )
            .with_title(format!("{}: {}", def.display_name, finding.topic))
            .with_category(def.requirement_category)
            .with_priority(def.requirement_priority)
            .with_rationale(def.rationale.clone());
            doc.add_requirement(req)?;
        }
        drop(dim_registry);

        // 3. Process ResearchSummary Consensus Points & Tradeoffs
        if let Some(sum) = summary {
            for cp in &sum.consensus_points {
                let key = next_key("NF", &mut prefix_counters);

                let prov = Provenance::new(
                    ProvenanceSourceType::RepositoryFile,
                    TrustLevel::VerifiedRepository,
                    "genesis_charter_performance",
                )
                .with_location("research/SUMMARY.md")
                .with_reason("Synthesized multi-dimensional consensus");

                let req = EngineeringRequirement::new(
                    key,
                    cp.consensus.clone(),
                    // Model-synthesized consensus is inference, not verified fact.
                    EpistemicStatus::InferredFact,
                    prov,
                    vec![format!("Validate consensus point: {}", cp.topic)],
                )
                .with_title(format!("Consensus: {}", cp.topic))
                .with_category(RequirementCategory::NonFunctional)
                .with_priority(RequirementPriority::Should)
                .with_rationale("Derived from research consensus across dimensions");

                doc.add_requirement(req)?;
            }

            // Record Tradeoffs as Requirement Decisions
            for to in &sum.tradeoffs {
                doc.requirement_decisions.push(format!(
                    "Tradeoff '{}': Selected '{}' because {}",
                    to.tradeoff_axis, to.decision, to.rationale
                ));
            }

            // Record Open Unknowns
            for unk in &sum.open_unknowns {
                doc.unknown_requirements.push(format!(
                    "Area '{}' (Risk: {}): Mitigation — {}",
                    unk.area, unk.risk_level, unk.mitigation_strategy
                ));
            }
        }

        if let Some(bm) = brownfield {
            let key = next_key("COMPAT", &mut prefix_counters);

            let prov = Provenance::new(
                ProvenanceSourceType::RepositoryFile,
                TrustLevel::VerifiedRepository,
                "brownfield_map",
            )
            .with_location("BROWNFIELD.md")
            .with_reason("Brownfield compatibility constraint");

            let mut crit = vec![
                format!(
                    "Preserve primary language: {}",
                    bm.topology.primary_language
                ),
                "Pass existing test suite before adding new features".to_string(),
            ];
            for conv in &bm.existing_conventions {
                crit.push(format!("Preserve convention: {}", conv));
            }

            let req = EngineeringRequirement::new(
                key,
                format!(
                    "Maintain compatibility with existing {} codebase and manifest conventions",
                    bm.topology.primary_language
                ),
                // Tool-observed topology constraint: inferred, not verified.
                EpistemicStatus::InferredFact,
                prov,
                crit,
            )
            .with_title("Brownfield Codebase Compatibility")
            .with_category(RequirementCategory::Compatibility)
            .with_priority(RequirementPriority::Must)
            .with_rationale("Discovered pre-existing repository topology and conventions");

            doc.add_requirement(req)?;

            if let Some(ref delta) = bm.delta_scope {
                doc.requirement_decisions
                    .push(format!("Brownfield delta scope: {}", delta));
            }
        }

        // Honest absence: when no security evidence was
        // recorded in the charter or research, the runtime MUST NOT invent a
        // baseline security requirement. The gap is recorded as an explicit
        // unresolved question with decision provenance instead.
        if doc.by_category(RequirementCategory::Security).is_empty() {
            doc.requirement_decisions.push(
                "No security evidence recorded in charter or research; security requirements deferred pending explicit security input"
                    .to_string(),
            );
            doc.unresolved_questions.push(
                "Elicit security requirements: authentication model, input validation boundaries, and authorization rules"
                    .to_string(),
            );
        }

        doc.sort_deterministic();
        Ok(doc)
    }
}
