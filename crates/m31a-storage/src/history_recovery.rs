//! Context window recovery and history windows reader.
//! Context recovery reader retrieving historic turns and message fragments across session restarts.

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Metadata regarding a recorded context window.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryWindowInfo {
    pub window_id: String,
    pub agent_name: String,
    pub created_at: String,
    pub item_count: usize,
}

/// A single recorded history item.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryItemInfo {
    pub item_id: String,
    pub window_id: String,
    pub agent_name: String,
    pub role: String,
    pub tool_namespace: Option<String>,
    pub tool_name: Option<String>,
    pub truncated_content: String,
    pub total_chars: usize,
    pub ordinal: i64,
    pub created_at: String,
}

/// Parameter filters for listing history items.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HistoryListItemsQuery {
    pub agent_name: Option<String>,
    pub window_id: Option<String>,
    pub role: Option<String>,
    pub tool_namespace: Option<String>,
    pub tool_name: Option<String>,
    pub limit: Option<usize>,
    pub recent_first: bool,
    pub max_chars_per_item: Option<usize>,
}

/// Result of reading a specific bounded character range of an item.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryReadResult {
    pub item_id: String,
    pub window_id: String,
    pub agent_name: String,
    pub role: String,
    pub total_chars: usize,
    pub offset_chars: usize,
    pub content: String,
    pub has_more: bool,
}

/// Query parameters for searching history contents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistorySearchQuery {
    pub query: String,
    pub agent_name: Option<String>,
    pub window_id: Option<String>,
    pub role: Option<String>,
    pub tool_namespace: Option<String>,
    pub tool_name: Option<String>,
    pub limit: Option<usize>,
    pub recent_first: bool,
}

/// Search match returned from history contents search.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistorySearchResult {
    pub item_id: String,
    pub window_id: String,
    pub agent_name: String,
    pub role: String,
    pub match_snippet: String,
    pub line_number: usize,
    pub created_at: String,
}

/// Persistent store managing context windows and history items.
#[derive(Clone)]
pub struct HistoryRecoveryStore {
    conn: Arc<Mutex<Connection>>,
}

impl HistoryRecoveryStore {
    /// Creates a store with tables initialized on the given connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> anyhow::Result<Self> {
        let store = Self { conn };
        store.init_tables()?;
        Ok(store)
    }

    /// Creates an in-memory history recovery store for testing.
    pub fn in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::new(Arc::new(Mutex::new(conn)))
    }

    fn init_tables(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS history_windows (
                window_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                agent_name TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS history_items (
                item_id TEXT PRIMARY KEY,
                window_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                agent_name TEXT NOT NULL,
                role TEXT NOT NULL,
                tool_namespace TEXT,
                tool_name TEXT,
                content TEXT NOT NULL,
                ordinal INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_hist_win_session ON history_windows(session_id, agent_name);
            CREATE INDEX IF NOT EXISTS idx_hist_item_win ON history_items(window_id, ordinal);
            CREATE INDEX IF NOT EXISTS idx_hist_item_session ON history_items(session_id, agent_name);
            "#,
        )?;
        Ok(())
    }

    /// Records a new or existing context window.
    pub fn record_window(
        &self,
        session_id: &str,
        agent_name: &str,
        window_id: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            r#"
            INSERT INTO history_windows (window_id, session_id, agent_name, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(window_id) DO NOTHING
            "#,
            params![window_id, session_id, agent_name, now],
        )?;
        Ok(())
    }

    /// Records an item into a context window.
    #[allow(clippy::too_many_arguments)]
    pub fn record_item(
        &self,
        session_id: &str,
        agent_name: &str,
        window_id: &str,
        item_id: &str,
        role: &str,
        tool_namespace: Option<&str>,
        tool_name: Option<&str>,
        content: &str,
        ordinal: i64,
    ) -> anyhow::Result<()> {
        self.record_window(session_id, agent_name, window_id)?;
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            r#"
            INSERT INTO history_items (
                item_id, window_id, session_id, agent_name, role,
                tool_namespace, tool_name, content, ordinal, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(item_id) DO UPDATE SET
                content = excluded.content,
                role = excluded.role,
                tool_namespace = excluded.tool_namespace,
                tool_name = excluded.tool_name
            "#,
            params![
                item_id,
                window_id,
                session_id,
                agent_name,
                role,
                tool_namespace,
                tool_name,
                content,
                ordinal,
                now,
            ],
        )?;
        Ok(())
    }

    /// Lists context windows for a session and optional agent.
    pub fn list_windows(
        &self,
        session_id: &str,
        agent_name: Option<&str>,
        limit: Option<usize>,
        recent_first: bool,
    ) -> anyhow::Result<Vec<HistoryWindowInfo>> {
        let conn = self.conn.lock().unwrap();
        let order = if recent_first { "DESC" } else { "ASC" };
        let sql = format!(
            r#"
            SELECT w.window_id, w.agent_name, w.created_at, COUNT(i.item_id) as item_count
            FROM history_windows w
            LEFT JOIN history_items i ON w.window_id = i.window_id
            WHERE w.session_id = ?1
              AND (?2 IS NULL OR w.agent_name = ?2)
            GROUP BY w.window_id
            ORDER BY w.created_at {order}
            LIMIT ?3
            "#
        );

        let lim = limit.unwrap_or(50) as i64;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![session_id, agent_name, lim], |row| {
            Ok(HistoryWindowInfo {
                window_id: row.get(0)?,
                agent_name: row.get(1)?,
                created_at: row.get(2)?,
                item_count: row.get(3)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Lists items across or within context windows matching query filters.
    pub fn list_items(
        &self,
        session_id: &str,
        query: &HistoryListItemsQuery,
    ) -> anyhow::Result<Vec<HistoryItemInfo>> {
        let conn = self.conn.lock().unwrap();
        let order = if query.recent_first { "DESC" } else { "ASC" };
        let sql = format!(
            r#"
            SELECT item_id, window_id, agent_name, role, tool_namespace, tool_name, content, ordinal, created_at
            FROM history_items
            WHERE session_id = ?1
              AND (?2 IS NULL OR agent_name = ?2)
              AND (?3 IS NULL OR window_id = ?3)
              AND (?4 IS NULL OR role = ?4)
              AND (?5 IS NULL OR tool_namespace = ?5)
              AND (?6 IS NULL OR tool_name = ?6)
            ORDER BY ordinal {order}, created_at {order}
            LIMIT ?7
            "#
        );

        let lim = query.limit.unwrap_or(100) as i64;
        let max_chars = query.max_chars_per_item.unwrap_or(500);

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![
                session_id,
                query.agent_name,
                query.window_id,
                query.role,
                query.tool_namespace,
                query.tool_name,
                lim,
            ],
            |row| {
                let item_id: String = row.get(0)?;
                let window_id: String = row.get(1)?;
                let agent_name: String = row.get(2)?;
                let role: String = row.get(3)?;
                let tool_namespace: Option<String> = row.get(4)?;
                let tool_name: Option<String> = row.get(5)?;
                let full_content: String = row.get(6)?;
                let ordinal: i64 = row.get(7)?;
                let created_at: String = row.get(8)?;

                let total_chars = full_content.chars().count();
                let truncated_content: String = if total_chars > max_chars {
                    let mut s: String = full_content.chars().take(max_chars).collect();
                    s.push_str("... [truncated]");
                    s
                } else {
                    full_content
                };

                Ok(HistoryItemInfo {
                    item_id,
                    window_id,
                    agent_name,
                    role,
                    tool_namespace,
                    tool_name,
                    truncated_content,
                    total_chars,
                    ordinal,
                    created_at,
                })
            },
        )?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Reads a bounded UTF-8 character range from a specific history item.
    pub fn read_item(
        &self,
        session_id: &str,
        window_id: &str,
        item_id: &str,
        offset_chars: Option<usize>,
        limit_chars: Option<usize>,
    ) -> anyhow::Result<HistoryReadResult> {
        let conn = self.conn.lock().unwrap();
        let sql = r#"
            SELECT agent_name, role, content
            FROM history_items
            WHERE session_id = ?1 AND window_id = ?2 AND item_id = ?3
        "#;

        let row = conn
            .query_row(sql, params![session_id, window_id, item_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .optional()?;

        let (agent_name, role, content) = match row {
            Some(data) => data,
            None => anyhow::bail!("Item '{item_id}' not found in window '{window_id}'"),
        };

        let total_chars = content.chars().count();
        let offset = offset_chars.unwrap_or(0);
        let limit = limit_chars.unwrap_or(2000);

        let slice: String = content.chars().skip(offset).take(limit).collect();
        let read_chars = slice.chars().count();
        let has_more = offset + read_chars < total_chars;

        Ok(HistoryReadResult {
            item_id: item_id.to_string(),
            window_id: window_id.to_string(),
            agent_name,
            role,
            total_chars,
            offset_chars: offset,
            content: slice,
            has_more,
        })
    }

    /// Searches history items by literal case-sensitive substring.
    pub fn search_contents(
        &self,
        session_id: &str,
        query: &HistorySearchQuery,
    ) -> anyhow::Result<Vec<HistorySearchResult>> {
        let conn = self.conn.lock().unwrap();
        let order = if query.recent_first { "DESC" } else { "ASC" };
        let sql = format!(
            r#"
            SELECT item_id, window_id, agent_name, role, content, created_at
            FROM history_items
            WHERE session_id = ?1
              AND (?2 IS NULL OR agent_name = ?2)
              AND (?3 IS NULL OR window_id = ?3)
              AND (?4 IS NULL OR role = ?4)
              AND (?5 IS NULL OR tool_namespace = ?5)
              AND (?6 IS NULL OR tool_name = ?6)
              AND instr(content, ?7) > 0
            ORDER BY ordinal {order}, created_at {order}
            LIMIT ?8
            "#
        );

        let lim = query.limit.unwrap_or(50) as i64;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![
                session_id,
                query.agent_name,
                query.window_id,
                query.role,
                query.tool_namespace,
                query.tool_name,
                query.query,
                lim,
            ],
            |row| {
                let item_id: String = row.get(0)?;
                let window_id: String = row.get(1)?;
                let agent_name: String = row.get(2)?;
                let role: String = row.get(3)?;
                let content: String = row.get(4)?;
                let created_at: String = row.get(5)?;

                // Find matching line and snippet
                let mut match_snippet = String::new();
                let mut match_line = 1;
                for (idx, line) in content.lines().enumerate() {
                    if line.contains(&query.query) {
                        match_snippet = line.trim().to_string();
                        match_line = idx + 1;
                        break;
                    }
                }

                Ok(HistorySearchResult {
                    item_id,
                    window_id,
                    agent_name,
                    role,
                    match_snippet,
                    line_number: match_line,
                    created_at,
                })
            },
        )?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_list_windows() {
        let store = HistoryRecoveryStore::in_memory().unwrap();
        store.record_window("sess_1", "coder", "win_001").unwrap();
        store.record_window("sess_1", "coder", "win_002").unwrap();

        let windows = store
            .list_windows("sess_1", Some("coder"), None, false)
            .unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].window_id, "win_001");
        assert_eq!(windows[1].window_id, "win_002");
    }

    #[test]
    fn test_record_items_and_bounded_read() {
        let store = HistoryRecoveryStore::in_memory().unwrap();
        let text = "Hello world! This is a long content to test UTF-8 slicing and offset reading.";
        store
            .record_item(
                "sess_1",
                "coder",
                "win_001",
                "item_101",
                "assistant",
                None,
                None,
                text,
                1,
            )
            .unwrap();

        let read1 = store
            .read_item("sess_1", "win_001", "item_101", Some(0), Some(12))
            .unwrap();
        assert_eq!(read1.content, "Hello world!");
        assert!(read1.has_more);
        assert_eq!(read1.total_chars, text.chars().count());

        let read2 = store
            .read_item("sess_1", "win_001", "item_101", Some(13), Some(100))
            .unwrap();
        assert_eq!(
            read2.content,
            "This is a long content to test UTF-8 slicing and offset reading."
        );
        assert!(!read2.has_more);
    }

    #[test]
    fn test_search_contents_across_items() {
        let store = HistoryRecoveryStore::in_memory().unwrap();
        store
            .record_item(
                "sess_1",
                "planner",
                "win_001",
                "item_1",
                "user",
                None,
                None,
                "Please configure Postgres connection string",
                1,
            )
            .unwrap();
        store
            .record_item(
                "sess_1",
                "planner",
                "win_001",
                "item_2",
                "assistant",
                None,
                None,
                "Line 1: init\nLine 2: Postgres pool configured at port 5432\nLine 3: done",
                2,
            )
            .unwrap();

        let search_query = HistorySearchQuery {
            query: "Postgres pool".to_string(),
            agent_name: None,
            window_id: None,
            role: None,
            tool_namespace: None,
            tool_name: None,
            limit: Some(10),
            recent_first: false,
        };

        let results = store.search_contents("sess_1", &search_query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "item_2");
        assert_eq!(results[0].line_number, 2);
        assert!(
            results[0]
                .match_snippet
                .contains("Postgres pool configured")
        );
    }
}
