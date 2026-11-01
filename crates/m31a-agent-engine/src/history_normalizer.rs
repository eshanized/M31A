//! Self-healing tool history normalization (inspired by OpenCode's `normalizeToolHistory`).
//!
//! Enforces structural LLM API validity guarantees:
//! 1. Synthesizes missing `tool` responses for any abandoned or interrupted tool calls.
//! 2. Replaces empty tool outputs (`""`) with `(no tool output)` to prevent provider HTTP 400 errors.
//! 3. Defers system updates so they never break the contiguous tool_call -> tool_result block required by frontier LLMs.

use m31a_core::types::{ChatMessage, Role};
use std::collections::HashSet;

pub const EMPTY_TOOL_OUTPUT: &str = "(no tool output)";
pub const MISSING_TOOL_RESULT: &str = "Tool execution was interrupted or missing.";

/// Normalizes message history in-place before dispatching to the LLM provider.
pub fn normalize_tool_history(messages: &mut Vec<ChatMessage>) {
    let mut pending_tool_calls: HashSet<String> = HashSet::new();
    let mut normalized = Vec::with_capacity(messages.len());
    let mut held_system_messages: Vec<ChatMessage> = Vec::new();

    for msg in messages.drain(..) {
        match msg.role {
            Role::Assistant => {
                // If there were pending tool calls from a previous turn, close them out first
                if !pending_tool_calls.is_empty() {
                    for missing_id in pending_tool_calls.drain() {
                        normalized
                            .push(ChatMessage::tool_response(missing_id, MISSING_TOOL_RESULT));
                    }
                    normalized.append(&mut held_system_messages);
                }

                // Track any new tool calls in this assistant message
                if let Some(ref calls) = msg.tool_calls {
                    for call in calls {
                        pending_tool_calls.insert(call.id.clone());
                    }
                }
                normalized.push(msg);
            }
            Role::Tool => {
                let mut tool_msg = msg;
                if let Some(ref call_id) = tool_msg.tool_call_id {
                    pending_tool_calls.remove(call_id);
                }
                // Guard: Ensure tool output is never empty string
                if let Some(ref mut content) = tool_msg.content
                    && content.trim().is_empty()
                {
                    *content = EMPTY_TOOL_OUTPUT.to_string();
                }
                normalized.push(tool_msg);

                // If all pending tools for this turn are answered, release deferred system messages
                if pending_tool_calls.is_empty() {
                    normalized.append(&mut held_system_messages);
                }
            }
            Role::System => {
                // Defer system messages if we are in the middle of a tool_call -> tool_result block
                if !pending_tool_calls.is_empty() {
                    held_system_messages.push(msg);
                } else {
                    normalized.push(msg);
                }
            }
            Role::User => {
                // Synthesize missing results for any abandoned tool calls before new user input
                if !pending_tool_calls.is_empty() {
                    for missing_id in pending_tool_calls.drain() {
                        normalized
                            .push(ChatMessage::tool_response(missing_id, MISSING_TOOL_RESULT));
                    }
                    normalized.append(&mut held_system_messages);
                }
                normalized.push(msg);
            }
        }
    }

    // Flush any trailing missing tool results at end of history
    for missing_id in pending_tool_calls {
        normalized.push(ChatMessage::tool_response(missing_id, MISSING_TOOL_RESULT));
    }
    normalized.append(&mut held_system_messages);

    *messages = normalized;
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_core::types::{FunctionCall, ToolCall};

    fn make_tool_call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: "{}".to_string(),
            },
        }
    }

    #[test]
    fn test_empty_tool_output_replaced() {
        let mut history = vec![
            ChatMessage::assistant(None, Some(vec![make_tool_call("call_1", "read_file")])),
            ChatMessage::tool_response("call_1", "   "),
        ];

        normalize_tool_history(&mut history);

        assert_eq!(history.len(), 2);
        assert_eq!(history[1].content.as_deref(), Some(EMPTY_TOOL_OUTPUT));
    }

    #[test]
    fn test_missing_tool_call_synthesized() {
        let mut history = vec![
            ChatMessage::assistant(
                None,
                Some(vec![
                    make_tool_call("call_1", "read_file"),
                    make_tool_call("call_2", "exec_command"),
                ]),
            ),
            ChatMessage::tool_response("call_1", "file content"),
            // call_2 was interrupted and never received a response!
            ChatMessage::user("continue please"),
        ];

        normalize_tool_history(&mut history);

        // Expect: Assistant, Tool(call_1), Tool(call_2, MISSING), User
        assert_eq!(history.len(), 4);
        assert_eq!(history[0].role, Role::Assistant);
        assert_eq!(history[1].role, Role::Tool);
        assert_eq!(history[1].tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(history[2].role, Role::Tool);
        assert_eq!(history[2].tool_call_id.as_deref(), Some("call_2"));
        assert_eq!(history[2].content.as_deref(), Some(MISSING_TOOL_RESULT));
        assert_eq!(history[3].role, Role::User);
    }

    #[test]
    fn test_deferred_system_message() {
        let mut history = vec![
            ChatMessage::assistant(None, Some(vec![make_tool_call("call_1", "read_file")])),
            ChatMessage::system("Background agent updated files"),
            ChatMessage::tool_response("call_1", "file content"),
        ];

        normalize_tool_history(&mut history);

        // System message must be deferred until after the tool response
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].role, Role::Assistant);
        assert_eq!(history[1].role, Role::Tool);
        assert_eq!(history[1].tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(history[2].role, Role::System);
        assert_eq!(
            history[2].content.as_deref(),
            Some("Background agent updated files")
        );
    }

    #[test]
    fn test_trailing_tool_call_synthesized() {
        let mut history = vec![ChatMessage::assistant(
            None,
            Some(vec![make_tool_call("call_orphan", "apply_patch")]),
        )];

        normalize_tool_history(&mut history);

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, Role::Assistant);
        assert_eq!(history[1].role, Role::Tool);
        assert_eq!(history[1].tool_call_id.as_deref(), Some("call_orphan"));
        assert_eq!(history[1].content.as_deref(), Some(MISSING_TOOL_RESULT));
    }
}
