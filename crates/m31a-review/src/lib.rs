//! `m31a-review`: Code review engine, static vulnerability analysis, and multi-model consensus lanes.
//!
//! Analyzes code modifications for bugs, security weaknesses, and architectural regressions,
//! aggregating findings across multiple model perspectives into actionable reports.

pub mod analyzer;
pub mod engine;
pub mod formatter;
pub mod lanes;
pub mod model;

pub use analyzer::StaticDiffAnalyzer;
pub use engine::ReviewEngine;
pub use formatter::ReviewFormatter;
pub use lanes::{
    ClaimStatus, ConsensusFinding, ConsensusReviewReport, MultiLaneReviewDispatcher,
    PrimaryConsolidator, ReviewerClaim, ReviewerLaneConfig,
};
pub use model::{
    ReviewCategory, ReviewFinding, ReviewReport, ReviewSeverity, ReviewVerdict, RiskLevel,
};
