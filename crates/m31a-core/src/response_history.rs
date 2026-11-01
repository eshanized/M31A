//! Conversation response history tail slicing, assistant token budgeting, and tool error classification.
//! Manages conversation history tail truncation, token budgeting, and tool error classification.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error returned while executing a model-visible tool invocation.
/// Error returned while executing a model-visible tool invocation.
#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum FunctionCallError {
    /// Safe, informative error returned to the model as tool output to permit self-correction.
    #[error("{0}")]
    RespondToModel(String),

    /// Fatal security, policy, or harness error terminating turn execution.
    #[error("Fatal error: {0}")]
    Fatal(String),
}

/// Generic response history item for sliding-window and token budget processing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseHistoryItem {
    pub role: String,
    pub content: String,
    pub tool_call_id: Option<String>,
}

impl ResponseHistoryItem {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            tool_call_id: None,
        }
    }

    pub fn with_tool_id(mut self, tool_id: impl Into<String>) -> Self {
        self.tool_call_id = Some(tool_id.into());
        self
    }

    pub fn is_user_message(&self) -> bool {
        self.role == "user"
    }

    pub fn is_assistant_message(&self) -> bool {
        self.role == "assistant"
    }

    pub fn estimated_tokens(&self) -> usize {
        (self.content.len() / 4).max(1)
    }
}

/// Retains conversation history items from the earliest of the last `user_message_count`
/// user messages through the latest user message.
///
/// Discards orphan trailing assistant/tool items after the latest user message, and drains
/// all history prior to the sliding user window.
/// Retains only the recent history window starting from the last N user messages.
pub fn retain_tail_from_last_n_user_messages(
    items: &mut Vec<ResponseHistoryItem>,
    user_message_count: usize,
) {
    if user_message_count == 0 {
        items.clear();
        return;
    }

    let Some(latest_user_idx) = items.iter().rposition(ResponseHistoryItem::is_user_message) else {
        items.clear();
        return;
    };

    // Purge any trailing items past the latest user message
    items.truncate(latest_user_idx + 1);

    // Locate the start index for the last N user messages
    let earliest_retained_user_idx = items
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, item)| item.is_user_message())
        .take(user_message_count)
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(latest_user_idx);

    items.drain(..earliest_retained_user_idx);
}

/// Truncates assistant output text across items to enforce a total token budget.
///
/// Assistant messages are checked in order. If an assistant item exceeds the remaining
/// token allowance, it is truncated in-place to fit. Subsequent assistant items are omitted.
/// Truncates assistant text responses in-place to fit within allocated token limits.
pub fn truncate_assistant_output_text_to_token_budget(
    items: &mut Vec<ResponseHistoryItem>,
    max_tokens: usize,
) {
    let mut remaining_tokens = max_tokens;

    items.retain_mut(|item| {
        if !item.is_assistant_message() {
            return true;
        }

        if remaining_tokens == 0 {
            return false;
        }

        let item_tokens = item.estimated_tokens();
        if item_tokens <= remaining_tokens {
            remaining_tokens = remaining_tokens.saturating_sub(item_tokens);
            true
        } else {
            // Truncate content to fit remaining token budget (approx 4 bytes/chars per token)
            let max_chars = remaining_tokens * 4;
            let ceiling = max_chars.min(item.content.len());
            let mut end = ceiling;
            while end > 0 && !item.content.is_char_boundary(end) {
                end -= 1;
            }

            item.content.truncate(end);
            item.content
                .push_str("\n... [truncated due to token budget]");
            remaining_tokens = 0;
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, text: &str) -> ResponseHistoryItem {
        ResponseHistoryItem::new(role, text)
    }

    #[test]
    fn test_retain_tail_from_last_n_user_messages() {
        let mut items = vec![
            msg("system", "System instructions"),
            msg("user", "User turn 1"),
            msg("assistant", "Assistant turn 1"),
            msg("user", "User turn 2"),
            msg("assistant", "Assistant turn 2"),
            msg("user", "User turn 3"),
            msg("assistant", "Trailing unconfirmed response"),
        ];

        // Retain last 2 user messages: should keep from "User turn 2" to "User turn 3",
        // dropping system + turn 1, and dropping trailing assistant after user turn 3.
        retain_tail_from_last_n_user_messages(&mut items, 2);

        assert_eq!(
            items,
            vec![
                msg("user", "User turn 2"),
                msg("assistant", "Assistant turn 2"),
                msg("user", "User turn 3"),
            ]
        );
    }

    #[test]
    fn test_retain_tail_zero_or_no_users() {
        let mut items = vec![msg("system", "init"), msg("user", "hello")];
        retain_tail_from_last_n_user_messages(&mut items, 0);
        assert!(items.is_empty());

        let mut no_users = vec![msg("system", "init"), msg("assistant", "hello")];
        retain_tail_from_last_n_user_messages(&mut no_users, 2);
        assert!(no_users.is_empty());
    }

    #[test]
    fn test_truncate_assistant_output_text_to_token_budget() {
        let mut items = vec![
            msg("user", "Please write code"),
            msg(
                "assistant",
                &"fn test() {\n    println!(\"Hello\");\n}\n".repeat(50),
            ), // ~350 tokens
            msg("user", "Next request"),
            msg("assistant", "Second assistant reply that should be dropped"),
        ];

        // Budget of 50 tokens (~200 chars)
        truncate_assistant_output_text_to_token_budget(&mut items, 50);

        // First user message preserved
        assert_eq!(items[0].content, "Please write code");
        // First assistant message truncated
        assert!(items[1].content.contains("[truncated due to token budget]"));
        // Second user message preserved
        assert_eq!(items[2].content, "Next request");
        // Second assistant message dropped because budget was exhausted
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_function_call_error_variants() {
        let respond = FunctionCallError::RespondToModel("File not found: test.rs".to_string());
        assert_eq!(respond.to_string(), "File not found: test.rs");

        let fatal = FunctionCallError::Fatal("Process memory corruption".to_string());
        assert_eq!(fatal.to_string(), "Fatal error: Process memory corruption");
    }
}
