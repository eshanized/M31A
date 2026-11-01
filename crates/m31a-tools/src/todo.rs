//! Task list manager (`todowrite`) tool for structured progress tracking.
//!
//! Provides the `todowrite` tool: allows agents to track multi-step goals with
//! real-time status transitions (`pending`, `in_progress`, `completed`, `cancelled`).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl std::fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for TodoStatus {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "in_progress" | "inprogress" => Ok(Self::InProgress),
            "completed" | "done" => Ok(Self::Completed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            _ => Ok(Self::Pending),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// Brief description of the task.
    pub content: String,
    /// Current status: pending, in_progress, completed, cancelled.
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoArgs {
    /// The updated todo list.
    pub todos: Vec<TodoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoResult {
    pub todos: Vec<TodoItem>,
    pub summary: String,
    pub completed_count: usize,
    pub total_count: usize,
}

/// Active session-scoped task store keyed by `thread_id`.
static SESSION_TODOS: LazyLock<RwLock<HashMap<String, Vec<TodoItem>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Last active session/thread identifier for fallback compatibility.
static LAST_ACTIVE_SESSION: LazyLock<RwLock<String>> =
    LazyLock::new(|| RwLock::new("default".to_string()));

/// Retrieves a clone of the session todo items for the given thread.
pub async fn get_session_todos(
    thread_id: &str,
    storage: Option<&m31a_storage::Storage>,
) -> Vec<TodoItem> {
    {
        let lock = SESSION_TODOS.read().await;
        if let Some(items) = lock.get(thread_id) {
            return items.clone();
        }
    }

    // Try loading from SQLite
    let loaded_records = match storage {
        Some(st) => st.get_session_todos(thread_id).ok(),
        None => m31a_storage::Storage::open_default()
            .ok()
            .and_then(|st| st.get_session_todos(thread_id).ok()),
    };
    if let Some(records) = loaded_records.filter(|r| !r.is_empty()) {
        let items: Vec<TodoItem> = records
            .into_iter()
            .map(|r| TodoItem {
                content: r.content,
                status: r.status.parse().unwrap_or(TodoStatus::Pending),
            })
            .collect();
        let mut lock = SESSION_TODOS.write().await;
        lock.insert(thread_id.to_string(), items.clone());
        return items;
    }

    Vec::new()
}

/// Retrieves a clone of the current/active session todo items.
pub async fn get_current_todos() -> Vec<TodoItem> {
    let session = {
        let lock = LAST_ACTIVE_SESSION.read().await;
        lock.clone()
    };
    get_session_todos(&session, None).await
}

/// Returns a concise badge string (e.g. "1/3 done" or "1/3 done (1 active)") for a specific session.
pub fn get_todo_badge_for_session(thread_id: &str) -> Option<String> {
    if let Some(badge) = SESSION_TODOS.try_read().ok().and_then(|lock| {
        lock.get(thread_id)
            .and_then(|items| format_todo_badge(items))
    }) {
        return Some(badge);
    }

    let loaded_records = m31a_storage::Storage::open_default()
        .ok()
        .and_then(|st| st.get_session_todos(thread_id).ok())
        .filter(|r| !r.is_empty());
    if let Some(records) = loaded_records {
        let items: Vec<TodoItem> = records
            .into_iter()
            .map(|r| TodoItem {
                content: r.content,
                status: r.status.parse().unwrap_or(TodoStatus::Pending),
            })
            .collect();
        return format_todo_badge(&items);
    }

    None
}

/// Returns a concise badge string for the active session, non-blocking.
pub fn get_todo_badge() -> Option<String> {
    let session = if let Ok(lock) = LAST_ACTIVE_SESSION.try_read() {
        lock.clone()
    } else {
        "default".to_string()
    };
    get_todo_badge_for_session(&session)
}

fn format_todo_badge(items: &[TodoItem]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let total = items.len();
    let completed = items
        .iter()
        .filter(|t| t.status == TodoStatus::Completed)
        .count();
    let in_prog = items
        .iter()
        .filter(|t| t.status == TodoStatus::InProgress)
        .count();
    if in_prog > 0 {
        Some(format!("{}/{} done ({} active)", completed, total, in_prog))
    } else {
        Some(format!("{}/{} done", completed, total))
    }
}

/// Summarizes a slice of todo items into a human-readable telemetry string.
pub fn summarize_todos(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return "Todos cleared.".to_string();
    }

    let total = todos.len();
    let completed = todos
        .iter()
        .filter(|t| t.status == TodoStatus::Completed)
        .count();
    let in_progress = todos
        .iter()
        .filter(|t| t.status == TodoStatus::InProgress)
        .count();
    let pending = todos
        .iter()
        .filter(|t| t.status == TodoStatus::Pending)
        .count();
    let cancelled = todos
        .iter()
        .filter(|t| t.status == TodoStatus::Cancelled)
        .count();

    let mut parts = vec![format!("{}/{} completed", completed, total)];
    if in_progress > 0 {
        parts.push(format!("{} in progress", in_progress));
    }
    if pending > 0 {
        parts.push(format!("{} pending", pending));
    }
    if cancelled > 0 {
        parts.push(format!("{} cancelled", cancelled));
    }

    let active_tasks: Vec<String> = todos
        .iter()
        .filter(|t| t.status == TodoStatus::InProgress)
        .map(|t| format!("\"{}\"", t.content))
        .collect();

    let details = if !active_tasks.is_empty() {
        format!(". In progress: {}", active_tasks.join(", "))
    } else {
        String::new()
    };

    format!("Todos updated: {}{}", parts.join(", "), details)
}

/// Executes the `todowrite` tool for a specific session, persisting to SQLite if storage is available.
pub async fn execute_todowrite_scoped(
    thread_id: &str,
    args: &TodoArgs,
    storage: Option<&m31a_storage::Storage>,
) -> Result<TodoResult> {
    {
        let mut lock = SESSION_TODOS.write().await;
        lock.insert(thread_id.to_string(), args.todos.clone());
        let mut active = LAST_ACTIVE_SESSION.write().await;
        *active = thread_id.to_string();
    }

    // Persist to SQLite if storage is available
    let now = chrono::Utc::now().timestamp_millis();
    let records: Vec<m31a_storage::SessionTodoRecord> = args
        .todos
        .iter()
        .enumerate()
        .map(|(idx, item)| m31a_storage::SessionTodoRecord {
            id: format!("{}-todo-{}", thread_id, idx),
            thread_id: thread_id.to_string(),
            content: item.content.clone(),
            status: item.status.to_string(),
            position: idx as i64,
            created_at_epoch_ms: now,
            updated_at_epoch_ms: now,
        })
        .collect();

    if let Some(st) = storage {
        let _ = st.save_session_todos(thread_id, &records);
    } else if let Ok(st) = m31a_storage::Storage::open_default() {
        let _ = st.save_session_todos(thread_id, &records);
    }

    let summary = summarize_todos(&args.todos);
    let completed_count = args
        .todos
        .iter()
        .filter(|t| t.status == TodoStatus::Completed)
        .count();
    let total_count = args.todos.len();

    Ok(TodoResult {
        todos: args.todos.clone(),
        summary,
        completed_count,
        total_count,
    })
}

/// Executes the `todowrite` tool using the active or default session.
pub async fn execute_todowrite(args: &TodoArgs) -> Result<TodoResult> {
    let session = {
        let lock = LAST_ACTIVE_SESSION.read().await;
        lock.clone()
    };
    execute_todowrite_scoped(&session, args, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_todowrite_lifecycle() {
        let items = vec![
            TodoItem {
                content: "Implement edit tool".to_string(),
                status: TodoStatus::Completed,
            },
            TodoItem {
                content: "Implement todowrite tool".to_string(),
                status: TodoStatus::InProgress,
            },
            TodoItem {
                content: "Implement schedule tool".to_string(),
                status: TodoStatus::Pending,
            },
        ];

        let res = execute_todowrite(&TodoArgs { todos: items }).await.unwrap();
        assert_eq!(res.completed_count, 1);
        assert_eq!(res.total_count, 3);
        assert!(res.summary.contains("1/3 completed"));
        assert!(
            res.summary
                .contains("In progress: \"Implement todowrite tool\"")
        );

        let current = get_current_todos().await;
        assert_eq!(current.len(), 3);
    }

    #[tokio::test]
    async fn test_session_scoped_todo_isolation() {
        let sess_a = "thread_alpha_123";
        let sess_b = "thread_beta_456";

        let items_a = vec![TodoItem {
            content: "Task A".to_string(),
            status: TodoStatus::InProgress,
        }];
        let items_b = vec![
            TodoItem {
                content: "Task B1".to_string(),
                status: TodoStatus::Completed,
            },
            TodoItem {
                content: "Task B2".to_string(),
                status: TodoStatus::Pending,
            },
        ];

        execute_todowrite_scoped(sess_a, &TodoArgs { todos: items_a }, None)
            .await
            .unwrap();
        execute_todowrite_scoped(sess_b, &TodoArgs { todos: items_b }, None)
            .await
            .unwrap();

        let todos_a = get_session_todos(sess_a, None).await;
        let todos_b = get_session_todos(sess_b, None).await;

        assert_eq!(todos_a.len(), 1);
        assert_eq!(todos_a[0].content, "Task A");

        assert_eq!(todos_b.len(), 2);
        assert_eq!(todos_b[0].content, "Task B1");
        assert_eq!(todos_b[1].content, "Task B2");

        let badge_a = get_todo_badge_for_session(sess_a);
        assert_eq!(badge_a, Some("0/1 done (1 active)".to_string()));

        let badge_b = get_todo_badge_for_session(sess_b);
        assert_eq!(badge_b, Some("1/2 done".to_string()));
    }
}
