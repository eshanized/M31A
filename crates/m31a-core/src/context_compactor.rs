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

    /// Returns the maximum allowable tokens for compactable dialogue after accounting
    /// for reserved system instructions and tool schemas.
    pub fn max_dialogue_tokens(&self) -> usize {
        self.max_context_tokens
            .saturating_sub(self.reserved_system_tokens)
    }

    /// Target token budget available specifically for conversational dialogue.
    pub fn target_dialogue_tokens(&self, system_tokens: usize) -> usize {
        let reserved = self.reserved_system_tokens.max(system_tokens);
        self.target_tokens().saturating_sub(reserved)
    }

    /// Evaluates if total tokens (including system and tool definitions) warrant compaction.
    pub fn should_compact_with_budget(
        &self,
        total_tokens: usize,
        tool_schema_tokens: usize,
    ) -> bool {
        let effective_tokens = total_tokens + tool_schema_tokens;
        effective_tokens >= self.trigger_threshold_tokens()
    }
}

pub const SUMMARY_PREFIX: &str = "### Earlier Dialogue Summary (Compacted Context)";

/// Returns true if the message is a system-generated compaction summary.
pub fn is_compaction_summary(m: &ChatMessage) -> bool {
    m.role == Role::System
        && m.content
            .as_deref()
            .map(|c| c.starts_with(SUMMARY_PREFIX))
            .unwrap_or(false)
}

/// Returns true if the message is an authoritative system instruction.
/// Authoritative system prompts must never be summarized, truncated, or discarded.
pub fn is_authoritative_system_message(m: &ChatMessage) -> bool {
    m.role == Role::System
        && !is_compaction_summary(m)
        && !m
            .content
            .as_deref()
            .map(crate::fragments::is_contextual_text)
            .unwrap_or(false)
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

    /// Returns estimated tokens remaining in the current context window,
    /// accounting for tool definition tokens and system token reservations.
    pub fn tokens_remaining_with_tools(
        &self,
        messages: &[crate::types::ChatMessage],
        tool_tokens: usize,
    ) -> Option<usize> {
        let total_tokens: usize =
            messages.iter().map(message_token_count).sum::<usize>() + tool_tokens;
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

        // P0-A: Separate immutable authoritative system instructions from compactable dialogue.
        // Authoritative system instructions must NEVER be summarized, truncated, or discarded.
        let mut authoritative_system = Vec::new();
        let mut compactable_dialogue = Vec::new();

        for msg in source {
            if is_authoritative_system_message(&msg) {
                authoritative_system.push(msg);
            } else {
                compactable_dialogue.push(msg);
            }
        }

        // If there is no compactable dialogue, preserve the authoritative instructions intact.
        if compactable_dialogue.is_empty() {
            return CompactedHistory {
                compacted_messages: authoritative_system,
                original_tokens: total_tokens,
                compacted_tokens: total_tokens,
                turns_summarized: 0,
                tools_digested: 0,
            };
        }

        let mut tools_digested = 0;

        // Pass 1: Digest oversized tool outputs in compactable dialogue
        let intermediate_dialogue: Vec<ChatMessage> = compactable_dialogue
            .into_iter()
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
                m
            })
            .collect();

        let system_tokens: usize = authoritative_system.iter().map(message_token_count).sum();
        let pass1_dialogue_tokens: usize =
            intermediate_dialogue.iter().map(message_token_count).sum();
        let pass1_total_tokens = system_tokens + pass1_dialogue_tokens;

        if pass1_total_tokens <= self.budget.target_tokens() {
            let mut output = authoritative_system;
            output.extend(intermediate_dialogue);
            return CompactedHistory {
                compacted_messages: output,
                original_tokens: total_tokens,
                compacted_tokens: pass1_total_tokens,
                turns_summarized: 0,
                tools_digested,
            };
        }

        // Pass 2: Summarize early dialogue turns (preserving the last 4 recent dialogue turns)
        let keep_tail_count = 4.min(intermediate_dialogue.len());
        let head_split_idx = intermediate_dialogue.len().saturating_sub(keep_tail_count);

        let head_to_summarize = &intermediate_dialogue[..head_split_idx];
        let tail_to_preserve = &intermediate_dialogue[head_split_idx..];

        let mut output_messages =
            Vec::with_capacity(authoritative_system.len() + 1 + tail_to_preserve.len());
        output_messages.extend(authoritative_system);

        let turns_summarized = head_to_summarize.len();
        if turns_summarized > 0 {
            let mut summary_lines = Vec::new();
            summary_lines.push(SUMMARY_PREFIX.to_string());
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
        }

        output_messages.extend_from_slice(tail_to_preserve);

        let final_tokens: usize = output_messages.iter().map(message_token_count).sum();
        CompactedHistory {
            compacted_messages: output_messages,
            original_tokens: total_tokens,
            compacted_tokens: final_tokens,
            turns_summarized,
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

    #[test]
    fn test_system_prompt_survives_normal_compaction_byte_for_byte() {
        let unique_prompt =
            "You are M31A. CRITICAL UNIQUE DIRECTIVE: #XYZ-98765-AUTH-TOKEN. Do not deviate.";
        let budget = CompactTokenBudget {
            max_context_tokens: 1_000,
            trigger_ratio: 0.50, // triggers at 500 tokens
            target_ratio: 0.25,
            reserved_system_tokens: 200,
            max_single_tool_output_tokens: 20,
            ..Default::default()
        };
        let compactor = ContextCompactor::new(budget);

        let messages = vec![
            ChatMessage::system(unique_prompt),
            ChatMessage::user("Long user message to trigger summarization ".repeat(50)),
            ChatMessage::assistant(Some("Assistant response 1 ".repeat(50)), None),
            ChatMessage::user("User follow-up message ".repeat(50)),
            ChatMessage::assistant(Some("Assistant response 2 ".repeat(50)), None),
            ChatMessage::user("Recent question 1"),
            ChatMessage::assistant(Some("Recent answer 1".into()), None),
            ChatMessage::user("Recent question 2"),
            ChatMessage::assistant(Some("Recent answer 2".into()), None),
        ];

        let result = compactor.compact_if_needed(&messages);
        assert!(
            result.turns_summarized > 0,
            "must have summarized early turns"
        );
        assert!(
            result.compacted_tokens < result.original_tokens,
            "must reduce tokens"
        );

        // System prompt MUST be at index 0 and identical byte-for-byte
        assert_eq!(result.compacted_messages[0].role, Role::System);
        assert_eq!(
            result.compacted_messages[0].content.as_deref(),
            Some(unique_prompt),
            "Authoritative system prompt must survive normal compaction byte-for-byte"
        );

        // Verify summary message is at index 1 and does not repeat system prompt as a summarized turn
        assert_eq!(result.compacted_messages[1].role, Role::System);
        let summary_text = result.compacted_messages[1].content.as_deref().unwrap();
        assert!(summary_text.starts_with(SUMMARY_PREFIX));
        assert!(
            !summary_text.contains("#XYZ-98765-AUTH-TOKEN"),
            "System prompt must not be summarized into the dialogue summary"
        );
    }

    #[test]
    fn test_system_prompt_survives_unconditional_compaction() {
        let unique_prompt = "M31A Authoritative Instructions - Unconditional Test - Token-778899";
        let budget = CompactTokenBudget {
            max_context_tokens: 500,
            trigger_ratio: 0.90,
            target_ratio: 0.20,
            reserved_system_tokens: 50,
            max_single_tool_output_tokens: 30,
            ..Default::default()
        };
        let compactor = ContextCompactor::new(budget);

        let messages = vec![
            ChatMessage::system(unique_prompt),
            ChatMessage::user("Turn 1 ".repeat(80)),
            ChatMessage::assistant(Some("Turn 2 ".repeat(80)), None),
            ChatMessage::user("Turn 3 ".repeat(80)),
            ChatMessage::assistant(Some("Turn 4 ".repeat(80)), None),
            ChatMessage::user("Turn 5"),
            ChatMessage::assistant(Some("Turn 6".into()), None),
        ];

        let result = compactor.compact_unconditional(&messages);
        assert!(result.turns_summarized > 0);
        assert_eq!(result.compacted_messages[0].role, Role::System);
        assert_eq!(
            result.compacted_messages[0].content.as_deref(),
            Some(unique_prompt),
            "System prompt must survive unconditional compaction byte-for-byte"
        );
    }

    #[test]
    fn test_system_prompt_survives_multiple_compaction_cycles() {
        let unique_prompt = "MULTI-CYCLE-INVARIANT-PROMPT-KEY-42";
        let budget = CompactTokenBudget {
            max_context_tokens: 800,
            trigger_ratio: 0.50,
            target_ratio: 0.25,
            reserved_system_tokens: 100,
            max_single_tool_output_tokens: 20,
            ..Default::default()
        };
        let compactor = ContextCompactor::new(budget);

        let mut history = vec![
            ChatMessage::system(unique_prompt),
            ChatMessage::user("Heavy message cycle 1 ".repeat(50)),
            ChatMessage::assistant(Some("Heavy response cycle 1 ".repeat(50)), None),
            ChatMessage::user("Heavy message cycle 1 follow up ".repeat(50)),
            ChatMessage::assistant(Some("Heavy response cycle 1 follow up ".repeat(50)), None),
            ChatMessage::user("Tail 1"),
            ChatMessage::assistant(Some("Tail 2".into()), None),
            ChatMessage::user("Tail 3"),
            ChatMessage::assistant(Some("Tail 4".into()), None),
        ];

        // Cycle 1
        let compacted1 = compactor.compact_if_needed(&history);
        assert!(compacted1.turns_summarized > 0);
        assert_eq!(
            compacted1.compacted_messages[0].content.as_deref(),
            Some(unique_prompt)
        );

        // Add more turns to compacted history to simulate continuous conversation
        history = compacted1.compacted_messages;
        history.push(ChatMessage::user("Heavy message cycle 2 ".repeat(60)));
        history.push(ChatMessage::assistant(
            Some("Heavy response cycle 2 ".repeat(60)),
            None,
        ));
        history.push(ChatMessage::user(
            "Heavy message cycle 2 follow up ".repeat(60),
        ));
        history.push(ChatMessage::assistant(
            Some("Heavy response cycle 2 follow up ".repeat(60)),
            None,
        ));
        history.push(ChatMessage::user("Recent tail A"));
        history.push(ChatMessage::assistant(Some("Recent tail B".into()), None));

        // Cycle 2
        let compacted2 = compactor.compact_if_needed(&history);
        assert!(compacted2.turns_summarized > 0);
        assert_eq!(
            compacted2.compacted_messages[0].content.as_deref(),
            Some(unique_prompt),
            "System prompt must remain byte-for-byte identical across second compaction cycle"
        );

        // Cycle 3: Add yet more turns and compact unconditionally
        history = compacted2.compacted_messages;
        history.push(ChatMessage::user("Cycle 3 heavy ".repeat(60)));
        history.push(ChatMessage::assistant(
            Some("Cycle 3 heavy reply ".repeat(60)),
            None,
        ));
        let compacted3 = compactor.compact_unconditional(&history);
        assert_eq!(
            compacted3.compacted_messages[0].content.as_deref(),
            Some(unique_prompt),
            "System prompt must remain byte-for-byte identical across third compaction cycle"
        );
    }

    #[test]
    fn test_tool_result_digestion_cannot_alter_system_instructions() {
        let unique_prompt = "PRESERVE-DURING-TOOL-DIGESTION-KEY";
        let budget = CompactTokenBudget {
            max_context_tokens: 5_000,
            trigger_ratio: 0.80,
            target_ratio: 0.30,
            reserved_system_tokens: 200,
            max_single_tool_output_tokens: 15, // aggressive tool digestion threshold
            ..Default::default()
        };
        let compactor = ContextCompactor::new(budget);

        let messages = vec![
            ChatMessage::system(unique_prompt),
            ChatMessage::user("Run command"),
            ChatMessage {
                role: Role::Tool,
                content: Some(
                    "Gigantic output with dozens of log lines and symbols...\n".repeat(30),
                ),
                tool_calls: None,
                tool_call_id: Some("call-xyz".into()),
                reasoning_content: None,
            },
            ChatMessage::assistant(Some("Command done".into()), None),
        ];

        let result = compactor.compact_unconditional(&messages);
        assert!(result.tools_digested > 0);
        assert_eq!(result.compacted_messages[0].role, Role::System);
        assert_eq!(
            result.compacted_messages[0].content.as_deref(),
            Some(unique_prompt),
            "Tool digestion must never modify or alter system instructions"
        );
    }

    #[test]
    fn test_malformed_history_preserves_authoritative_prompt() {
        let unique_prompt = "SURVIVE-MALFORMED-HISTORY-PROMPT";
        let budget = CompactTokenBudget::default();
        let compactor = ContextCompactor::new(budget);

        // Case 1: Only system prompt present
        let only_sys = vec![ChatMessage::system(unique_prompt)];
        let res1 = compactor.compact_unconditional(&only_sys);
        assert_eq!(res1.compacted_messages.len(), 1);
        assert_eq!(
            res1.compacted_messages[0].content.as_deref(),
            Some(unique_prompt)
        );

        // Case 2: System prompt with only contextual fragments (which get dropped)
        let sys_and_contextual = vec![
            ChatMessage::system(unique_prompt),
            ChatMessage::user("<token_budget>9999</token_budget>"),
            ChatMessage::user("<context_window>approaching</context_window>"),
        ];
        let res2 = compactor.compact_unconditional(&sys_and_contextual);
        assert_eq!(res2.compacted_messages.len(), 1);
        assert_eq!(
            res2.compacted_messages[0].content.as_deref(),
            Some(unique_prompt)
        );
    }

    #[test]
    fn test_compaction_budget_accounts_for_system_and_tool_tokens() {
        let budget = CompactTokenBudget {
            max_context_tokens: 100_000,
            trigger_ratio: 0.75, // 75,000
            prepare_ratio: 0.65, // 65,000
            target_ratio: 0.40,  // 40,000
            reserved_system_tokens: 5_000,
            max_single_tool_output_tokens: 800,
        };

        assert_eq!(budget.max_dialogue_tokens(), 95_000);
        assert_eq!(budget.target_dialogue_tokens(5_000), 35_000);
        // If system prompt is larger than reserved (e.g. 8,000 tokens)
        assert_eq!(budget.target_dialogue_tokens(8_000), 32_000);

        // Trigger logic with tool tokens
        assert!(!budget.should_compact_with_budget(70_000, 2_000)); // 72,000 < 75,000
        assert!(budget.should_compact_with_budget(70_000, 6_000)); // 76,000 >= 75,000

        let compactor = ContextCompactor::new(budget);
        let messages = vec![
            ChatMessage::system("Base system prompt instructions"),
            ChatMessage::user("Hello agent"),
        ];
        let remaining_without_tools = compactor.tokens_remaining(&messages).unwrap();
        let remaining_with_tools = compactor
            .tokens_remaining_with_tools(&messages, 10_000)
            .unwrap();
        assert_eq!(remaining_without_tools - remaining_with_tools, 10_000);
    }
}
