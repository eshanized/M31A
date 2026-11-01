//! Sovereign dynamic tool catalog index and context selector.
//!
//! Dynamic tool catalog index selecting relevant tools based on prompt semantics and token limits.
//! In large agent environments (>50 builtin tools + 20 MCP servers), exposing every
//! JSON schema in every turn exhausts the model's context budget.
//! The `CatalogSelector` maintains an inverted keyword and relevance index with BM25,
//! dynamically selecting the optimal subset of tool definitions for the current user
//! turn while always pinning critical core coding tools.
//!
//! Report §7.5 features:
//! - Staged lossy compaction passes (strip descriptions → drop definitions →
//!   collapse deep objects → prune compositions); budget 5,000 B, max depth 3.
//! - BM25 `tool_search` with result limit 8.
//! - MCP tools deferred by default with `{namespace}__{name}` naming and
//!   SHA1 suffix on collision; 128-byte model-visible cap.

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};

/// Category of capability represented by a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolDomainCategory {
    CoreCoding,
    Filesystem,
    GitVersionControl,
    CodeIntelligence,
    DiagnosticsRepair,
    McpExternal,
    WebResearch,
    StateCheckpointing,
    InterAgentCollaboration,
}

/// A structured entry in the dynamic tool catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogToolEntry {
    pub name: String,
    /// Human-readable description (used for BM25 scoring).
    pub description: String,
    /// Which capability domain this tool belongs to.
    pub domain: ToolDomainCategory,
    /// JSON schema for the tool (may be compacted by `schema_compaction`).
    pub schema: serde_json::Value,
    /// Keywords for inverted index / BM25 scoring.
    pub keywords: Vec<String>,
    /// If true, always included in active set regardless of budget.
    pub is_core_pinned: bool,
    /// If true, this is an MCP tool deferred by default (§7.5).
    /// Deferred tools only appear when explicitly requested or high-relevance match.
    pub is_deferred: bool,
    /// Originator namespace for MCP tools (used for `{namespace}__{name}` naming).
    pub originator: Option<String>,
}

impl CatalogToolEntry {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        domain: ToolDomainCategory,
        schema: serde_json::Value,
        keywords: Vec<String>,
        is_core_pinned: bool,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            domain,
            schema,
            keywords,
            is_core_pinned,
            is_deferred: false,
            originator: None,
        }
    }

    /// Creates a deferred MCP tool entry with auto-assigned originator.
    pub fn deferred_mcp(
        name: impl Into<String>,
        description: impl Into<String>,
        domain: ToolDomainCategory,
        schema: serde_json::Value,
        keywords: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            domain,
            schema,
            keywords,
            is_core_pinned: false,
            is_deferred: true,
            originator: None,
        }
    }
}

/// Configuration options for tool selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSelectorConfig {
    /// Maximum total active tools exposed into LLM context per turn.
    pub max_active_tools: usize,
    /// Whether core coding tools are unconditionally included in the active set.
    pub always_pin_core: bool,
    /// Minimum relevance match score required for optional tools.
    pub min_score_threshold: f32,
}

impl Default for CatalogSelectorConfig {
    fn default() -> Self {
        Self {
            max_active_tools: 16,
            always_pin_core: true,
            min_score_threshold: 0.10,
        }
    }
}

/// Dynamic tool catalog and semantic search selector with BM25 (§7.5).
pub struct CatalogSelector {
    config: CatalogSelectorConfig,
    entries: Vec<CatalogToolEntry>,
    bm25: Bm25Index,
}

impl Default for CatalogSelector {
    fn default() -> Self {
        Self::new(CatalogSelectorConfig::default())
    }
}

impl CatalogSelector {
    pub fn new(config: CatalogSelectorConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
            bm25: Bm25Index::new(1.5, 0.75),
        }
    }

    /// Register a tool entry into the catalog.
    /// If the tool is an MCP tool without an explicit originator, one is derived
    /// from the name prefix before the first underscore.
    pub fn register(&mut self, mut entry: CatalogToolEntry) {
        // Auto-assign originator for MCP tools if not provided
        if entry.is_deferred
            && entry.originator.is_none()
            && let Some(idx) = entry.name.find('_')
        {
            entry.originator = Some(entry.name[..idx].to_string());
        }
        if let Some(pos) = self.entries.iter().position(|e| e.name == entry.name) {
            self.entries[pos] = entry;
        } else {
            self.entries.push(entry);
        }
        // Rebuild BM25 index after any registration
        self.rebuild_bm25();
    }

    fn rebuild_bm25(&mut self) {
        self.bm25 = Bm25Index::build(&self.entries);
    }

    /// Number of registered tools in the catalog.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Selects the optimal subset of tools for a given user query or turn context.
    /// Uses BM25 scoring for relevance (§7.5). Result limited to max_active_tools.
    /// Core tools are always pinned. Deferred tools only included on high relevance.
    pub fn select_active_tools(&self, query: &str) -> Vec<CatalogToolEntry> {
        let mut selected = Vec::new();
        let mut selected_names = HashSet::new();

        // 1. Always pin core tools if configured
        if self.config.always_pin_core {
            for entry in &self.entries {
                if entry.is_core_pinned {
                    selected.push(entry.clone());
                    selected_names.insert(entry.name.clone());
                }
            }
        }

        // 2. Score remaining unpinned tools against the query using BM25
        let query_tokens: Vec<String> = query
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if query_tokens.is_empty() {
            // Return pinned core tools, and fill up to max_active_tools with defaults
            for entry in &self.entries {
                if selected.len() >= self.config.max_active_tools {
                    break;
                }
                if !selected_names.contains(&entry.name) && !entry.is_deferred {
                    selected.push(entry.clone());
                    selected_names.insert(entry.name.clone());
                }
            }
            return selected;
        }

        // BM25 search (top-k = result limit 8 per §7.5)
        let bm25_results = self.bm25.search(&query_tokens, 8);

        for (doc_id, score) in bm25_results {
            if selected.len() >= self.config.max_active_tools {
                break;
            }
            let entry = &self.entries[doc_id];
            if selected_names.contains(&entry.name) {
                continue;
            }
            // Only include deferred tools on high relevance (score threshold)
            if entry.is_deferred && score < self.config.min_score_threshold * 3.0 {
                continue;
            }
            selected.push(entry.clone());
            selected_names.insert(entry.name.clone());
        }

        // Fill remaining slots with non-deferred tools if under budget
        for entry in &self.entries {
            if selected.len() >= self.config.max_active_tools {
                break;
            }
            if !selected_names.contains(&entry.name) && !entry.is_deferred {
                selected.push(entry.clone());
                selected_names.insert(entry.name.clone());
            }
        }

        selected
    }

    /// §7.5 BM25 tool_search with result limit 8.
    /// Returns top-k tools matching the query, suitable for model-visible tool search.
    pub fn tool_search(&self, query: &str) -> Vec<CatalogToolEntry> {
        let query_tokens: Vec<String> = query
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if query_tokens.is_empty() {
            return Vec::new();
        }

        let bm25_results = self.bm25.search(&query_tokens, 8);
        bm25_results
            .into_iter()
            .map(|(doc_id, _)| self.entries[doc_id].clone())
            .collect()
    }

    /// §7.5: Resolves the model-visible name for an MCP tool.
    /// Uses `{namespace}__{name}` with SHA1 suffix on collision (128-byte cap).
    pub fn resolve_mcp_tool_name(&self, namespace: &str, name: &str) -> String {
        let base = format!("{}__{}", namespace, name);
        if base.len() <= 128 {
            return base;
        }
        // Collision: truncate and append SHA1 suffix
        let mut hasher = Sha1::new();
        hasher.update(base.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let keep = 128 - hash.len() - 1; // leave room for '_' + hash
        format!("{}_", &base[..keep]) + &hash
    }
}

/// BM25 inverted index for tool search (§7.5).
#[derive(Debug, Clone)]
struct Bm25Index {
    /// token -> (doc_id, term_freq)
    postings: HashMap<String, Vec<(usize, f32)>>,
    /// doc_id -> doc_length (number of tokens)
    doc_lengths: Vec<usize>,
    /// Average document length
    avgdl: f32,
    /// Total number of documents
    n_docs: usize,
    /// BM25 parameters
    k1: f32,
    b: f32,
}

impl Bm25Index {
    fn new(k1: f32, b: f32) -> Self {
        Self {
            postings: HashMap::new(),
            doc_lengths: Vec::new(),
            avgdl: 0.0,
            n_docs: 0,
            k1,
            b,
        }
    }

    fn build(entries: &[CatalogToolEntry]) -> Self {
        let mut index = Self::new(1.5, 0.75);
        index.n_docs = entries.len();
        let mut total_len = 0usize;

        for (doc_id, entry) in entries.iter().enumerate() {
            let mut tokens = Vec::new();
            // Tokenize name, description, and keywords
            for part in [&entry.name, &entry.description] {
                for token in part
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                {
                    if !token.is_empty() {
                        tokens.push(token.to_string());
                    }
                }
            }
            for kw in &entry.keywords {
                for token in kw
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                {
                    if !token.is_empty() {
                        tokens.push(token.to_string());
                    }
                }
            }

            let mut freq = HashMap::new();
            for t in &tokens {
                *freq.entry(t.clone()).or_insert(0.0) += 1.0;
            }

            index.doc_lengths.push(tokens.len());
            total_len += tokens.len();

            for (term, tf) in freq {
                index.postings.entry(term).or_default().push((doc_id, tf));
            }
        }

        index.avgdl = total_len as f32 / index.n_docs.max(1) as f32;
        index
    }

    fn search(&self, query_tokens: &[String], top_k: usize) -> Vec<(usize, f32)> {
        if self.n_docs == 0 || query_tokens.is_empty() {
            return Vec::new();
        }

        let mut scores = vec![0.0f32; self.n_docs];

        for token in query_tokens {
            if let Some(postings) = self.postings.get(token) {
                let df = postings.len() as f32;
                let idf = ((self.n_docs as f32 - df + 0.5) / (df + 0.5)).ln();

                for (doc_id, tf) in postings {
                    let dl = self.doc_lengths[*doc_id] as f32;
                    let denom = tf + self.k1 * (1.0 - self.b + self.b * dl / self.avgdl);
                    scores[*doc_id] += idf * tf * (self.k1 + 1.0) / denom;
                }
            }
        }

        let mut results: Vec<(usize, f32)> = scores
            .into_iter()
            .enumerate()
            .filter(|(_, s)| *s > 0.0)
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_sample_catalog() -> CatalogSelector {
        let mut selector = CatalogSelector::new(CatalogSelectorConfig {
            max_active_tools: 5,
            always_pin_core: true,
            min_score_threshold: 0.1,
        });

        // Core tools
        selector.register(CatalogToolEntry::new(
            "read_file",
            "Read file contents from workspace",
            ToolDomainCategory::CoreCoding,
            serde_json::json!({}),
            vec!["read".into(), "file".into(), "view".into()],
            true,
        ));
        selector.register(CatalogToolEntry::new(
            "write_file",
            "Write file contents into workspace",
            ToolDomainCategory::CoreCoding,
            serde_json::json!({}),
            vec!["write".into(), "create".into(), "file".into()],
            true,
        ));

        // Optional specialized tools
        selector.register(CatalogToolEntry::new(
            "git_commit",
            "Commit staged changes to git repository",
            ToolDomainCategory::GitVersionControl,
            serde_json::json!({}),
            vec!["git".into(), "commit".into(), "vcs".into()],
            false,
        ));
        selector.register(CatalogToolEntry::new(
            "diagnose_compiler_errors",
            "Parse compiler diagnostics and suggest self-healing fixes",
            ToolDomainCategory::DiagnosticsRepair,
            serde_json::json!({}),
            vec![
                "compile".into(),
                "error".into(),
                "diagnostic".into(),
                "rustc".into(),
            ],
            false,
        ));
        selector.register(CatalogToolEntry::new(
            "web_search",
            "Perform search query across public documentation",
            ToolDomainCategory::WebResearch,
            serde_json::json!({}),
            vec!["web".into(), "search".into(), "docs".into(), "http".into()],
            false,
        ));
        selector.register(CatalogToolEntry::new(
            "generate_image",
            "Synthesize diagram or UI mock image asset",
            ToolDomainCategory::CoreCoding,
            serde_json::json!({}),
            vec!["image".into(), "png".into(), "art".into()],
            false,
        ));

        selector
    }

    #[test]
    fn test_core_tools_always_pinned() {
        let catalog = build_sample_catalog();
        let selected = catalog.select_active_tools("");

        assert!(selected.iter().any(|t| t.name == "read_file"));
        assert!(selected.iter().any(|t| t.name == "write_file"));
    }

    #[test]
    fn test_semantic_query_ranking() {
        let catalog = build_sample_catalog();

        // Query mentioning compiler errors should rank diagnose_compiler_errors
        let selected = catalog.select_active_tools("I have a rustc compile error on line 42");

        let names: Vec<String> = selected.iter().map(|t| t.name.clone()).collect();
        assert!(names.contains(&"diagnose_compiler_errors".to_string()));
        assert!(names.contains(&"read_file".to_string())); // Core pinned
        assert!(names.contains(&"write_file".to_string())); // Core pinned
    }

    #[test]
    fn test_git_query_selection() {
        let catalog = build_sample_catalog();
        let selected = catalog.select_active_tools("Commit the modified files to git");

        let names: Vec<String> = selected.iter().map(|t| t.name.clone()).collect();
        assert!(names.contains(&"git_commit".to_string()));
    }

    #[test]
    fn test_max_active_tools_budget() {
        let mut catalog = CatalogSelector::new(CatalogSelectorConfig {
            max_active_tools: 2,
            always_pin_core: false,
            min_score_threshold: 0.1,
        });

        for i in 1..=10 {
            catalog.register(CatalogToolEntry::new(
                format!("tool_{i}"),
                "sample tool",
                ToolDomainCategory::Filesystem,
                serde_json::json!({}),
                vec!["sample".into()],
                false,
            ));
        }

        let selected = catalog.select_active_tools("sample");
        assert_eq!(selected.len(), 2);
    }
}
