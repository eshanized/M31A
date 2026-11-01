//! Context compaction and multi-turn digest engine.
//!
//! Evaluates token pressure in ongoing agent dialogues, calculates turn compaction
//! budgets, collapses oversized tool outputs into semantic digests, and prepares
//! compacted history views with preserved instruction tiers and goal anchors.

use crate::head_tail_buffer::approx_token_count;
use crate::types::{ChatMessage, Role};
use serde::{Deserialize, Serialize};

/// Token budgeting and thresholds for conversation context compaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactTokenBudget {
    /// Context window limit of the target model (e.g. 131,072 for Llama 3.3 / Claude 3.5).
    pub max_context_tokens: usize,
    /// Percentage threshold (0.0 to 1.0) at which compaction should trigger (default: 0.75).
    pub trigger_ratio: f64,
    /// Percentage threshold (0.0 to 1.0) at which background compaction
    /// preparation should start (default: 0.65) — report §5.2: prepare
    /// before the foreground limit is hit.
    #[serde(default = "default_prepare_ratio")]
    pub prepare_ratio: f64,
    /// Target percentage to compress down to (default: 0.40).
    pub target_ratio: f64,
    /// Tokens reserved strictly for system prompts and tool schemas.
    pub reserved_system_tokens: usize,
    /// Maximum tokens allowed for any single tool call response before collapsing to digest.
    pub max_single_tool_output_tokens: usize,
}

impl Default for CompactTokenBudget {
    fn default() -> Self {
        Self {
            max_context_tokens: 128_000,
            trigger_ratio: 0.75,
            prepare_ratio: 0.65,
            target_ratio: 0.40,
            reserved_system_tokens: 4_000,
            max_single_tool_output_tokens: 800,
        }
    }
}

impl CompactTokenBudget {
    pub fn trigger_threshold_tokens(&self) -> usize {
        (self.max_context_tokens as f64 * self.trigger_ratio) as usize
    }

    /// Token estimate at which background compaction preparation starts (§5.2).
    pub fn prepare_threshold_tokens(&self) -> usize {
        (self.max_context_tokens as f64 * self.prepare_ratio) as usize
    }

    pub fn target_tokens(&self) -> usize {
        (self.max_context_tokens as f64 * self.target_ratio) as usize
    }

    pub fn should_compact(&self, total_current_tokens: usize) -> bool {
        total_current_tokens >= self.trigger_threshold_tokens()
    }
}

fn default_prepare_ratio() -> f64 {
    0.65
}

/// A compacted history snapshot with metadata.#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactedHistory {
    pub compacted_messages: Vec<ChatMessage>,
    pub original_tokens: usize,
    pub compacted_tokens: usize,
    pub turns_summarized: usize,
    pub tools_digested: usize,
}

/// Collapses large tool call outputs into high-signal structural digests.
///
/// If a tool output exceeds `max_tokens`:
/// - Preserves error messages in full so the agent understands failures.
/// - For successful large outputs, produces a concise summary with byte/line counts
///   and key structural indicators (e.g. file paths, headers).
pub fn digest_tool_output(
    content: &str,
    tool_name: Option<&str>,
    max_tokens: usize,
) -> (String, bool) {
    let tokens = approx_token_count(content);
    if tokens <= max_tokens {
        return (content.to_string(), false);
    }

    // Always preserve error diagnostics
    let lower = content.to_ascii_lowercase();
    if lower.contains("error:") || lower.contains("fatal:") || lower.contains("exception:") {
        return (content.to_string(), false);
    }

    let line_count = content.lines().count();
    let byte_count = content.len();
    let name = tool_name.unwrap_or("tool");

    let mut preview_lines = Vec::new();
    for line in content.lines().take(4) {
        preview_lines.push(line.trim_end());
    }

    let digest = format!(
        "[Digested {} output: {} lines, {} bytes (~{} tokens)]\n{}\n... [output omitted for context efficiency] ...",
        name,
        line_count,
        byte_count,
        tokens,
        preview_lines.join("\n")
    );

    (digest, true)
}

/// Multi-turn context compactor.
#[derive(Debug, Clone)]
pub struct ContextCompactor {
    budget: CompactTokenBudget,
}

impl ContextCompactor {
    pub fn new(budget: CompactTokenBudget) -> Self {
        Self { budget }
    }

    /// Read-only access to the token budget (thresholds used by callers such as
    /// the engine's wrap-up reminder).
    pub fn budget(&self) -> &CompactTokenBudget {
        &self.budget
    }

    /// Returns estimated tokens remaining in the current context window.
    /// Returns None if total tokens cannot be determined (no message history).
    pub fn tokens_remaining(&self, messages: &[crate::types::ChatMessage]) -> Option<usize> {
        let total_tokens: usize = messages.iter().map(message_token_count).sum();
        let trigger = self.budget.trigger_threshold_tokens();
        if total_tokens >= trigger {
            Some(0)
        } else {
            Some(trigger - total_tokens)
        }
    }

    /// Forces a new context window by performing unconditional compaction.
    /// Returns the compacted history.
    ///
    /// Note: This performs compaction but the actual context reset must be
    /// handled by the caller (e.g., engine replaces history with compacted version).
    pub fn force_new_context(&self, messages: &[crate::types::ChatMessage]) -> CompactedHistory {
        self.compact_unconditional(messages)
    }

    /// Evaluates dialogue history and compresses early turns if threshold is reached.
    pub fn compact_if_needed(&self, messages: &[ChatMessage]) -> CompactedHistory {
        let total_tokens: usize = messages.iter().map(message_token_count).sum();

        if !self.budget.should_compact(total_tokens) {
            return CompactedHistory {
                compacted_messages: messages.to_vec(),
                original_tokens: total_tokens,
                compacted_tokens: total_tokens,
                turns_summarized: 0,
                tools_digested: 0,
            };
        }

        self.compact_messages(messages, total_tokens)
    }

    /// Runs the compaction transform regardless of the trigger threshold.
    ///
    /// Used by the engine's background preparation (§5.2): the engine decides
    /// when to prepare (prepare ratio), so the transform itself must not
    /// re-gate on the foreground trigger.
    pub fn compact_unconditional(&self, messages: &[ChatMessage]) -> CompactedHistory {
        let total_tokens: usize = messages.iter().map(message_token_count).sum();
        self.compact_messages(messages, total_tokens)
    }

    fn compact_messages(&self, messages: &[ChatMessage], total_tokens: usize) -> CompactedHistory {
        // §5.4: contextual fragments (<token_budget>, <context_window>, …)
        // are rollback-trimmable — dropped outright on compaction rather
        // than summarized (they re-inject after rollover).
        let droppable = messages.iter().any(|m| {
            m.content
                .as_deref()
                .map(crate::fragments::is_contextual_text)
                .unwrap_or(false)
        });
        let source: Vec<ChatMessage> = if droppable {
            messages
                .iter()
                .filter(|m| {
                    !m.content
                        .as_deref()
                        .map(crate::fragments::is_contextual_text)
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        } else {
            messages.to_vec()
        };
        if source.is_empty() {
            // Never compact to empty: keep the original messages.
            return CompactedHistory {
                compacted_messages: messages.to_vec(),
                original_tokens: total_tokens,
                compacted_tokens: total_tokens,
                turns_summarized: 0,
                tools_digested: 0,
            };
        }
        let messages: &[ChatMessage] = &source;
        let mut output_messages = Vec::new();
        let mut tools_digested = 0;

        // Pass 1: Digest oversized tool outputs across the board
        let intermediate: Vec<ChatMessage> = messages
            .iter()
            .map(|m| {
                if m.role == Role::Tool
                    && let Some(ref text) = m.content
                {
                    let (digested, was_digested) =
                        digest_tool_output(text, None, self.budget.max_single_tool_output_tokens);
                    if was_digested {
                        tools_digested += 1;
                        let mut copy = m.clone();
                        copy.content = Some(digested);
                        return copy;
                    }
                }
                m.clone()
            })
            .collect();

        let pass1_tokens: usize = intermediate.iter().map(message_token_count).sum();
        if pass1_tokens <= self.budget.target_tokens() {
            return CompactedHistory {
                compacted_messages: intermediate,
                original_tokens: total_tokens,
                compacted_tokens: pass1_tokens,
                turns_summarized: 0,
                tools_digested,
            };
        }

        // Pass 2: Summarize early turns (preserving the last 4 recent turns)
        let keep_tail_count = 4.min(intermediate.len());
        let head_split_idx = intermediate.len().saturating_sub(keep_tail_count);

        let head_to_summarize = &intermediate[..head_split_idx];
        let tail_to_preserve = &intermediate[head_split_idx..];

        let mut summary_lines = Vec::new();
        summary_lines.push("### Earlier Dialogue Summary (Compacted Context)".to_string());
        for (i, msg) in head_to_summarize.iter().enumerate() {
            let role_name = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => "System",
                Role::Tool => "Tool",
            };
            let text = msg.content.as_deref().unwrap_or("[no content]");
            let snippet: String = text.chars().take(120).collect();
            summary_lines.push(format!(
                "- [Turn {} - {}]: {}...",
                i + 1,
                role_name,
                snippet.replace('\n', " ")
            ));
        }

        let summary_message = ChatMessage::system(summary_lines.join("\n"));

        output_messages.push(summary_message);
        output_messages.extend_from_slice(tail_to_preserve);

        let final_tokens: usize = output_messages.iter().map(message_token_count).sum();
        CompactedHistory {
            compacted_messages: output_messages,
            original_tokens: total_tokens,
            compacted_tokens: final_tokens,
            turns_summarized: head_to_summarize.len(),
            tools_digested,
        }
    }
}

fn message_token_count(m: &ChatMessage) -> usize {
    let mut count = 0;
    if let Some(ref text) = m.content {
        count += approx_token_count(text);
    }
    if let Some(ref reasoning) = m.reasoning_content {
        count += approx_token_count(reasoning);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digest_tool_output_preserves_errors_and_short_outputs() {
        let short = "File written successfully.";
        let (res, digested) = digest_tool_output(short, Some("write_file"), 50);
        assert!(!digested);
        assert_eq!(res, short);

        let large_err = "Error: compilation failed\nLine 42: undefined variable foo\n".repeat(20);
        let (err_res, err_digested) = digest_tool_output(&large_err, Some("bash"), 5);
        assert!(!err_digested);
        assert_eq!(err_res, large_err);
    }

    #[test]
    fn test_digest_large_tool_output() {
        let large_success = "Found 500 files matching pattern *.rs\n".repeat(50);
        let (digest, digested) = digest_tool_output(&large_success, Some("find_files"), 20);
        assert!(digested);
        assert!(digest.contains("[Digested find_files output:"));
        assert!(digest.contains("omitted for context efficiency"));
    }

    #[test]
    fn test_prepare_threshold_math() {
        let budget = CompactTokenBudget::default();
        assert_eq!(budget.prepare_ratio, 0.65);
        assert_eq!(
            budget.prepare_threshold_tokens(),
            (128_000.0 * 0.65) as usize
        );
        assert!(budget.prepare_threshold_tokens() < budget.trigger_threshold_tokens());
    }

    #[test]
    fn test_compact_unconditional_below_trigger() {
        // compact_if_needed no-ops below the trigger; compact_unconditional
        // (the background-prepare transform, §5.2) still digests oversized
        // tool outputs.
        let budget = CompactTokenBudget {
            max_context_tokens: 100_000, // trigger 75k — far away
            max_single_tool_output_tokens: 20,
            ..Default::default()
        };
        let compactor = ContextCompactor::new(budget);

        let messages = vec![
            ChatMessage::user("Please build the project."),
            ChatMessage {
                role: Role::Tool,
                content: Some("Cargo build completed with 120 lines of log output\n".repeat(40)),
                tool_calls: None,
                tool_call_id: Some("call-1".to_string()),
                reasoning_content: None,
            },
        ];

        let gated = compactor.compact_if_needed(&messages);
        assert_eq!(gated.turns_summarized, 0);
        assert_eq!(gated.tools_digested, 0);

        let prepared = compactor.compact_unconditional(&messages);
        assert_eq!(
            prepared.tools_digested, 1,
            "prepare digests even below trigger"
        );
        assert!(prepared.compacted_tokens < prepared.original_tokens);
    }

    #[test]
    fn test_context_compactor_threshold_trigger() {
        let budget = CompactTokenBudget {
            max_context_tokens: 1_000,
            trigger_ratio: 0.50, // trigger at 500 tokens
            target_ratio: 0.30,
            max_single_tool_output_tokens: 20,
            ..Default::default()
        };

        let compactor = ContextCompactor::new(budget);

        let messages = vec![
            ChatMessage::user("Please build the project."),
            ChatMessage {
                role: Role::Tool,
                content: Some("Cargo build completed with 120 lines of log output\n".repeat(40)),
                tool_calls: None,
                tool_call_id: Some("call-1".to_string()),
                reasoning_content: None,
            },
            ChatMessage::assistant(
                Some("Build successful. Now running tests.".to_string()),
                None,
            ),
            ChatMessage::user("Proceed with release binary."),
        ];

        let result = compactor.compact_if_needed(&messages);
        assert!(result.tools_digested > 0);
        assert!(result.compacted_tokens < result.original_tokens);
    }

    #[test]
    fn test_contextual_fragments_dropped_on_compaction() {
        // §5.4: contextual (<context_window>-style) messages are
        // rollback-trimmable — dropped on compaction, not summarized.
        assert!(crate::fragments::is_contextual_text(
            "<context_window>\nThe conversation is approaching..."
        ));
        assert!(crate::fragments::is_contextual_text(
            "  <token_budget>128000</token_budget>"
        ));
        assert!(!crate::fragments::is_contextual_text(
            "Please build the project."
        ));

        let budget = CompactTokenBudget {
            max_context_tokens: 1_000, // trigger 750
            trigger_ratio: 0.75,
            max_single_tool_output_tokens: 20,
            ..Default::default()
        };
        let compactor = ContextCompactor::new(budget);

        let messages = vec![
            ChatMessage::user("Build the project.".repeat(200)),
            ChatMessage::user(
                "<context_window>\nThe conversation is approaching the automatic compaction threshold.\n</context_window>".to_string(),
            ),
        ];

        let result = compactor.compact_if_needed(&messages);
        let has_contextual = result.compacted_messages.iter().any(|m| {
            m.content
                .as_deref()
                .map(crate::fragments::is_contextual_text)
                .unwrap_or(false)
        });
        assert!(
            !has_contextual,
            "contextual fragment must not survive compaction"
        );
    }

    #[test]
    fn test_never_compact_to_empty_from_contextual_only() {
        // If every message is contextual, compaction keeps the originals.
        let budget = CompactTokenBudget::default();
        let compactor = ContextCompactor::new(budget);

        let messages = vec![
            ChatMessage::user("<context_window>approaching</context_window>".to_string()),
            ChatMessage::user("<token_budget>1</token_budget>".to_string()),
        ];
        let result = compactor.compact_if_needed(&messages);
        assert_eq!(result.compacted_messages.len(), 2);
        assert_eq!(result.turns_summarized, 0);
    }
}
