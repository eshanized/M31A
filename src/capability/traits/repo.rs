//! Repository graph and code navigation service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use crate::repo::query::ChangeImpactReport;
use crate::repo::types::{IndexStatus, RepositorySymbol};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Individual code search match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CodeSearchResult {
    pub file_path: String,
    pub line_number: usize,
    pub line_content: String,
}

/// Asynchronous service seam for repository queries, symbol index, and code search.
#[async_trait]
pub trait RepositoryService: Send + Sync + 'static {
    /// Query symbols by name or prefix across the code graph.
    async fn query_symbols(&self, query: &str) -> Result<Vec<RepositorySymbol>, CapabilityError>;

    /// Find callers, callees, or dependencies of a symbol.
    async fn query_dependencies(
        &self,
        symbol_id: &str,
    ) -> Result<Vec<RepositorySymbol>, CapabilityError>;

    /// Search repository text files for matching query terms.
    async fn search_code(&self, query: &str) -> Result<Vec<CodeSearchResult>, CapabilityError>;

    /// Find all tests relevant to a target symbol ID or file path.
    async fn find_tests(&self, target: &str) -> Result<Vec<String>, CapabilityError>;

    /// Compute change impact analysis for a set of target files.
    async fn analyze_impact(
        &self,
        changed_files: &[String],
    ) -> Result<ChangeImpactReport, CapabilityError>;

    /// Check repository indexing status and staleness.
    async fn get_overview(&self) -> Result<IndexStatus, CapabilityError>;
}
