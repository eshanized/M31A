//! In-memory conversation transcript items, status notices, and tool progress representation.
//!
//! Models conversational items, thought chunks, file diff previews, and tool execution status items.

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub enum NoticeLevel {
    Info,
    Warning,
    Error,
    Success,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCallEntry {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub output: Option<String>,
    pub success: Option<bool>,
    pub in_progress: bool,
    pub collapsed: bool,
    pub duration_ms: Option<u64>,
}

impl ToolCallEntry {
    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptItem {
    User {
        text: String,
        timestamp: DateTime<Utc>,
    },
    Thinking {
        text: String,
        collapsed: bool,
    },
    Assistant {
        text: String,
    },
    ToolCall(ToolCallEntry),
    Notice {
        text: String,
        level: NoticeLevel,
    },
}

impl TranscriptItem {
    pub fn user(text: impl Into<String>) -> Self {
        Self::User {
            text: text.into(),
            timestamp: Utc::now(),
        }
    }

    pub fn thinking(text: impl Into<String>) -> Self {
        Self::Thinking {
            text: text.into(),
            collapsed: false,
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self::Assistant { text: text.into() }
    }

    pub fn notice(text: impl Into<String>, level: NoticeLevel) -> Self {
        Self::Notice {
            text: text.into(),
            level,
        }
    }

    pub fn tool_call(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self::ToolCall(ToolCallEntry {
            id: id.into(),
            name: name.into(),
            arguments: arguments.into(),
            output: None,
            success: None,
            in_progress: true,
            collapsed: false,
            duration_ms: None,
        })
    }
}
