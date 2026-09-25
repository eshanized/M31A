use crate::repo::graph::{RepositoryEdgeKind, RepositoryGraph};
use crate::repo::scanner::{ScanSummary, classify_file, detect_subsystem};
use crate::repo::types::{
    EntryPoint, EntryPointKind, FactClass, FileClassification, RepositorySymbol, SymbolKind,
};
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

/// Incremental SQLite repository cache backed by Migration 005 tables (REP-02, REP-03, D-10).
#[derive(Debug, Clone)]
pub struct SqliteRepoCache {
    pool: SqlitePool,
}

impl SqliteRepoCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Incremental sync for a file and its extracted symbols.
    /// Returns `Ok(false)` if the file's content_hash matches the cache (skips indexing).
    /// Returns `Ok(true)` if the file was new or modified and re-indexed.
    pub async fn sync_file(
        &self,
        file_path: &str,
        content_hash: &str,
        language: &str,
        symbols: &[RepositorySymbol],
    ) -> Result<bool, sqlx::Error> {
        // 1. Check existing cached hash
        let existing_hash: Option<String> =
            sqlx::query_scalar("SELECT content_hash FROM repo_cache_files WHERE file_path = ?")
                .bind(file_path)
                .fetch_optional(&self.pool)
                .await?;

        if let Some(ref hash) = existing_hash
            && hash == content_hash
        {
            return Ok(false);
        }

        // 2. Transactionally update file and symbols
        let mut tx = self.pool.begin().await?;

        let now = Utc::now().to_rfc3339();

        // Upsert file metadata
        sqlx::query(
            r#"
            INSERT INTO repo_cache_files (file_path, content_hash, language, last_indexed_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(file_path) DO UPDATE SET
                content_hash = excluded.content_hash,
                language = excluded.language,
                last_indexed_at = excluded.last_indexed_at
            "#,
        )
        .bind(file_path)
        .bind(content_hash)
        .bind(language)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        // Delete existing symbols for this file
        sqlx::query("DELETE FROM repo_cache_symbols WHERE file_path = ?")
            .bind(file_path)
            .execute(&mut *tx)
            .await?;

        // Insert new symbols
        for sym in symbols {
            sqlx::query(
                r#"
                INSERT INTO repo_cache_symbols (
                    id, file_path, name, qualified_name, kind,
                    start_line, end_line, signature, fact_class, doc_comment
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&sym.id)
            .bind(&sym.file_path)
            .bind(&sym.name)
            .bind(&sym.qualified_name)
            .bind(sym.kind.as_str())
            .bind(sym.start_line as i64)
            .bind(sym.end_line as i64)
            .bind(&sym.signature)
            .bind(sym.fact_class.as_str())
            .bind(&sym.doc_comment)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(true)
    }

    /// Persist inter-symbol relationships.
    pub async fn sync_edges(
        &self,
        edges: &[(&str, &str, RepositoryEdgeKind)],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        for (src, tgt, kind) in edges {
            sqlx::query(
                r#"
                INSERT INTO repo_cache_edges (source_symbol_id, target_symbol_id, edge_kind)
                VALUES (?, ?, ?)
                ON CONFLICT(source_symbol_id, target_symbol_id, edge_kind) DO NOTHING
                "#,
            )
            .bind(src)
            .bind(tgt)
            .bind(kind.as_str())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Load all cached symbols across the repository.
    pub async fn load_cached_symbols(&self) -> Result<Vec<RepositorySymbol>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_path, name, qualified_name, kind, start_line, end_line, signature, fact_class, doc_comment
            FROM repo_cache_symbols
            ORDER BY file_path, start_line
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut symbols = Vec::with_capacity(rows.len());
        for row in rows {
            let kind_str: String = row.get("kind");
            let fact_str: String = row.get("fact_class");

            symbols.push(RepositorySymbol::new(
                row.get::<String, _>("id"),
                row.get::<String, _>("name"),
                row.get::<String, _>("qualified_name"),
                SymbolKind::from_str_name(&kind_str).unwrap_or(SymbolKind::Variable),
                row.get::<String, _>("file_path"),
                row.get::<i64, _>("start_line") as usize,
                row.get::<i64, _>("end_line") as usize,
                row.get::<String, _>("signature"),
                FactClass::from_str_name(&fact_str).unwrap_or(FactClass::Unknown),
                row.get::<Option<String>, _>("doc_comment"),
            ));
        }

        Ok(symbols)
    }

    /// Rebuild the complete in-memory `RepositoryGraph` from persistent SQLite cache.
    pub async fn rebuild_graph_from_cache(
        &self,
        generation_id: u64,
    ) -> Result<RepositoryGraph, sqlx::Error> {
        let mut graph = RepositoryGraph::new(generation_id);

        // 1. Load files
        let file_rows =
            sqlx::query("SELECT file_path, language, content_hash FROM repo_cache_files")
                .fetch_all(&self.pool)
                .await?;

        for r in file_rows {
            let path: String = r.get("file_path");
            let lang: String = r.get("language");
            let content_hash: String = r.get("content_hash");
            graph.add_file(&path, &lang);
            graph.set_file_hash(&path, &content_hash);

            let classification = classify_file(&path, false, &[]);
            graph.set_file_classification(&path, classification);
            if let Some(sub) = detect_subsystem(&path) {
                graph.set_file_subsystem(&path, sub);
            }

            if path == "src/main.rs" || path == "main.rs" {
                graph.add_entry_point(EntryPoint::new(
                    format!("ep:main:{path}"),
                    EntryPointKind::BinaryMain,
                    &path,
                    Some("main".to_string()),
                    1,
                    "Application entry point",
                ));
            } else if path == "src/lib.rs" || path == "lib.rs" {
                graph.add_entry_point(EntryPoint::new(
                    format!("ep:lib:{path}"),
                    EntryPointKind::LibraryRoot,
                    &path,
                    None,
                    1,
                    "Crate library interface",
                ));
            } else if classification == FileClassification::Test {
                graph.add_entry_point(EntryPoint::new(
                    format!("ep:test:{path}"),
                    EntryPointKind::TestEntry,
                    &path,
                    None,
                    1,
                    "Integration test suite",
                ));
            }
        }

        // 2. Load symbols
        let symbols = self.load_cached_symbols().await?;
        for sym in symbols {
            let parent_idx = graph.file_nodes.get(&sym.file_path).copied();
            graph.add_symbol(sym, parent_idx);
        }

        // 3. Load edges
        let edge_rows = sqlx::query(
            "SELECT source_symbol_id, target_symbol_id, edge_kind FROM repo_cache_edges",
        )
        .fetch_all(&self.pool)
        .await?;

        for r in edge_rows {
            let src: String = r.get("source_symbol_id");
            let tgt: String = r.get("target_symbol_id");
            let kind_str: String = r.get("edge_kind");
            if let Some(kind) = RepositoryEdgeKind::from_str_name(&kind_str) {
                graph.add_relationship(&src, &tgt, kind);
            }
        }

        Ok(graph)
    }

    /// Remove a file and its symbols and edges from the cache.
    pub async fn remove_file(&self, file_path: &str) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM repo_cache_symbols WHERE file_path = ?")
            .bind(file_path)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM repo_cache_files WHERE file_path = ?")
            .bind(file_path)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Incrementally synchronize an entire `ScanSummary` into the SQLite cache,
    /// cleaning up deleted files, updating modified files, and syncing edges.
    pub async fn sync_scan(&self, summary: &ScanSummary) -> Result<usize, sqlx::Error> {
        let mut modified_count = 0;
        let mut scanned_paths = HashSet::new();

        for file in &summary.files {
            scanned_paths.insert(file.relative_path.clone());
            let updated = self
                .sync_file(
                    &file.relative_path,
                    &file.content_hash,
                    file.language.as_str(),
                    &file.symbols,
                )
                .await?;
            if updated {
                modified_count += 1;
            }
        }

        // Clean up files in cache that no longer exist
        let cached_files: Vec<String> =
            sqlx::query_scalar("SELECT file_path FROM repo_cache_files")
                .fetch_all(&self.pool)
                .await?;

        for cached_path in cached_files {
            if !scanned_paths.contains(&cached_path) {
                self.remove_file(&cached_path).await?;
                modified_count += 1;
            }
        }

        // Sync edges
        let edge_refs: Vec<(&str, &str, RepositoryEdgeKind)> = summary
            .edges
            .iter()
            .map(|(s, t, k)| (s.as_str(), t.as_str(), *k))
            .collect();
        self.sync_edges(&edge_refs).await?;

        Ok(modified_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::sqlite::schema::run_migrations;
    use crate::repo::types::SubsystemKind;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_sqlite_repo_cache_incremental_sync() {
        let pool = create_test_db().await;
        let cache = SqliteRepoCache::new(pool);

        let sym1 = RepositorySymbol::new(
            "src/main.rs::fn::main",
            "main",
            "crate::main",
            SymbolKind::Function,
            "src/main.rs",
            1,
            5,
            "fn main()",
            FactClass::VerifiedFact,
            None,
        );

        // First sync: new file -> returns true (indexed)
        let indexed = cache
            .sync_file(
                "src/main.rs",
                "hash_v1",
                "rust",
                std::slice::from_ref(&sym1),
            )
            .await
            .unwrap();
        assert!(indexed);

        // Second sync with identical hash -> returns false (skipped)
        let skipped = cache
            .sync_file(
                "src/main.rs",
                "hash_v1",
                "rust",
                std::slice::from_ref(&sym1),
            )
            .await
            .unwrap();
        assert!(!skipped);

        // Third sync with new hash -> returns true (re-indexed)
        let updated = cache
            .sync_file(
                "src/main.rs",
                "hash_v2",
                "rust",
                std::slice::from_ref(&sym1),
            )
            .await
            .unwrap();
        assert!(updated);

        // Check symbols
        let loaded = cache.load_cached_symbols().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "main");
    }

    #[tokio::test]
    async fn test_sqlite_repo_cache_rebuild_graph() {
        let pool = create_test_db().await;
        let cache = SqliteRepoCache::new(pool);

        let sym1 = RepositorySymbol::new(
            "src/lib.rs::fn::calc",
            "calc",
            "crate::calc",
            SymbolKind::Function,
            "src/lib.rs",
            1,
            10,
            "pub fn calc() -> i32",
            FactClass::VerifiedFact,
            None,
        );

        let sym2 = RepositorySymbol::new(
            "src/lib.rs::struct::Calculator",
            "Calculator",
            "crate::Calculator",
            SymbolKind::Struct,
            "src/lib.rs",
            12,
            20,
            "pub struct Calculator",
            FactClass::VerifiedFact,
            None,
        );

        cache
            .sync_file("src/lib.rs", "h123", "rust", &[sym1, sym2])
            .await
            .unwrap();
        cache
            .sync_edges(&[(
                "src/lib.rs::fn::calc",
                "src/lib.rs::struct::Calculator",
                RepositoryEdgeKind::Calls,
            )])
            .await
            .unwrap();

        let graph = cache.rebuild_graph_from_cache(10).await.unwrap();
        assert_eq!(graph.generation_id, 10);
        assert_eq!(graph.node_count(), 3); // 1 file + 2 symbols
        assert_eq!(graph.edge_count(), 3); // 2 Contains + 1 Calls

        let sym = graph.get_symbol("src/lib.rs::fn::calc").unwrap();
        assert_eq!(sym.name, "calc");
    }

    #[tokio::test]
    async fn test_sync_scan_and_remove_file() {
        let pool = create_test_db().await;
        let cache = SqliteRepoCache::new(pool);

        let sym1 = RepositorySymbol::new(
            "src/capability/service.rs::fn::run",
            "run",
            "crate::service::run",
            SymbolKind::Function,
            "src/capability/service.rs",
            1,
            5,
            "pub fn run()",
            FactClass::VerifiedFact,
            None,
        );

        let scanned_file = crate::repo::scanner::ScannedFile {
            relative_path: "src/capability/service.rs".to_string(),
            language: crate::repo::scanner::SourceLanguage::Rust,
            content_hash: "hash_scan_1".to_string(),
            size_bytes: 120,
            classification: FileClassification::Source,
            subsystem: Some(SubsystemKind::Capability),
            symbols: vec![sym1],
        };

        let summary = ScanSummary {
            files: vec![scanned_file],
            total_symbols: 1,
            scanned_file_count: 1,
            edges: vec![(
                "src/capability/service.rs::fn::run".to_string(),
                "src/capability/service.rs::fn::run".to_string(),
                RepositoryEdgeKind::Calls,
            )],
            entry_points: Vec::new(),
            ignored_file_count: 0,
        };

        let count = cache.sync_scan(&summary).await.unwrap();
        assert_eq!(count, 1);

        let graph = cache.rebuild_graph_from_cache(1).await.unwrap();
        assert_eq!(
            graph.get_all_files(),
            vec!["src/capability/service.rs".to_string()]
        );
        assert_eq!(
            graph.get_file_subsystem("src/capability/service.rs"),
            SubsystemKind::Capability
        );

        // Remove file
        cache
            .remove_file("src/capability/service.rs")
            .await
            .unwrap();
        let empty_graph = cache.rebuild_graph_from_cache(2).await.unwrap();
        assert!(empty_graph.get_all_files().is_empty());
    }
}
