//! Repository graph capability provider wrapping BoundedQueryEngine (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::repo::{CodeSearchResult, RepositoryService};
use crate::repo::query::{BoundedQueryEngine, QueryBounds};
use crate::repo::types::RepositorySymbol;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

/// Native provider for repository queries backed by BoundedQueryEngine and filesystem search.
pub struct RepositoryGraphProvider {
    engine: Option<Arc<BoundedQueryEngine>>,
    workspace_root: PathBuf,
}

impl RepositoryGraphProvider {
    /// Create a new provider for the workspace root with optional query engine.
    pub fn new(
        workspace_root: impl Into<PathBuf>,
        engine: Option<Arc<BoundedQueryEngine>>,
    ) -> Self {
        Self {
            engine,
            workspace_root: workspace_root.into(),
        }
    }
}

#[async_trait]
impl RepositoryService for RepositoryGraphProvider {
    async fn query_symbols(&self, query: &str) -> Result<Vec<RepositorySymbol>, CapabilityError> {
        let Some(engine) = &self.engine else {
            return Ok(Vec::new());
        };

        let result = engine.find_symbols(query, QueryBounds::default());
        Ok(result.data)
    }

    async fn query_dependencies(
        &self,
        symbol_id: &str,
    ) -> Result<Vec<RepositorySymbol>, CapabilityError> {
        let Some(engine) = &self.engine else {
            return Ok(Vec::new());
        };

        if let Some(hood) = engine.get_neighborhood(symbol_id, QueryBounds::default()) {
            let mut all = hood.data.callers;
            all.extend(hood.data.callees);
            all.extend(hood.data.dependencies);
            Ok(all)
        } else {
            Ok(Vec::new())
        }
    }

    async fn search_code(&self, query: &str) -> Result<Vec<CodeSearchResult>, CapabilityError> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        let mut dirs = vec![self.workspace_root.clone()];

        while let Some(dir) = dirs.pop() {
            let mut entries = match tokio::fs::read_dir(&dir).await {
                Ok(e) => e,
                Err(_) => continue,
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if let Ok(file_type) = entry.file_type().await {
                    if file_type.is_dir() {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if !name_str.starts_with('.')
                            && name_str != "target"
                            && name_str != "node_modules"
                        {
                            dirs.push(path);
                        }
                    } else if file_type.is_file()
                        && let Ok(content) = tokio::fs::read_to_string(&path).await
                    {
                        let rel_path = path
                            .strip_prefix(&self.workspace_root)
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|_| path.display().to_string());

                        for (idx, line) in content.lines().enumerate() {
                            if line.to_lowercase().contains(&query_lower) {
                                results.push(CodeSearchResult {
                                    file_path: rel_path.clone(),
                                    line_number: idx + 1,
                                    line_content: line.trim().to_string(),
                                });
                                if results.len() >= 100 {
                                    return Ok(results);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    async fn find_tests(&self, target: &str) -> Result<Vec<String>, CapabilityError> {
        let Some(engine) = &self.engine else {
            return Ok(Vec::new());
        };
        if target.contains("::") || target.contains('#') {
            let res = engine.find_tests_for_symbol(target, QueryBounds::default());
            Ok(res.data.into_iter().map(|s| s.file_path).collect())
        } else {
            let res = engine.find_tests_for_file(target, QueryBounds::default());
            Ok(res.data)
        }
    }

    async fn analyze_impact(
        &self,
        changed_files: &[String],
    ) -> Result<crate::repo::query::ChangeImpactReport, CapabilityError> {
        let Some(engine) = &self.engine else {
            return Ok(crate::repo::query::ChangeImpactReport {
                changed_files: changed_files.to_vec(),
                directly_affected_symbols: Vec::new(),
                downstream_affected_symbols: Vec::new(),
                affected_files: changed_files.to_vec(),
                recommended_tests: Vec::new(),
                affected_subsystems: Vec::new(),
            });
        };
        let res = engine.calculate_change_impact(changed_files, QueryBounds::default());
        Ok(res.data)
    }

    async fn get_overview(&self) -> Result<crate::repo::types::IndexStatus, CapabilityError> {
        let Some(engine) = &self.engine else {
            return Ok(crate::repo::types::IndexStatus::Empty);
        };
        Ok(engine.get_index_status())
    }
}
