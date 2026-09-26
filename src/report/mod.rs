//! Deterministic Mission Completion Reporting and Evidence Linking (RPT-01..03, D-13).

pub mod evidence;
pub mod generator;
pub mod json;
pub mod markdown;
pub mod model;

pub use evidence::{EvidenceError, EvidenceLinker, EvidenceUri};
pub use generator::{ReportError, ReportGenerator};
pub use json::JsonReportProjector;
pub use markdown::MarkdownReportProjector;
pub use model::{
    AgentReportItem, ArtifactReportItem, CompletionReport, CompletionStatus, FailureReportItem,
    FileChangeReportItem, GitReportState, ModelReportItem, PlanReportSummary, PolicyReportItem,
    RecoveryReportItem, ReportCandidate, RequirementReportItem, ReviewReportItem, TaskReportItem,
    VerificationReportItem,
};
