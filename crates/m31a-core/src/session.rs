//! Conversation session lifecycle, metadata tracking, and persistence states.
//!
//! Represents active user sessions, active turn states, thread IDs, and runtime execution flags.

use crate::types::{ChatMessage, Role, SessionMetadata, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SemanticTurnFacts {
    pub files_touched: Vec<String>,
    pub commands_executed: Vec<String>,
    pub user_directives: Vec<String>,
    pub assistant_conclusions: Vec<String>,
    pub tool_errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetRecommendation {
    Normal,
    WarningCompressSoon,
    CriticalCompressNow,
    EmergencyPrune,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBudgetStatus {
    pub current_tokens: usize,
    pub max_tokens: usize,
    pub percent_used: f64,
    pub recommendation: BudgetRecommendation,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub metadata: SessionMetadata,
    pub history: Vec<ChatMessage>,
    pub token_usage: TokenUsage,
    pub active_plan: Option<String>,
    pub active_goal: Option<String>,
    pub active: bool,
}

impl Session {
    pub fn new(model: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let cwd_str = cwd.into().to_string_lossy().to_string();
        Self {
            metadata: SessionMetadata {
                id: id.clone(),
                title: format!("Session {}", &id[..8]),
                model: model.into(),
                cwd: cwd_str,
                created_at: now,
                updated_at: now,
                extra: HashMap::new(),
            },
            history: Vec::new(),
            token_usage: TokenUsage::default(),
            active_plan: None,
            active_goal: None,
            active: true,
        }
    }

    pub fn push_message(&mut self, message: ChatMessage) {
        self.history.push(message);
        self.metadata.updated_at = chrono::Utc::now();
    }

    pub fn record_tokens(&mut self, prompt: u64, completion: u64, reasoning: u64) {
        self.token_usage.prompt_tokens += prompt;
        self.token_usage.completion_tokens += completion;
        self.token_usage.reasoning_tokens += reasoning;
        self.token_usage.total_tokens =
            self.token_usage.prompt_tokens + self.token_usage.completion_tokens;
    }

    /// Check if conversation history has exceeded the token auto-compaction limit
    pub fn should_compact(&self, token_limit: usize) -> bool {
        let estimated_tokens = self.estimate_history_tokens();
        self.token_usage.total_tokens as usize >= token_limit || estimated_tokens >= token_limit
    }

    /// Evaluates current token usage against a max context window threshold,
    /// returning budget ratio and proactive optimization recommendation.
    pub fn evaluate_context_budget(&self, max_tokens: usize) -> ContextBudgetStatus {
        let current_tokens = self
            .estimate_history_tokens()
            .max(self.token_usage.total_tokens as usize);
        let max_tokens = max_tokens.max(1);
        let ratio = current_tokens as f64 / max_tokens as f64;
        let percent_used = ratio * 100.0;

        let recommendation = if ratio >= 0.92 {
            BudgetRecommendation::EmergencyPrune
        } else if ratio >= 0.80 {
            BudgetRecommendation::CriticalCompressNow
        } else if ratio >= 0.65 {
            BudgetRecommendation::WarningCompressSoon
        } else {
            BudgetRecommendation::Normal
        };

        ContextBudgetStatus {
            current_tokens,
            max_tokens,
            percent_used,
            recommendation,
        }
    }

    /// Rough token estimation (~4 chars per token)
    pub fn estimate_history_tokens(&self) -> usize {
        let total_chars: usize = self
            .history
            .iter()
            .map(|m| m.content.as_deref().unwrap_or_default().len())
            .sum();
        total_chars / 4
    }

    /// Prunes large intermediate tool outputs in conversational history to their head & tail lines
    /// to immediately free context budget during emergency conditions.
    pub fn emergency_prune_tool_outputs(&mut self, max_lines_per_tool: usize) -> usize {
        let mut pruned_count = 0;
        for msg in &mut self.history {
            if msg.role == Role::Tool
                && let Some(ref mut content) = msg.content
            {
                let lines: Vec<&str> = content.lines().collect();
                if lines.len() > max_lines_per_tool * 2 {
                    let head = &lines[..max_lines_per_tool];
                    let tail = &lines[lines.len() - max_lines_per_tool..];
                    let truncated = format!(
                        "{}\n... [Truncated {} lines of output to preserve context budget] ...\n{}",
                        head.join("\n"),
                        lines.len() - (max_lines_per_tool * 2),
                        tail.join("\n")
                    );
                    *content = truncated;
                    pruned_count += 1;
                }
            }
        }
        pruned_count
    }

    /// Extracts semantic turn facts (files touched, commands executed, user directives,
    /// assistant conclusions, and tool errors) from conversation messages.
    pub fn extract_semantic_facts(&self, messages: &[ChatMessage]) -> SemanticTurnFacts {
        let mut files_set = HashSet::new();
        let mut commands = Vec::new();
        let mut user_directives = Vec::new();
        let mut assistant_conclusions = Vec::new();
        let mut tool_errors = Vec::new();

        for msg in messages {
            match msg.role {
                Role::User => {
                    if let Some(ref text) = msg.content {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() && !trimmed.starts_with("<compacted_context>") {
                            let snippet: String = trimmed
                                .lines()
                                .filter(|l| !l.trim().is_empty())
                                .take(2)
                                .collect::<Vec<_>>()
                                .join(" ");
                            let truncated = if snippet.len() > 180 {
                                format!("{}...", &snippet[..180])
                            } else {
                                snippet
                            };
                            if !user_directives.contains(&truncated) {
                                user_directives.push(truncated);
                            }
                        }
                    }
                }
                Role::Assistant => {
                    if let Some(ref text) = msg.content {
                        for line in text.lines() {
                            let line_t = line.trim();
                            if line_t.starts_with("- ") || line_t.starts_with("* ") {
                                let item = line_t
                                    .trim_start_matches("- ")
                                    .trim_start_matches("* ")
                                    .trim();
                                if item.len() > 15 && item.len() < 220 {
                                    let item_str = item.to_string();
                                    if !assistant_conclusions.contains(&item_str) {
                                        assistant_conclusions.push(item_str);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(ref calls) = msg.tool_calls {
                        for call in calls {
                            let args_str = &call.function.arguments;
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(args_str) {
                                for key in &["path", "file_path", "Path", "target", "file"] {
                                    if let Some(p) = v.get(*key).and_then(|val| val.as_str())
                                        && !p.is_empty()
                                    {
                                        files_set.insert(p.to_string());
                                    }
                                }
                                for key in &["CommandLine", "command", "cmd"] {
                                    if let Some(c) = v.get(*key).and_then(|val| val.as_str())
                                        && !c.is_empty()
                                    {
                                        let cmd_short = if c.len() > 90 {
                                            format!("{}...", &c[..90])
                                        } else {
                                            c.to_string()
                                        };
                                        if !commands.contains(&cmd_short) {
                                            commands.push(cmd_short);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Role::Tool => {
                    if let Some(ref text) = msg.content
                        && (text.contains("error")
                            || text.contains("Error")
                            || text.contains("FAILED")
                            || text.contains("fatal"))
                    {
                        let first_line = text
                            .lines()
                            .find(|l| {
                                l.contains("error")
                                    || l.contains("Error")
                                    || l.contains("FAILED")
                                    || l.contains("fatal")
                            })
                            .unwrap_or_else(|| text.lines().next().unwrap_or_default())
                            .trim();
                        let err_snippet = if first_line.len() > 140 {
                            format!("{}...", &first_line[..140])
                        } else {
                            first_line.to_string()
                        };
                        if !tool_errors.contains(&err_snippet) {
                            tool_errors.push(err_snippet);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut files_touched: Vec<String> = files_set.into_iter().collect();
        files_touched.sort();

        SemanticTurnFacts {
            files_touched,
            commands_executed: commands,
            user_directives,
            assistant_conclusions,
            tool_errors,
        }
    }

    /// Formats extracted semantic turn facts into a structured, highly dense compaction block.
    pub fn format_semantic_summary(&self, facts: &SemanticTurnFacts) -> String {
        let mut s = String::from(
            "<compacted_context>\n### Prior Conversation Milestones & Accomplishments\n",
        );

        if !facts.user_directives.is_empty() {
            s.push_str("#### User Requests & Directives:\n");
            for req in facts.user_directives.iter().take(6) {
                s.push_str(&format!("- {}\n", req));
            }
        }

        if !facts.assistant_conclusions.is_empty() {
            s.push_str("\n#### Key Actions & Milestones:\n");
            for act in facts.assistant_conclusions.iter().take(8) {
                s.push_str(&format!("- {}\n", act));
            }
        }

        if !facts.files_touched.is_empty() {
            s.push_str("\n#### Files Touched / Modified:\n");
            for file in facts.files_touched.iter().take(12) {
                s.push_str(&format!("- `{}`\n", file));
            }
        }

        if !facts.commands_executed.is_empty() {
            s.push_str("\n#### Shell Commands Executed:\n");
            for cmd in facts.commands_executed.iter().take(6) {
                s.push_str(&format!("- `{}`\n", cmd));
            }
        }

        if !facts.tool_errors.is_empty() {
            s.push_str("\n#### Encountered Issues & Remediations:\n");
            for err in facts.tool_errors.iter().take(4) {
                s.push_str(&format!("- {}\n", err));
            }
        }

        if let Some(ref plan) = self.active_plan {
            s.push_str(&format!("\n### Active Plan\n{}\n", plan));
        }
        if let Some(ref goal) = self.active_goal {
            s.push_str(&format!("\n### Active Goal\n{}\n", goal));
        }

        s.push_str("</compacted_context>");
        s
    }

    /// Compacts older conversation history turns into a structured semantic summary block
    /// while preserving system prompts, active plan/goals, and recent turns.
    pub fn compact_history(&mut self, keep_recent_turns: usize) -> Option<String> {
        if self.history.len() <= keep_recent_turns + 2 {
            return None;
        }

        // Separate system messages from conversational messages
        let mut system_messages = Vec::new();
        let mut conv_messages = Vec::new();

        for msg in self.history.drain(..) {
            if msg.role == Role::System {
                system_messages.push(msg);
            } else {
                conv_messages.push(msg);
            }
        }

        if conv_messages.len() <= keep_recent_turns {
            self.history = system_messages;
            self.history.extend(conv_messages);
            return None;
        }

        let split_idx = conv_messages.len().saturating_sub(keep_recent_turns);
        let older = &conv_messages[..split_idx];
        let recent = &conv_messages[split_idx..];

        // Synthesize structured semantic compaction summary of older turns
        let facts = self.extract_semantic_facts(older);
        let summary = self.format_semantic_summary(&facts);

        // Rebuild history: system messages -> compacted context message -> recent messages
        let mut new_history = system_messages;
        new_history.push(ChatMessage::user(summary.clone()));
        new_history.push(ChatMessage::assistant(
            Some("Understood. I have internalized the compacted session context and am continuing the current task.".to_string()),
            None,
        ));
        new_history.extend(recent.to_vec());

        self.history = new_history;
        self.metadata.updated_at = chrono::Utc::now();

        Some(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FunctionCall, ToolCall};

    #[test]
    fn test_compaction_lifecycle() {
        let mut session = Session::new("test-model", ".");
        session.push_message(ChatMessage::system("System instructions"));

        for i in 1..=10 {
            session.push_message(ChatMessage::user(format!("User message {}", i)));
            session.push_message(ChatMessage::assistant(
                Some(format!("Assistant response {}", i)),
                None,
            ));
        }

        assert_eq!(session.history.len(), 21);
        let summary = session.compact_history(4);
        assert!(summary.is_some());
        assert!(summary.unwrap().contains("<compacted_context>"));
        // System + 1 User summary + 1 Assistant ack + 4 recent messages = 7
        assert_eq!(session.history.len(), 7);
    }

    #[test]
    fn test_semantic_turn_facts_extraction_and_formatting() {
        let session = Session::new("test-model", ".");
        let mut messages = Vec::new();

        messages.push(ChatMessage::user(
            "Please modify src/main.rs and run tests.",
        ));

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "write_file".to_string(),
                arguments: serde_json::json!({
                    "path": "src/main.rs",
                    "content": "fn main() {}"
                })
                .to_string(),
            },
        };
        let exec_call = ToolCall {
            id: "call_2".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "exec_command".to_string(),
                arguments: serde_json::json!({
                    "command": "cargo test --workspace"
                })
                .to_string(),
            },
        };

        messages.push(ChatMessage::assistant(
            Some(
                "- Refactored main loop to avoid lock conflicts\n- Added integration test suite"
                    .to_string(),
            ),
            Some(vec![tool_call, exec_call]),
        ));

        messages.push(ChatMessage {
            role: Role::Tool,
            content: Some("Error: compilation failed on line 42".to_string()),
            tool_calls: None,
            tool_call_id: Some("call_2".to_string()),
            reasoning_content: None,
        });

        let facts = session.extract_semantic_facts(&messages);
        assert!(facts.files_touched.contains(&"src/main.rs".to_string()));
        assert!(
            facts
                .commands_executed
                .contains(&"cargo test --workspace".to_string())
        );
        assert_eq!(facts.user_directives.len(), 1);
        assert!(facts.user_directives[0].contains("Please modify src/main.rs"));
        assert_eq!(facts.assistant_conclusions.len(), 2);
        assert!(
            facts
                .tool_errors
                .iter()
                .any(|e| e.contains("compilation failed"))
        );

        let summary = session.format_semantic_summary(&facts);
        assert!(summary.contains("<compacted_context>"));
        assert!(summary.contains("`src/main.rs`"));
        assert!(summary.contains("`cargo test --workspace`"));
        assert!(summary.contains("Refactored main loop"));
        assert!(summary.contains("compilation failed"));
    }

    #[test]
    fn test_context_budget_evaluation_and_emergency_prune() {
        let mut session = Session::new("test-model", ".");

        let budget_empty = session.evaluate_context_budget(100_000);
        assert_eq!(budget_empty.recommendation, BudgetRecommendation::Normal);

        // Add large tool message with 100 lines
        let large_output = (1..=100)
            .map(|i| format!("Compiling dependency package lib_{} ... ok", i))
            .collect::<Vec<_>>()
            .join("\n");

        session.push_message(ChatMessage {
            role: Role::Tool,
            content: Some(large_output),
            tool_calls: None,
            tool_call_id: Some("call_big".to_string()),
            reasoning_content: None,
        });

        let pruned = session.emergency_prune_tool_outputs(5);
        assert_eq!(pruned, 1);

        let content = session.history[0].content.as_ref().unwrap();
        assert!(content.contains("[Truncated 90 lines of output to preserve context budget]"));
        assert!(content.contains("lib_1"));
        assert!(content.contains("lib_100"));

        // Simulate high token usage
        session.record_tokens(85_000, 5_000, 1_000);
        let budget_critical = session.evaluate_context_budget(100_000);
        assert_eq!(
            budget_critical.recommendation,
            BudgetRecommendation::CriticalCompressNow
        );
    }
}
