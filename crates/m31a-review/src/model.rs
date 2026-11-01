//! Data models for code review findings, severity, and reports.

use std::path::PathBuf;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum ReviewSeverity {
    Critical,
    High,
    Medium,
    Low,
    Suggestion,
    Praise,
}

impl ReviewSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "CRITICAL",
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
            Self::Suggestion => "SUGGESTION",
            Self::Praise => "PRAISE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ReviewCategory {
    Security,
    Correctness,
    Performance,
    Architecture,
    Maintainability,
}

impl ReviewCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Security => "Security",
            Self::Correctness => "Correctness",
            Self::Performance => "Performance",
            Self::Architecture => "Architecture",
            Self::Maintainability => "Maintainability",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewFinding {
    pub file_path: PathBuf,
    pub line_number: Option<usize>,
    pub severity: ReviewSeverity,
    pub category: ReviewCategory,
    pub title: String,
    pub description: String,
    pub code_snippet: Option<String>,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReviewVerdict {
    Approve,
    Comment,
    RequestChanges,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewReport {
    pub verdict: ReviewVerdict,
    pub risk_level: RiskLevel,
    pub files_reviewed: usize,
    pub total_additions: usize,
    pub total_deletions: usize,
    pub findings: Vec<ReviewFinding>,
    pub summary: String,
}

impl ReviewReport {
    pub fn critical_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::Critical)
            .count()
    }

    pub fn high_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::High)
            .count()
    }

    pub fn medium_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::Medium)
            .count()
    }
}
