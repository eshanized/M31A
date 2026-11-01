//! Private rollout notes and virtual agent filesystem storage.
//!
//! Virtual persistent notes store maintaining agent memory across context compaction boundaries.
//! Allows agents and subagents to maintain private notes that survive context-window
//! compactions, with virtual path sandboxing and 1MB size bounds.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum allowed note size per virtual file (1,000,000 UTF-8 bytes).
pub const MAX_NOTE_BYTES: usize = 1_000_000;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum NotesError {
    #[error("note `{path}` for agent `{agent}` not found")]
    NotFound { agent: String, path: String },

    #[error("note size ({actual} bytes) exceeds maximum allowable limit of {MAX_NOTE_BYTES} bytes")]
    PayloadTooLarge { actual: usize },

    #[error("invalid virtual note path `{0}`: {1}")]
    InvalidPath(String, &'static str),

    #[error("database error: {0}")]
    DbError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoteFileInfo {
    pub agent_id: String,
    pub virtual_path: String,
    pub size_bytes: usize,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoteSearchResult {
    pub agent_id: String,
    pub virtual_path: String,
    pub match_line: usize,
    pub snippet: String,
}

/// Normalizes and validates a virtual notes path:
/// - Strips leading/trailing slashes.
/// - Rejects empty paths.
/// - Rejects path traversal (`.` or `..`).
/// - Rejects NUL bytes.
pub fn validate_and_normalize_path(raw_path: &str) -> Result<String, NotesError> {
    let clean = raw_path.trim().replace('\\', "/");
    let clean = clean.trim_matches('/');

    if clean.is_empty() {
        return Err(NotesError::InvalidPath(
            raw_path.to_string(),
            "path cannot be empty",
        ));
    }

    if clean.contains('\0') {
        return Err(NotesError::InvalidPath(
            raw_path.to_string(),
            "path cannot contain NUL byte",
        ));
    }

    for component in clean.split('/') {
        if component.is_empty() {
            return Err(NotesError::InvalidPath(
                raw_path.to_string(),
                "path contains empty segment",
            ));
        }
        if component == "." || component == ".." {
            return Err(NotesError::InvalidPath(
                raw_path.to_string(),
                "relative traversal ('.' or '..') is forbidden",
            ));
        }
    }

    Ok(clean.to_string())
}

/// Thread-safe Rollout Notes storage manager backed by SQLite.
#[derive(Clone)]
pub struct RolloutNotesManager {
    conn: Arc<std::sync::Mutex<Connection>>,
}

impl RolloutNotesManager {
    pub fn new(conn: Arc<std::sync::Mutex<Connection>>) -> Result<Self, NotesError> {
        let lock = conn
            .lock()
            .map_err(|e| NotesError::DbError(e.to_string()))?;
        lock.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS rollout_notes (
                agent_id TEXT NOT NULL,
                virtual_path TEXT NOT NULL,
                content TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (agent_id, virtual_path)
            );
            CREATE INDEX IF NOT EXISTS idx_rollout_notes_prefix 
                ON rollout_notes(agent_id, virtual_path);
            "#,
        )
        .map_err(|e| NotesError::DbError(e.to_string()))?;

        drop(lock);
        Ok(Self { conn })
    }

    /// Creates or completely overwrites a private note file for the agent.
    pub fn write_file(
        &self,
        agent_id: &str,
        path: &str,
        content: &str,
    ) -> Result<NoteFileInfo, NotesError> {
        let normalized = validate_and_normalize_path(path)?;
        let size = content.len();

        if size > MAX_NOTE_BYTES {
            return Err(NotesError::PayloadTooLarge { actual: size });
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let lock = self
            .conn
            .lock()
            .map_err(|e| NotesError::DbError(e.to_string()))?;

        lock.execute(
            r#"
            INSERT INTO rollout_notes (agent_id, virtual_path, content, size_bytes, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(agent_id, virtual_path) DO UPDATE SET
                content = excluded.content,
                size_bytes = excluded.size_bytes,
                updated_at = excluded.updated_at
            "#,
            params![agent_id, normalized, content, size as i64, now as i64],
        )
        .map_err(|e| NotesError::DbError(e.to_string()))?;

        Ok(NoteFileInfo {
            agent_id: agent_id.to_string(),
            virtual_path: normalized,
            size_bytes: size,
            updated_at: now,
        })
    }

    /// Appends content to an existing note file, or creates it if absent.
    pub fn append_to_file(
        &self,
        agent_id: &str,
        path: &str,
        extra_content: &str,
    ) -> Result<NoteFileInfo, NotesError> {
        let normalized = validate_and_normalize_path(path)?;

        let lock = self
            .conn
            .lock()
            .map_err(|e| NotesError::DbError(e.to_string()))?;

        let existing: Option<String> = lock
            .query_row(
                "SELECT content FROM rollout_notes WHERE agent_id = ?1 AND virtual_path = ?2",
                params![agent_id, normalized],
                |row| row.get(0),
            )
            .ok();

        let mut combined = existing.unwrap_or_default();
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(extra_content);

        let size = combined.len();
        if size > MAX_NOTE_BYTES {
            return Err(NotesError::PayloadTooLarge { actual: size });
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        lock.execute(
            r#"
            INSERT INTO rollout_notes (agent_id, virtual_path, content, size_bytes, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(agent_id, virtual_path) DO UPDATE SET
                content = excluded.content,
                size_bytes = excluded.size_bytes,
                updated_at = excluded.updated_at
            "#,
            params![agent_id, normalized, combined, size as i64, now as i64],
        )
        .map_err(|e| NotesError::DbError(e.to_string()))?;

        Ok(NoteFileInfo {
            agent_id: agent_id.to_string(),
            virtual_path: normalized,
            size_bytes: size,
            updated_at: now,
        })
    }

    /// Reads the raw text content of a virtual note file.
    pub fn read_file(&self, agent_id: &str, path: &str) -> Result<String, NotesError> {
        let normalized = validate_and_normalize_path(path)?;
        let lock = self
            .conn
            .lock()
            .map_err(|e| NotesError::DbError(e.to_string()))?;

        lock.query_row(
            "SELECT content FROM rollout_notes WHERE agent_id = ?1 AND virtual_path = ?2",
            params![agent_id, normalized],
            |row| row.get(0),
        )
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => NotesError::NotFound {
                agent: agent_id.to_string(),
                path: normalized,
            },
            e => NotesError::DbError(e.to_string()),
        })
    }

    /// Lists note files for an agent, optionally filtering by path prefix.
    pub fn list_files_by_prefix(
        &self,
        agent_id: &str,
        prefix: Option<&str>,
    ) -> Result<Vec<NoteFileInfo>, NotesError> {
        let lock = self
            .conn
            .lock()
            .map_err(|e| NotesError::DbError(e.to_string()))?;

        let (query, filter_prefix) = if let Some(p) = prefix {
            let p_clean = p.trim().trim_matches('/');
            (
                "SELECT agent_id, virtual_path, size_bytes, updated_at FROM rollout_notes 
                 WHERE agent_id = ?1 AND virtual_path LIKE ?2 || '%' 
                 ORDER BY virtual_path ASC",
                format!("{p_clean}/"),
            )
        } else {
            (
                "SELECT agent_id, virtual_path, size_bytes, updated_at FROM rollout_notes 
                 WHERE agent_id = ?1 
                 ORDER BY virtual_path ASC",
                String::new(),
            )
        };

        fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<NoteFileInfo> {
            Ok(NoteFileInfo {
                agent_id: row.get(0)?,
                virtual_path: row.get(1)?,
                size_bytes: row.get::<_, i64>(2)? as usize,
                updated_at: row.get::<_, i64>(3)? as u64,
            })
        }

        let mut out = Vec::new();
        if prefix.is_some() {
            let mut stmt = lock
                .prepare(query)
                .map_err(|e| NotesError::DbError(e.to_string()))?;
            let rows = stmt
                .query_map(params![agent_id, filter_prefix], map_row)
                .map_err(|e| NotesError::DbError(e.to_string()))?;
            for r in rows.flatten() {
                out.push(r);
            }
        } else {
            let mut stmt = lock
                .prepare(query)
                .map_err(|e| NotesError::DbError(e.to_string()))?;
            let rows = stmt
                .query_map(params![agent_id], map_row)
                .map_err(|e| NotesError::DbError(e.to_string()))?;
            for r in rows.flatten() {
                out.push(r);
            }
        }

        Ok(out)
    }

    /// Performs substring search across an agent's notes, returning matching lines and snippets.
    pub fn search_contents(
        &self,
        agent_id: &str,
        query: &str,
    ) -> Result<Vec<NoteSearchResult>, NotesError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let lock = self
            .conn
            .lock()
            .map_err(|e| NotesError::DbError(e.to_string()))?;

        let mut stmt = lock
            .prepare(
                "SELECT virtual_path, content FROM rollout_notes 
                 WHERE agent_id = ?1 AND content LIKE '%' || ?2 || '%'",
            )
            .map_err(|e| NotesError::DbError(e.to_string()))?;

        let rows = stmt
            .query_map(params![agent_id, query], |row| {
                let vpath: String = row.get(0)?;
                let content: String = row.get(1)?;
                Ok((vpath, content))
            })
            .map_err(|e| NotesError::DbError(e.to_string()))?;

        let mut results = Vec::new();
        let query_lower = query.to_ascii_lowercase();

        for r in rows {
            let (vpath, content) = r.map_err(|e| NotesError::DbError(e.to_string()))?;
            for (idx, line) in content.lines().enumerate() {
                if line.to_ascii_lowercase().contains(&query_lower) {
                    results.push(NoteSearchResult {
                        agent_id: agent_id.to_string(),
                        virtual_path: vpath.clone(),
                        match_line: idx + 1,
                        snippet: line.trim().to_string(),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Deletes a specific note file.
    pub fn delete_file(&self, agent_id: &str, path: &str) -> Result<bool, NotesError> {
        let normalized = validate_and_normalize_path(path)?;
        let lock = self
            .conn
            .lock()
            .map_err(|e| NotesError::DbError(e.to_string()))?;

        let count = lock
            .execute(
                "DELETE FROM rollout_notes WHERE agent_id = ?1 AND virtual_path = ?2",
                params![agent_id, normalized],
            )
            .map_err(|e| NotesError::DbError(e.to_string()))?;

        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_manager() -> RolloutNotesManager {
        let conn = Connection::open_in_memory().unwrap();
        RolloutNotesManager::new(Arc::new(std::sync::Mutex::new(conn))).unwrap()
    }

    #[test]
    fn test_virtual_path_validation() {
        assert_eq!(validate_and_normalize_path("plan.md").unwrap(), "plan.md");
        assert_eq!(
            validate_and_normalize_path("/architecture/database.md").unwrap(),
            "architecture/database.md"
        );
        assert_eq!(
            validate_and_normalize_path("deep\\nested\\file.txt").unwrap(),
            "deep/nested/file.txt"
        );

        // Forbidden traversal & empty
        assert!(validate_and_normalize_path("").is_err());
        assert!(validate_and_normalize_path("..").is_err());
        assert!(validate_and_normalize_path("foo/../bar").is_err());
        assert!(validate_and_normalize_path("foo/./bar").is_err());
    }

    #[test]
    fn test_notes_write_read_append_lifecycle() {
        let mgr = in_memory_manager();

        // 1. Write file
        let info = mgr
            .write_file(
                "agent-1",
                "research/summary.md",
                "# Findings\nInitial report",
            )
            .unwrap();
        assert_eq!(info.virtual_path, "research/summary.md");
        assert_eq!(info.size_bytes, 25);

        // 2. Read file
        let content = mgr.read_file("agent-1", "research/summary.md").unwrap();
        assert_eq!(content, "# Findings\nInitial report");

        // 3. Append to file
        let app_info = mgr
            .append_to_file("agent-1", "research/summary.md", "Second paragraph")
            .unwrap();
        assert!(app_info.size_bytes > 25);

        let updated = mgr.read_file("agent-1", "research/summary.md").unwrap();
        assert!(updated.contains("Initial report"));
        assert!(updated.contains("Second paragraph"));

        // 4. Cross-agent isolation
        let other_read = mgr.read_file("agent-2", "research/summary.md");
        assert!(matches!(other_read, Err(NotesError::NotFound { .. })));
    }

    #[test]
    fn test_list_and_search_notes() {
        let mgr = in_memory_manager();
        mgr.write_file("agent-1", "backend/api.md", "REST API routes documentation")
            .unwrap();
        mgr.write_file("agent-1", "backend/auth.md", "OAuth2 PKCE token exchange")
            .unwrap();
        mgr.write_file("agent-1", "frontend/ui.md", "React and Tailwind widgets")
            .unwrap();

        // List all
        let all = mgr.list_files_by_prefix("agent-1", None).unwrap();
        assert_eq!(all.len(), 3);

        // Search contents
        let search_res = mgr.search_contents("agent-1", "OAuth2").unwrap();
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].virtual_path, "backend/auth.md");
        assert!(search_res[0].snippet.contains("PKCE"));

        // Delete
        assert!(mgr.delete_file("agent-1", "frontend/ui.md").unwrap());
        let remaining = mgr.list_files_by_prefix("agent-1", None).unwrap();
        assert_eq!(remaining.len(), 2);
    }
}
