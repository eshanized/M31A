//! Bidirectional traceability matrix linking requirements, research, architecture, ADRs, and roadmap phases.

use super::adr::{AdrId, AdrRegistry};
use super::architecture::ArchitectureDocument;
use super::requirements::RequirementsDocument;
use super::risks::RiskRegister;
use super::roadmap::Roadmap;
use crate::planning::requirements::{RequirementCategory, RequirementKey, RequirementPriority};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Bidirectional trace link connecting an engineering requirement across all planning layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceLink {
    pub requirement_key: RequirementKey,
    pub title: String,
    pub category: RequirementCategory,
    pub priority: RequirementPriority,
    pub source_evidence: Vec<String>,
    pub components: Vec<String>,
    pub adrs: Vec<AdrId>,
    pub phases: Vec<String>,
    pub verification_criteria: Vec<String>,
    pub is_deferred: bool,
}

/// Consolidated Traceability Matrix mapping every requirement to upstream and downstream artifacts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TraceabilityMatrix {
    pub links: BTreeMap<RequirementKey, TraceLink>,
}

impl TraceabilityMatrix {
    pub fn new() -> Self {
        Self {
            links: BTreeMap::new(),
        }
    }

    /// Build bidirectional matrix by scanning requirements, architecture, ADRs, and roadmap.
    pub fn build(
        requirements: &RequirementsDocument,
        arch: &ArchitectureDocument,
        adrs: &AdrRegistry,
        roadmap: &Roadmap,
        _risks: &RiskRegister,
    ) -> Self {
        let mut matrix = Self::new();

        // 1. Initialize links from requirements
        for req in &requirements.requirements {
            let mut evidence = Vec::new();
            if let Some(ref loc) = req.provenance.location {
                evidence.push(loc.clone());
            }
            if let Some(ref r) = req.provenance.reason {
                evidence.push(format!("Reason: {}", r));
            }

            let link = TraceLink {
                requirement_key: req.key.clone(),
                title: req.title_str().to_string(),
                category: req.category,
                priority: req.priority,
                source_evidence: evidence,
                components: Vec::new(),
                adrs: Vec::new(),
                phases: Vec::new(),
                verification_criteria: req.satisfaction_criteria.clone(),
                is_deferred: req.priority == RequirementPriority::Deferred,
            };
            matrix.links.insert(req.key.clone(), link);
        }

        // 2. Link components from architecture
        for comp in &arch.components {
            for req_key in &comp.requirement_refs {
                if let Some(link) = matrix
                    .links
                    .get_mut(req_key)
                    .filter(|l| !l.components.contains(&comp.id))
                {
                    link.components.push(comp.id.clone());
                }
            }
        }

        // 3. Link ADRs
        for adr in adrs.adrs.values() {
            for req_key in &adr.linked_requirements {
                if let Some(link) = matrix
                    .links
                    .get_mut(req_key)
                    .filter(|l| !l.adrs.contains(&adr.id))
                {
                    link.adrs.push(adr.id.clone());
                }
            }
        }

        // 4. Link roadmap phases
        for phase in &roadmap.phases {
            for req_key in &phase.requirement_refs {
                if let Some(link) = matrix.links.get_mut(req_key) {
                    if !link.phases.contains(&phase.id) {
                        link.phases.push(phase.id.clone());
                    }
                    if !link
                        .verification_criteria
                        .contains(&phase.verification_strategy)
                    {
                        link.verification_criteria
                            .push(phase.verification_strategy.clone());
                    }
                }
            }
        }

        matrix
    }

    /// Validate that no non-deferred requirement is orphaned (missing architecture or missing phase).
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut violations = Vec::new();

        for link in self.links.values() {
            if link.is_deferred {
                continue;
            }

            if link.components.is_empty() {
                violations.push(format!(
                    "Requirement '{}' has no mapped architecture components",
                    link.requirement_key
                ));
            }

            if link.phases.is_empty() {
                violations.push(format!(
                    "Requirement '{}' has no mapped roadmap phases",
                    link.requirement_key
                ));
            }

            if link.verification_criteria.is_empty() {
                violations.push(format!(
                    "Requirement '{}' has no verification criteria",
                    link.requirement_key
                ));
            }
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    /// Render Markdown representation of the Traceability Matrix.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("## Traceability Matrix\n\n");
        out.push_str(
            "| Requirement | Category | Priority | Components | ADRs | Phases | Verification |\n",
        );
        out.push_str("|---|---|---|---|---|---|---|\n");

        for link in self.links.values() {
            let comps = if link.components.is_empty() {
                "-".to_string()
            } else {
                link.components
                    .iter()
                    .map(|c| format!("`{}`", c))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let adr_str = if link.adrs.is_empty() {
                "-".to_string()
            } else {
                link.adrs
                    .iter()
                    .map(|a| format!("`{}`", a))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let phase_str = if link.phases.is_empty() {
                "-".to_string()
            } else {
                link.phases
                    .iter()
                    .map(|p| format!("`{}`", p))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let verif = if link.verification_criteria.is_empty() {
                "-".to_string()
            } else {
                format!("{} criteria", link.verification_criteria.len())
            };

            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | {} | {} | {} |\n",
                link.requirement_key,
                link.category,
                link.priority,
                comps,
                adr_str,
                phase_str,
                verif
            ));
        }
        out.push('\n');

        out
    }
}
