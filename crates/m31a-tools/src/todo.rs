//! Task list manager (`todowrite`) tool for structured progress tracking.
//!
//! Provides the `todowrite` tool: allows agents to track multi-step goals with
//! real-time status transitions (`pending`, `in_progress`, `completed`, `cancelled`).

use anyhow::Result;
use serde::{Deserialize, Serialize};
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

/// Global in-memory storage for active session todo items.
static CURRENT_TODOS: RwLock<Vec<TodoItem>> = RwLock::const_new(Vec::new());

/// Retrieves a clone of the current session todo items.
pub async fn get_current_todos() -> Vec<TodoItem> {
    let lock = CURRENT_TODOS.read().await;
    lock.clone()
}

/// Returns a concise badge string (e.g. "1/3 done" or "1/3 done (1 active)") if any todos exist, non-blocking.
pub fn get_todo_badge() -> Option<String> {
    if let Ok(lock) = CURRENT_TODOS.try_read() {
        if lock.is_empty() {
            None
        } else {
            let total = lock.len();
            let completed = lock
                .iter()
                .filter(|t| t.status == TodoStatus::Completed)
                .count();
            let in_prog = lock
                .iter()
                .filter(|t| t.status == TodoStatus::InProgress)
                .count();
            if in_prog > 0 {
                Some(format!("{}/{} done ({} active)", completed, total, in_prog))
            } else {
                Some(format!("{}/{} done", completed, total))
            }
        }
    } else {
        None
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

/// Executes the `todowrite` tool.
pub async fn execute_todowrite(args: &TodoArgs) -> Result<TodoResult> {
    let mut lock = CURRENT_TODOS.write().await;
    *lock = args.todos.clone();

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
}
