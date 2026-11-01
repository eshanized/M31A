//! Multi-Format Transcript Exporter for M31A Agent Sessions.
//!
//! Translates stored thread turns, items (user prompts, thoughts/reasoning, assistant
//! responses, and tool calls/outputs) into clean, standalone Markdown documents,
//! streaming JSONL envelopes, and self-contained HTML reports with modern responsive styling.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::db::{ThreadRecord, TurnRecord};

/// Individual transcript item representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptItemRecord {
    pub id: String,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub item_type: String,
    pub content: String,
    pub created_at: i64,
}

/// A logical grouping of a turn and its associated items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptTurnGroup {
    pub turn: Option<TurnRecord>,
    pub items: Vec<TranscriptItemRecord>,
}

/// Multi-format session transcript exporter.
#[derive(Debug, Clone)]
pub struct TranscriptExporter {
    conn: Arc<Mutex<Connection>>,
}

impl TranscriptExporter {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Retrieve raw thread metadata, turns, and items from SQLite.
    pub fn fetch_transcript_data(
        &self,
        thread_id: &str,
    ) -> Result<(ThreadRecord, Vec<TranscriptTurnGroup>)> {
        let conn = self.conn.lock().unwrap();

        // 1. Fetch thread metadata
        let mut thread_stmt = conn.prepare(
            r#"
            SELECT id, title, model_provider, cwd, sandbox_policy, approval_mode, is_archived, parent_thread_id, section_id, created_at, updated_at
            FROM threads
            WHERE id = ?1
            "#,
        )?;

        let thread: ThreadRecord = thread_stmt
            .query_row(params![thread_id], |row| {
                Ok(ThreadRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    model: row.get(2)?,
                    cwd: row.get(3)?,
                    sandbox_policy: row.get(4)?,
                    approval_mode: row.get(5)?,
                    is_archived: row.get::<_, i64>(6)? != 0,
                    parent_thread_id: row.get(7)?,
                    section_id: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .with_context(|| format!("Thread '{}' not found", thread_id))?;

        // 2. Fetch turns
        let mut turn_stmt = conn.prepare(
            r#"
            SELECT id, thread_id, turn_index, status, created_at
            FROM thread_turns
            WHERE thread_id = ?1
            ORDER BY turn_index ASC
            "#,
        )?;
        let turn_rows = turn_stmt.query_map(params![thread_id], |row| {
            Ok(TurnRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                turn_index: row.get::<_, i64>(2)? as usize,
                status: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut turns = Vec::new();
        for r in turn_rows {
            turns.push(r?);
        }

        // 3. Fetch items
        let mut item_stmt = conn.prepare(
            r#"
            SELECT id, thread_id, turn_id, item_type, content, created_at
            FROM thread_items
            WHERE thread_id = ?1
            ORDER BY created_at ASC
            "#,
        )?;
        let item_rows = item_stmt.query_map(params![thread_id], |row| {
            Ok(TranscriptItemRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                turn_id: row.get(2)?,
                item_type: row.get(3)?,
                content: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        let mut items = Vec::new();
        for r in item_rows {
            items.push(r?);
        }

        // Group items by turn
        let mut groups = Vec::new();
        if turns.is_empty() {
            // Group all items together under an empty turn
            groups.push(TranscriptTurnGroup { turn: None, items });
        } else {
            for t in turns {
                let turn_id = t.id.clone();
                let matching_items: Vec<TranscriptItemRecord> = items
                    .iter()
                    .filter(|item| item.turn_id.as_deref() == Some(&turn_id))
                    .cloned()
                    .collect();
                groups.push(TranscriptTurnGroup {
                    turn: Some(t),
                    items: matching_items,
                });
            }

            // Also check for any unassigned items (turn_id is None or not matched)
            let unassigned: Vec<TranscriptItemRecord> = items
                .into_iter()
                .filter(|item| {
                    item.turn_id.is_none()
                        || !groups
                            .iter()
                            .any(|g| g.turn.as_ref().map(|t| &t.id) == item.turn_id.as_ref())
                })
                .collect();
            if !unassigned.is_empty() {
                groups.push(TranscriptTurnGroup {
                    turn: None,
                    items: unassigned,
                });
            }
        }

        Ok((thread, groups))
    }

    /// Export the session transcript as a clean, formatted Markdown document.
    pub fn export_to_markdown(&self, thread_id: &str) -> Result<String> {
        let (thread, groups) = self.fetch_transcript_data(thread_id)?;
        Self::render_markdown(&thread, &groups)
    }

    /// Render Markdown from given thread record and turn groups.
    pub fn render_markdown(
        thread: &ThreadRecord,
        groups: &[TranscriptTurnGroup],
    ) -> Result<String> {
        let mut out = String::new();
        let created_dt = DateTime::from_timestamp(thread.created_at, 0).unwrap_or_else(Utc::now);

        out.push_str(&format!("# M31A Session: {}\n\n", thread.title));
        out.push_str("### Session Metadata\n\n");
        out.push_str(&format!("- **Thread ID**: `{}`\n", thread.id));
        out.push_str(&format!("- **Model**: `{}`\n", thread.model));
        out.push_str(&format!("- **Workspace**: `{}`\n", thread.cwd));
        out.push_str(&format!(
            "- **Sandbox Policy**: `{}`\n",
            thread.sandbox_policy
        ));
        out.push_str(&format!(
            "- **Approval Mode**: `{}`\n",
            thread.approval_mode
        ));
        out.push_str(&format!("- **Created**: `{}`\n\n", created_dt.to_rfc3339()));
        out.push_str("---\n\n");

        for (idx, group) in groups.iter().enumerate() {
            let turn_label = match &group.turn {
                Some(t) => format!("Turn #{} ({})", t.turn_index, t.status),
                None => format!("Phase {}", idx + 1),
            };
            out.push_str(&format!("## {}\n\n", turn_label));

            for item in &group.items {
                match item.item_type.as_str() {
                    "user_message" | "user" => {
                        out.push_str("### User\n\n");
                        out.push_str(&item.content);
                        out.push_str("\n\n");
                    }
                    "reasoning" | "thought" => {
                        out.push_str("<details>\n<summary><em>Thought Process</em></summary>\n\n");
                        out.push_str(&item.content);
                        out.push_str("\n\n</details>\n\n");
                    }
                    "assistant_message" | "assistant" => {
                        out.push_str("### M31A Agent\n\n");
                        out.push_str(&item.content);
                        out.push_str("\n\n");
                    }
                    "tool_call" => {
                        out.push_str("#### Tool Invocation\n\n");
                        out.push_str("```json\n");
                        out.push_str(&item.content);
                        out.push_str("\n```\n\n");
                    }
                    "tool_output" | "tool_return" => {
                        out.push_str("#### Tool Output\n\n");
                        out.push_str("```\n");
                        out.push_str(&item.content);
                        out.push_str("\n```\n\n");
                    }
                    _ => {
                        out.push_str(&format!("> **[{}]**: {}\n\n", item.item_type, item.content));
                    }
                }
            }
            out.push_str("---\n\n");
        }

        Ok(out)
    }

    /// Export the session transcript as a streaming JSONL envelope.
    pub fn export_to_jsonl(&self, thread_id: &str) -> Result<String> {
        let (thread, groups) = self.fetch_transcript_data(thread_id)?;
        Self::render_jsonl(&thread, &groups)
    }

    /// Render JSONL lines from given thread record and turn groups.
    pub fn render_jsonl(thread: &ThreadRecord, groups: &[TranscriptTurnGroup]) -> Result<String> {
        let mut lines = Vec::new();

        // 1. Thread Header line
        let meta_line = serde_json::json!({
            "type": "thread_metadata",
            "thread_id": thread.id,
            "title": thread.title,
            "model": thread.model,
            "cwd": thread.cwd,
            "sandbox_policy": thread.sandbox_policy,
            "approval_mode": thread.approval_mode,
            "created_at": thread.created_at,
            "updated_at": thread.updated_at,
        });
        lines.push(serde_json::to_string(&meta_line)?);

        // 2. Turns and Items
        for group in groups {
            if let Some(turn) = &group.turn {
                let turn_line = serde_json::json!({
                    "type": "turn_start",
                    "turn_id": turn.id,
                    "turn_index": turn.turn_index,
                    "status": turn.status,
                    "created_at": turn.created_at,
                });
                lines.push(serde_json::to_string(&turn_line)?);
            }

            for item in &group.items {
                let item_line = serde_json::json!({
                    "type": "item",
                    "item_id": item.id,
                    "turn_id": item.turn_id,
                    "item_type": item.item_type,
                    "content": item.content,
                    "created_at": item.created_at,
                });
                lines.push(serde_json::to_string(&item_line)?);
            }

            if let Some(turn) = &group.turn {
                let turn_end_line = serde_json::json!({
                    "type": "turn_end",
                    "turn_id": turn.id,
                    "turn_index": turn.turn_index,
                    "status": turn.status,
                });
                lines.push(serde_json::to_string(&turn_end_line)?);
            }
        }

        Ok(lines.join("\n") + "\n")
    }

    /// Export the session transcript as a self-contained responsive HTML report.
    pub fn export_to_html(&self, thread_id: &str) -> Result<String> {
        let (thread, groups) = self.fetch_transcript_data(thread_id)?;
        Self::render_html(&thread, &groups)
    }

    /// Render self-contained HTML from thread record and turn groups.
    pub fn render_html(thread: &ThreadRecord, groups: &[TranscriptTurnGroup]) -> Result<String> {
        let title_escaped = html_escape(&thread.title);
        let mut html = String::new();

        html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html.push_str("<meta charset=\"UTF-8\">\n");
        html.push_str(
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
        );
        html.push_str(&format!(
            "<title>M31A Session - {}</title>\n",
            title_escaped
        ));
        html.push_str("<style>\n");
        html.push_str(
            r#"
            :root {
                --bg: #0d1117;
                --surface: #161b22;
                --border: #30363d;
                --text: #c9d1d9;
                --text-muted: #8b949e;
                --primary: #58a6ff;
                --accent: #238636;
                --warn: #d29922;
                --card-bg: #1c2128;
                --code-bg: #090d13;
            }
            * { box-sizing: border-box; margin: 0; padding: 0; }
            body {
                background: var(--bg);
                color: var(--text);
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
                line-height: 1.6;
                padding: 2rem;
            }
            .container { max-width: 900px; margin: 0 auto; }
            header {
                background: var(--surface);
                border: 1px solid var(--border);
                border-radius: 8px;
                padding: 1.5rem;
                margin-bottom: 2rem;
            }
            h1 { font-size: 1.8rem; color: #f0f6fc; margin-bottom: 0.5rem; }
            .badge-bar { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 0.75rem; }
            .badge {
                background: #21262d;
                border: 1px solid var(--border);
                padding: 0.2rem 0.6rem;
                border-radius: 4px;
                font-size: 0.85rem;
                color: var(--text-muted);
            }
            .badge strong { color: var(--text); }
            .turn-card {
                background: var(--surface);
                border: 1px solid var(--border);
                border-radius: 8px;
                margin-bottom: 1.5rem;
                overflow: hidden;
            }
            .turn-header {
                background: #21262d;
                padding: 0.75rem 1.25rem;
                font-weight: 600;
                border-bottom: 1px solid var(--border);
                color: #f0f6fc;
                display: flex;
                justify-content: space-between;
            }
            .turn-body { padding: 1.25rem; display: flex; flex-direction: column; gap: 1rem; }
            .msg {
                border-radius: 6px;
                padding: 1rem;
            }
            .msg-user {
                background: #1f2937;
                border-left: 4px solid var(--primary);
            }
            .msg-assistant {
                background: var(--card-bg);
                border-left: 4px solid var(--accent);
            }
            .msg-reasoning {
                background: #1b2028;
                border-left: 4px solid var(--warn);
                font-size: 0.92rem;
            }
            .msg-reasoning summary {
                cursor: pointer;
                font-weight: 600;
                color: var(--warn);
                outline: none;
            }
            .msg-tool {
                background: var(--code-bg);
                border: 1px solid var(--border);
                border-radius: 6px;
                font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
                font-size: 0.88rem;
                overflow-x: auto;
            }
            .tool-header {
                background: #1a202c;
                padding: 0.4rem 0.8rem;
                border-bottom: 1px solid var(--border);
                font-weight: 600;
                color: var(--text-muted);
            }
            pre { padding: 0.8rem; overflow-x: auto; white-space: pre-wrap; word-break: break-word; }
            code { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; }
            "#,
        );
        html.push_str("</style>\n</head>\n<body>\n<div class=\"container\">\n");

        // Header
        html.push_str("<header>\n");
        html.push_str(&format!("  <h1>{}</h1>\n", title_escaped));
        html.push_str("  <div class=\"badge-bar\">\n");
        html.push_str(&format!(
            "    <span class=\"badge\">ID: <strong>{}</strong></span>\n",
            html_escape(&thread.id)
        ));
        html.push_str(&format!(
            "    <span class=\"badge\">Model: <strong>{}</strong></span>\n",
            html_escape(&thread.model)
        ));
        html.push_str(&format!(
            "    <span class=\"badge\">Sandbox: <strong>{}</strong></span>\n",
            html_escape(&thread.sandbox_policy)
        ));
        html.push_str(&format!(
            "    <span class=\"badge\">Approval: <strong>{}</strong></span>\n",
            html_escape(&thread.approval_mode)
        ));
        html.push_str(&format!(
            "    <span class=\"badge\">CWD: <strong>{}</strong></span>\n",
            html_escape(&thread.cwd)
        ));
        html.push_str("  </div>\n");
        html.push_str("</header>\n\n");

        // Turns
        for (idx, group) in groups.iter().enumerate() {
            html.push_str("<div class=\"turn-card\">\n");
            let turn_label = match &group.turn {
                Some(t) => format!("Turn #{} &bull; Status: {}", t.turn_index, t.status),
                None => format!("Phase {}", idx + 1),
            };
            html.push_str(&format!(
                "  <div class=\"turn-header\"><span>{}</span></div>\n",
                turn_label
            ));
            html.push_str("  <div class=\"turn-body\">\n");

            for item in &group.items {
                let content_escaped = html_escape(&item.content);
                match item.item_type.as_str() {
                    "user_message" | "user" => {
                        html.push_str("    <div class=\"msg msg-user\">\n");
                        html.push_str("      <strong>User:</strong><br>\n");
                        html.push_str(&format!("      <p>{}</p>\n", content_escaped));
                        html.push_str("    </div>\n");
                    }
                    "reasoning" | "thought" => {
                        html.push_str("    <details class=\"msg msg-reasoning\">\n");
                        html.push_str("      <summary>Thought Process</summary>\n");
                        html.push_str(&format!(
                            "      <p style=\"margin-top: 0.5rem;\">{}</p>\n",
                            content_escaped
                        ));
                        html.push_str("    </details>\n");
                    }
                    "assistant_message" | "assistant" => {
                        html.push_str("    <div class=\"msg msg-assistant\">\n");
                        html.push_str("      <strong>M31A Agent:</strong><br>\n");
                        html.push_str(&format!("      <p>{}</p>\n", content_escaped));
                        html.push_str("    </div>\n");
                    }
                    "tool_call" => {
                        html.push_str("    <div class=\"msg-tool\">\n");
                        html.push_str("      <div class=\"tool-header\">Tool Call</div>\n");
                        html.push_str(&format!(
                            "      <pre><code>{}</code></pre>\n",
                            content_escaped
                        ));
                        html.push_str("    </div>\n");
                    }
                    "tool_output" | "tool_return" => {
                        html.push_str("    <div class=\"msg-tool\">\n");
                        html.push_str("      <div class=\"tool-header\">Tool Output</div>\n");
                        html.push_str(&format!(
                            "      <pre><code>{}</code></pre>\n",
                            content_escaped
                        ));
                        html.push_str("    </div>\n");
                    }
                    _ => {
                        html.push_str(&format!(
                            "    <div class=\"msg\"><small><strong>[{}]:</strong></small> {}</div>\n",
                            html_escape(&item.item_type),
                            content_escaped
                        ));
                    }
                }
            }

            html.push_str("  </div>\n");
            html.push_str("</div>\n\n");
        }

        html.push_str("</div>\n</body>\n</html>\n");
        Ok(html)
    }
}

/// Simple HTML sanitizer for text rendering.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::db::Storage;
    use chrono::Utc;
    use m31a_core::types::SessionMetadata;

    #[test]
    fn test_transcript_export_markdown_jsonl_html() {
        let storage = Storage::open_in_memory().unwrap();
        let meta = SessionMetadata {
            id: "thread-export-test".to_string(),
            title: "Test Session".to_string(),
            model: "test-model".to_string(),
            cwd: "/tmp".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        let thread_id = meta.id.clone();

        storage
            .save_thread(&meta, "/tmp/rollout", "read-only", "on-failure")
            .unwrap();

        let turn_id = "turn-001";
        storage
            .save_turn(turn_id, &thread_id, 0, "completed")
            .unwrap();

        storage
            .save_item(
                "item-1",
                &thread_id,
                Some(turn_id),
                "user_message",
                "Refactor the parser",
            )
            .unwrap();
        storage
            .save_item(
                "item-2",
                &thread_id,
                Some(turn_id),
                "reasoning",
                "Analyzing abstract syntax tree...",
            )
            .unwrap();
        storage
            .save_item(
                "item-3",
                &thread_id,
                Some(turn_id),
                "assistant_message",
                "I have refactored the parser.",
            )
            .unwrap();

        let exporter = storage.transcript_exporter();

        // 1. Markdown Export
        let md = exporter.export_to_markdown(&thread_id).unwrap();
        assert!(md.contains("# M31A Session:"));
        assert!(md.contains("### User"));
        assert!(md.contains("Refactor the parser"));
        assert!(md.contains("<em>Thought Process</em>"));
        assert!(md.contains("### M31A Agent"));

        // 2. JSONL Export
        let jsonl = exporter.export_to_jsonl(&thread_id).unwrap();
        let lines: Vec<&str> = jsonl.lines().collect();
        assert!(lines.len() >= 4); // metadata + turn_start + items + turn_end
        let parsed_meta: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed_meta["type"], "thread_metadata");
        assert_eq!(parsed_meta["thread_id"], thread_id);

        // 3. HTML Export
        let html = exporter.export_to_html(&thread_id).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Refactor the parser"));
        assert!(html.contains("Thought Process"));
        assert!(html.contains("M31A Agent"));
    }
}
