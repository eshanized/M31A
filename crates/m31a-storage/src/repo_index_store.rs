//! Durable SQLite persistence and incremental reconciliation for repository intelligence index.

use anyhow::Result;
use chrono::Utc;
use m31a_ast::{
    AstCodebaseIndex, AstSymbol, AstSymbolKind, LanguageId, RelationshipKind,
    ResolutionStatus, SourceRange, SymbolRelationship,
};
use rusqlite::{Connection, params};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Persistent repository index storage manager.
pub struct RepoIndexStore {
    conn: Arc<Mutex<Connection>>,
}

impl RepoIndexStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Derives stable repo ID from the root path.
    pub fn derive_repo_id(root: &Path) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(root.to_string_lossy().as_bytes());
        format!("repo_{}", &format!("{:x}", hasher.finalize())[..16])
    }

    /// Saves or atomically replaces the complete repository index into SQLite.
    pub fn save_index(&self, index: &AstCodebaseIndex) -> Result<()> {
        let repo_id = Self::derive_repo_id(index.root_path());
        let root_str = index.root_path().to_string_lossy().to_string();
        let repo_name = index
            .root_path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo")
            .to_string();

        let (branch, sha, is_monorepo) = if let Some(inv) = index.inventory() {
            (inv.git_branch.clone(), inv.git_head_sha.clone(), inv.is_monorepo)
        } else {
            (None, None, false)
        };

        let now = Utc::now().timestamp_millis();
        let mut guard = self.conn.lock().unwrap();
        let tx = guard.transaction()?;

        // 1. Upsert repo_indexes record
        tx.execute(
            r#"
            INSERT INTO repo_indexes (
                repo_id, root_path, repo_name, git_branch, git_head_sha, status,
                is_monorepo, files_indexed, symbols_indexed, updated_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(repo_id) DO UPDATE SET
                git_branch = excluded.git_branch,
                git_head_sha = excluded.git_head_sha,
                status = excluded.status,
                is_monorepo = excluded.is_monorepo,
                files_indexed = excluded.files_indexed,
                symbols_indexed = excluded.symbols_indexed,
                updated_at_epoch_ms = excluded.updated_at_epoch_ms;
            "#,
            params![
                repo_id,
                root_str,
                repo_name,
                branch,
                sha,
                index.status().as_str(),
                if is_monorepo { 1 } else { 0 },
                index.files_indexed() as i64,
                index.total_symbols() as i64,
                now,
            ],
        )?;

        // 2. Clear previous entries for this repo to maintain consistency
        tx.execute("DELETE FROM repo_symbols WHERE repo_id = ?1;", params![repo_id])?;
        tx.execute("DELETE FROM repo_relationships WHERE repo_id = ?1;", params![repo_id])?;
        tx.execute("DELETE FROM repo_indexed_files WHERE repo_id = ?1;", params![repo_id])?;

        // 3. Insert indexed files
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO repo_indexed_files (
                    id, repo_id, path, relative_path, content_hash, language,
                    size_bytes, is_test, package_id, updated_at_epoch_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);
                "#,
            )?;

            for (path, hash) in index.file_hashes() {
                let id = format!("{}_{}", repo_id, hash);
                let rel = path.strip_prefix(index.root_path()).unwrap_or(path);
                let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                let lang = LanguageId::from_path(path);

                stmt.execute(params![
                    id,
                    repo_id,
                    path.to_string_lossy().to_string(),
                    rel.to_string_lossy().to_string(),
                    hash,
                    lang.as_str(),
                    size as i64,
                    0,
                    Option::<String>::None,
                    now,
                ])?;
            }
        }

        // 4. Insert symbols
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO repo_symbols (
                    id, repo_id, file_path, name, qualified_name, kind, language,
                    start_line, end_line, signature, parent_scope, package_id,
                    visibility, is_public, doc_comment
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15);
                "#,
            )?;

            for sym in index.all_symbols() {
                stmt.execute(params![
                    sym.id,
                    repo_id,
                    sym.file_path.to_string_lossy().to_string(),
                    sym.name,
                    sym.qualified_name,
                    sym.kind.as_str(),
                    sym.language.as_str(),
                    sym.range.start_line as i64,
                    sym.range.end_line as i64,
                    sym.signature,
                    sym.parent_scope,
                    sym.package_id,
                    sym.visibility.as_str(),
                    if sym.is_public { 1 } else { 0 },
                    sym.doc_comment,
                ])?;
            }
        }

        // 5. Insert relationships
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO repo_relationships (
                    id, repo_id, source_id, target_id, source_symbol, target_symbol,
                    kind, file_path, line, status, confidence, provenance
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);
                "#,
            )?;

            for (idx, rel) in index.all_relationships().iter().enumerate() {
                let unique_id = format!("{}_{}", rel.id, idx);
                stmt.execute(params![
                    unique_id,
                    repo_id,
                    rel.source_id,
                    rel.target_id,
                    rel.source_symbol,
                    rel.target_symbol,
                    rel.kind.as_str(),
                    rel.file_path.to_string_lossy().to_string(),
                    rel.line as i64,
                    rel.status.as_str(),
                    rel.confidence,
                    rel.provenance,
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Loads the persisted index from SQLite if available.
    pub fn load_index(&self, root: &Path) -> Result<Option<AstCodebaseIndex>> {
        let repo_id = Self::derive_repo_id(root);
        let guard = self.conn.lock().unwrap();

        let exists: bool = guard.query_row(
            "SELECT 1 FROM repo_indexes WHERE repo_id = ?1;",
            params![repo_id],
            |_| Ok(true),
        ).unwrap_or(false);

        if !exists {
            return Ok(None);
        }

        let mut index = AstCodebaseIndex::new();

        // 1. Load symbols
        let mut sym_stmt = guard.prepare(
            r#"
            SELECT id, file_path, name, qualified_name, kind, language, start_line,
                   end_line, signature, parent_scope, package_id, visibility, is_public, doc_comment
            FROM repo_symbols WHERE repo_id = ?1;
            "#,
        )?;

        let sym_rows = sym_stmt.query_map(params![repo_id], |row| {
            let id: String = row.get(0)?;
            let file_path: String = row.get(1)?;
            let name: String = row.get(2)?;
            let qualified_name: String = row.get(3)?;
            let kind_str: String = row.get(4)?;
            let _lang_str: String = row.get(5)?;
            let start_line: i64 = row.get(6)?;
            let end_line: i64 = row.get(7)?;
            let signature: String = row.get(8)?;
            let parent_scope: Option<String> = row.get(9)?;
            let package_id: Option<String> = row.get(10)?;
            let _vis_str: String = row.get(11)?;
            let is_public: i64 = row.get(12)?;
            let doc_comment: Option<String> = row.get(13)?;

            let kind = parse_symbol_kind(&kind_str);
            let path_buf = PathBuf::from(file_path);
            let mut sym = AstSymbol::new(
                name,
                kind,
                path_buf,
                SourceRange::new(start_line as usize, end_line as usize, 0, 0),
                signature,
            );
            sym.id = id;
            sym.qualified_name = qualified_name;
            sym.parent_scope = parent_scope;
            sym.package_id = package_id;
            sym.is_public = is_public != 0;
            sym.doc_comment = doc_comment;

            Ok(sym)
        })?;

        for sym in sym_rows.flatten() {
            index.insert_symbol(sym);
        }

        // 2. Load relationships
        let mut rel_stmt = guard.prepare(
            r#"
            SELECT id, source_id, target_id, source_symbol, target_symbol, kind, file_path, line, status, confidence, provenance
            FROM repo_relationships WHERE repo_id = ?1;
            "#,
        )?;

        let rel_rows = rel_stmt.query_map(params![repo_id], |row| {
            let id: String = row.get(0)?;
            let source_id: String = row.get(1)?;
            let target_id: Option<String> = row.get(2)?;
            let source_symbol: String = row.get(3)?;
            let target_symbol: String = row.get(4)?;
            let kind_str: String = row.get(5)?;
            let file_path: String = row.get(6)?;
            let line: i64 = row.get(7)?;
            let status_str: String = row.get(8)?;
            let confidence: f64 = row.get(9)?;
            let provenance: String = row.get(10)?;

            let kind = parse_rel_kind(&kind_str);
            let status = parse_res_status(&status_str);

            Ok(SymbolRelationship {
                id,
                source_id,
                target_id,
                source_symbol,
                target_symbol,
                kind,
                file_path: PathBuf::from(file_path),
                line: line as usize,
                range: None,
                status,
                confidence,
                provenance,
                diagnostic: None,
            })
        })?;

        for rel in rel_rows.flatten() {
            index.symbol_graph().find_callers(&rel.target_symbol); // warm up
        }

        Ok(Some(index))
    }
}

fn parse_symbol_kind(s: &str) -> AstSymbolKind {
    match s {
        "function" => AstSymbolKind::Function,
        "method" => AstSymbolKind::Method,
        "struct" => AstSymbolKind::Struct,
        "enum" => AstSymbolKind::Enum,
        "class" => AstSymbolKind::Class,
        "interface" => AstSymbolKind::Interface,
        "trait" => AstSymbolKind::Trait,
        "type" => AstSymbolKind::TypeAlias,
        "const" => AstSymbolKind::Constant,
        "module" => AstSymbolKind::Module,
        "macro" => AstSymbolKind::Macro,
        _ => AstSymbolKind::Function,
    }
}

fn parse_rel_kind(s: &str) -> RelationshipKind {
    match s {
        "calls" => RelationshipKind::Calls,
        "references" => RelationshipKind::References,
        "implements" => RelationshipKind::Implements,
        "extends" => RelationshipKind::Extends,
        "tests" => RelationshipKind::Tests,
        "imports" => RelationshipKind::Imports,
        "exports" => RelationshipKind::Exports,
        "re_exports" => RelationshipKind::ReExports,
        _ => RelationshipKind::References,
    }
}

fn parse_res_status(s: &str) -> ResolutionStatus {
    match s {
        "exact" => ResolutionStatus::Exact,
        "ast_derived" => ResolutionStatus::AstDerived,
        "compiler_assisted" => ResolutionStatus::CompilerAssisted,
        "heuristic" => ResolutionStatus::Heuristic,
        _ => ResolutionStatus::Unresolved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_index_store_roundtrip() {
        let storage = crate::Storage::open_in_memory().unwrap();

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("m31a-ast");

        let index = AstCodebaseIndex::build(&root, 10);
        assert!(!index.all_symbols().is_empty());

        storage.save_repo_index(&index).unwrap();

        let loaded = storage.load_repo_index(&root).unwrap();
        assert!(loaded.is_some());
        let loaded_index = loaded.unwrap();
        assert_eq!(loaded_index.total_symbols(), index.total_symbols());
    }
}
