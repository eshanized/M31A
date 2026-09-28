//! Multi-dimensional research synthesis, consensus extraction, and SUMMARY.md generation.

use super::errors::GenesisError;
use super::project::ProjectCharter;
use super::provenance::ResearchFinding;
use super::research_decision::ResearchDimension;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// An agreed-upon recommendation across multiple research dimensions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusPoint {
    pub topic: String,
    pub consensus: String,
    pub supporting_dimensions: Vec<ResearchDimension>,
}

/// A contradiction detected between research dimensions and its resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionResolution {
    pub topic: String,
    pub conflicting_views: Vec<String>,
    pub resolution: String,
    pub governing_invariant: String,
}

/// Tradeoff analysis balancing performance, complexity, safety, and delivery velocity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeoffAnalysis {
    pub tradeoff_axis: String,
    pub decision: String,
    pub rationale: String,
}

/// A candidate architecture, library, or approach rejected with evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedAlternative {
    pub candidate: String,
    pub rejected_for: String,
    pub evidence_rationale: String,
}

/// An unresolved uncertainty or domain risk requiring explicit mitigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenUnknown {
    pub area: String,
    pub risk_level: String,
    pub mitigation_strategy: String,
}

/// Aggregated synthesis artifact persisted to `.planning/research/SUMMARY.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchSummary {
    pub project_name: String,
    pub executive_summary: String,
    pub consensus_points: Vec<ConsensusPoint>,
    pub contradictions: Vec<ContradictionResolution>,
    pub tradeoffs: Vec<TradeoffAnalysis>,
    pub rejected_alternatives: Vec<RejectedAlternative>,
    pub open_unknowns: Vec<OpenUnknown>,
}

impl ResearchSummary {
    pub fn new(project_name: impl Into<String>, executive_summary: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            executive_summary: executive_summary.into(),
            consensus_points: Vec::new(),
            contradictions: Vec::new(),
            tradeoffs: Vec::new(),
            rejected_alternatives: Vec::new(),
            open_unknowns: Vec::new(),
        }
    }

    /// Validate that the summary meets quality gates.
    pub fn validate(&self) -> Result<(), GenesisError> {
        if self.executive_summary.trim().is_empty() {
            return Err(GenesisError::QualityGate(
                "ResearchSummary executive_summary cannot be empty".to_string(),
            ));
        }
        if self.consensus_points.is_empty() {
            return Err(GenesisError::QualityGate(
                "ResearchSummary must include at least one consensus point".to_string(),
            ));
        }
        Ok(())
    }

    /// Render into canonical markdown for `.planning/research/SUMMARY.md`.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Research Summary: {}\n\n", self.project_name));

        out.push_str("## Executive Summary\n");
        out.push_str(self.executive_summary.trim());
        out.push_str("\n\n");

        out.push_str("## Consensus Recommendations\n");
        for cp in &self.consensus_points {
            let dims = cp
                .supporting_dimensions
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "- **{}** (Supported by: {}): {}\n",
                cp.topic, dims, cp.consensus
            ));
        }
        out.push('\n');

        if !self.contradictions.is_empty() {
            out.push_str("## Resolved Contradictions\n");
            for cr in &self.contradictions {
                out.push_str(&format!("### {}\n", cr.topic));
                out.push_str("- **Conflicting Views**:\n");
                for v in &cr.conflicting_views {
                    out.push_str(&format!("  - {}\n", v));
                }
                out.push_str(&format!("- **Resolution**: {}\n", cr.resolution));
                out.push_str(&format!(
                    "- **Governing Invariant**: {}\n\n",
                    cr.governing_invariant
                ));
            }
        }

        out.push_str("## Tradeoff Analysis\n");
        for to in &self.tradeoffs {
            out.push_str(&format!("- **{}**:\n", to.tradeoff_axis));
            out.push_str(&format!("  - Decision: {}\n", to.decision));
            out.push_str(&format!("  - Rationale: {}\n", to.rationale));
        }
        out.push('\n');

        out.push_str("## Rejected Alternatives\n");
        if self.rejected_alternatives.is_empty() {
            out.push_str("- None formally rejected\n");
        } else {
            for ra in &self.rejected_alternatives {
                out.push_str(&format!(
                    "- **{}**: Rejected because {}. Evidence: {}\n",
                    ra.candidate, ra.rejected_for, ra.evidence_rationale
                ));
            }
        }
        out.push('\n');

        out.push_str("## Known Uncertainties & Risks\n");
        if self.open_unknowns.is_empty() {
            out.push_str("- None identified\n");
        } else {
            for ou in &self.open_unknowns {
                out.push_str(&format!(
                    "- **{}** (Risk: {}): Mitigation — {}\n",
                    ou.area, ou.risk_level, ou.mitigation_strategy
                ));
            }
        }
        out.push('\n');

        out
    }

    /// Persist to `<workspace_root>/<projection_dir>/research/SUMMARY.md`.
    pub fn save_to_dir(
        &self,
        workspace_root: &Path,
        projection_dir: &str,
    ) -> Result<PathBuf, GenesisError> {
        let dir = workspace_root.join(projection_dir).join("research");
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join("SUMMARY.md");
        std::fs::write(&file_path, self.to_markdown())?;
        Ok(file_path)
    }
}

/// Domain engine that synthesizes findings from research dimensions.
pub struct ResearchSynthesizer;

impl ResearchSynthesizer {
    /// Synthesize structured findings into a comprehensive ResearchSummary.
    pub fn synthesize(
        charter: &ProjectCharter,
        findings: &[ResearchFinding],
    ) -> Result<ResearchSummary, GenesisError> {
        let mut summary = ResearchSummary::new(
            &charter.project_name,
            format!(
                "Comprehensive research synthesized across {} domain findings for project '{}'.",
                findings.len(),
                charter.project_name
            ),
        );

        // Group findings by dimension
        for f in findings {
            summary.consensus_points.push(ConsensusPoint {
                topic: f.topic.clone(),
                consensus: f.summary.clone(),
                supporting_dimensions: vec![f.dimension.clone()],
            });

            for risk in &f.risks {
                summary.open_unknowns.push(OpenUnknown {
                    area: f.topic.clone(),
                    risk_level: "Medium".to_string(),
                    mitigation_strategy: risk.clone(),
                });
            }

            // Finding tradeoffs propagate into the tradeoff analysis with
            // their dimension as the axis context (genuine research propagation:
            // finding content must reach synthesis).
            for tradeoff in &f.tradeoffs {
                summary.tradeoffs.push(TradeoffAnalysis {
                    tradeoff_axis: format!("{} dimension: {}", f.dimension, f.topic),
                    decision: tradeoff.clone(),
                    rationale: format!(
                        "Recorded by {} research from gathered evidence",
                        f.dimension
                    ),
                });
            }
        }

        // Persistence tradeoff derives from charter storage evidence only.
        // Without evidence the selection stays an explicitly open unknown
        // instead of defaulting to an unevidenced storage technology.
        match charter.technical_preferences.storage.clone() {
            Some(storage) => summary.tradeoffs.push(TradeoffAnalysis {
                tradeoff_axis: "Persistence Strategy".to_string(),
                decision: storage,
                rationale:
                    "Balances operational simplicity and transactional guarantees for target workload"
                        .to_string(),
            }),
            None => summary.open_unknowns.push(OpenUnknown {
                area: "Durable storage technology selection".to_string(),
                risk_level: "Medium".to_string(),
                mitigation_strategy:
                    "Select storage from persistence evidence gathered during research or implementation planning"
                        .to_string(),
            }),
        }

        // Ensure minimum consensus points. When no findings exist this
        // fallback is explicitly labeled as an unevidenced baseline so that
        // downstream consumers never mistake it for researched consensus.
        // It attributes no dimension: claiming Stack/Architecture support
        // without evidence would be fabrication.
        if summary.consensus_points.is_empty() {
            summary.consensus_points.push(ConsensusPoint {
                topic: "Unevidenced Baseline (no research findings)".to_string(),
                consensus: "No research evidence was gathered; proceed only on charter evidence and explicit user confirmation"
                    .to_string(),
                supporting_dimensions: Vec::new(),
            });
        }

        Ok(summary)
    }

    /// Synthesize from raw markdown documents written by research dimension steps.
    ///
    /// Filenames resolve through the dimension registry. A document from an
    /// unregistered dimension is preserved in an explicitly-marked bucket —
    /// never misattributed to an unrelated dimension. Empty documents
    /// produce no consensus point: absence of content is not a finding.
    pub fn synthesize_from_dimension_texts(
        charter: &ProjectCharter,
        dimension_contents: &[(&str, &str)], // (filename, text)
    ) -> Result<ResearchSummary, GenesisError> {
        use super::dimension_registry::ResearchDimensionRegistry;
        let mut summary = ResearchSummary::new(
            &charter.project_name,
            format!(
                "Synthesized research from {} dimension documents for project '{}'.",
                dimension_contents.len(),
                charter.project_name
            ),
        );

        let registry = ResearchDimensionRegistry::global().read().map_err(|_| {
            GenesisError::ResearchDecision("dimension registry lock poisoned".to_string())
        })?;
        for (filename, text) in dimension_contents {
            let first_para = text
                .lines()
                .find(|l| !l.starts_with('#') && !l.trim().is_empty())
                .map(str::trim);
            let Some(first_para) = first_para else {
                tracing::warn!(
                    "research document '{}' carries no content; recording absence, not a finding",
                    filename
                );
                continue;
            };

            match registry.resolve_filename(filename) {
                Some(def) => {
                    summary.consensus_points.push(ConsensusPoint {
                        topic: format!("{} Findings", filename.trim_end_matches(".md")),
                        consensus: first_para.to_string(),
                        supporting_dimensions: vec![def.id.clone()],
                    });
                }
                None => {
                    summary.consensus_points.push(ConsensusPoint {
                        topic: format!(
                            "{} Findings (unregistered dimension)",
                            filename.trim_end_matches(".md")
                        ),
                        consensus: first_para.to_string(),
                        supporting_dimensions: Vec::new(),
                    });
                }
            }
        }

        // No fixed packaging tradeoff is emitted here: deployment shape is
        // target-derived from dimension evidence, never presupposed.

        Ok(summary)
    }
}
