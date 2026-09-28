//! Research source provenance, evidence tracking, and citation models.

use super::research_decision::ResearchDimension;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Epistemic source category for researched evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchSourceType {
    /// Authoritative official framework/language documentation.
    OfficialDocs,
    /// Upstream source repository or reference implementation.
    RepoSource,
    /// Public issue tracker, bug report, or CVE disclosure.
    IssueTracker,
    /// Published performance benchmark or empirical measurement.
    Benchmark,
    /// Formal RFC, standard specification, or security advisory.
    StandardsDoc,
    /// Inspected files within the local target repository.
    LocalCodebase,
}

impl std::fmt::Display for ResearchSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OfficialDocs => write!(f, "Official Docs"),
            Self::RepoSource => write!(f, "Repository Source"),
            Self::IssueTracker => write!(f, "Issue Tracker"),
            Self::Benchmark => write!(f, "Benchmark"),
            Self::StandardsDoc => write!(f, "Standards RFC"),
            Self::LocalCodebase => write!(f, "Local Codebase"),
        }
    }
}

/// Verifiable evidence citation backing a research finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchEvidence {
    pub evidence_id: String,
    pub dimension: ResearchDimension,
    pub source_type: ResearchSourceType,
    pub source_uri: String,
    pub title: String,
    pub snippet: String,
    pub verified: bool,
    pub accessed_at: DateTime<Utc>,
}

impl ResearchEvidence {
    pub fn new(
        dimension: ResearchDimension,
        source_type: ResearchSourceType,
        source_uri: impl Into<String>,
        title: impl Into<String>,
        snippet: impl Into<String>,
    ) -> Self {
        Self {
            evidence_id: uuid::Uuid::now_v7().to_string(),
            dimension,
            source_type,
            source_uri: source_uri.into(),
            title: title.into(),
            snippet: snippet.into(),
            verified: true,
            accessed_at: Utc::now(),
        }
    }
}

/// A structured finding produced by an individual dimension researcher.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchFinding {
    pub dimension: ResearchDimension,
    pub topic: String,
    pub summary: String,
    pub recommendations: Vec<String>,
    pub tradeoffs: Vec<String>,
    pub evidence: Vec<ResearchEvidence>,
    pub confidence: f32,
    pub risks: Vec<String>,
}

impl ResearchFinding {
    pub fn new(
        dimension: ResearchDimension,
        topic: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            dimension,
            topic: topic.into(),
            summary: summary.into(),
            recommendations: Vec::new(),
            tradeoffs: Vec::new(),
            evidence: Vec::new(),
            confidence: 0.9,
            risks: Vec::new(),
        }
    }

    /// Render this finding into canonical markdown for a dimension artifact (e.g. STACK.md).
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("### {}\n\n", self.topic));
        out.push_str(&format!("{}\n\n", self.summary.trim()));

        if !self.recommendations.is_empty() {
            out.push_str("#### Recommendations\n");
            for r in &self.recommendations {
                out.push_str(&format!("- {}\n", r));
            }
            out.push('\n');
        }

        if !self.tradeoffs.is_empty() {
            out.push_str("#### Tradeoffs\n");
            for t in &self.tradeoffs {
                out.push_str(&format!("- {}\n", t));
            }
            out.push('\n');
        }

        if !self.risks.is_empty() {
            out.push_str("#### Risks & Hazards\n");
            for r in &self.risks {
                out.push_str(&format!("- {}\n", r));
            }
            out.push('\n');
        }

        if !self.evidence.is_empty() {
            out.push_str("#### Verified Evidence\n");
            for (idx, e) in self.evidence.iter().enumerate() {
                out.push_str(&format!(
                    "[{}] **{}** ({}) - *{}*\n> {}\n\n",
                    idx + 1,
                    e.title,
                    e.source_type,
                    e.source_uri,
                    e.snippet.replace('\n', "\n> ")
                ));
            }
        }

        out
    }
}
