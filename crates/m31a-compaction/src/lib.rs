//! Session Compaction and Summarization
//!
//! Automatic context window management for long-running sessions.

pub mod degradation;
pub mod monitor;

pub use degradation::{
    PROGRESSIVE_LEVELS, STRUCTURED_SUMMARY_TEMPLATE, TranscriptDegradationLevel, degrade_transcript,
};
pub use monitor::{
    CRITICAL_HEADROOM_PCT, ContextHeadroomMonitor, EXHAUSTED_HEADROOM_PCT, HeadroomSeverity,
    HeadroomSnapshot, WARNING_HEADROOM_PCT,
};

use anyhow::Result;
use m31a_core::types::ChatMessage;
use m31a_provider::{ChatRequest, ModelProvider};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Configuration for session compaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Token threshold as percentage of context window (0.0-1.0)
    pub threshold_pct: f32,
    /// Minimum tokens before compaction is considered
    pub min_tokens: usize,
    /// Maximum tokens to retain after compaction
    pub max_retained_tokens: usize,
    /// Whether to preserve tool calls in summary
    pub preserve_tool_calls: bool,
    /// Whether to preserve reasoning traces
    pub preserve_reasoning: bool,
    /// Custom system prompt for summarization
    pub summary_prompt: Option<String>,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            threshold_pct: 0.8, // Compact at 80% context window
            min_tokens: 1000,
            max_retained_tokens: 4000,
            preserve_tool_calls: true,
            preserve_reasoning: true,
            summary_prompt: None,
        }
    }
}

/// Summary of a compacted conversation segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub original_turns: usize,
    pub original_tokens: usize,
    pub summary_tokens: usize,
    pub summary_text: String,
    pub key_decisions: Vec<String>,
    pub files_modified: Vec<String>,
    pub tools_used: Vec<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Result of a compaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionResult {
    pub session_id: String,
    pub compacted: bool,
    pub original_tokens: usize,
    pub new_tokens: usize,
    pub summaries_created: usize,
    pub summary_ids: Vec<String>,
}

/// Trait for storage backends that support compaction
pub trait CompactionStorage: Send + Sync {
    /// Get conversation history for a session
    fn get_history(&self, session_id: &str) -> Result<Vec<ChatMessage>>;

    /// Replace history with compacted version
    fn replace_history(&self, session_id: &str, messages: Vec<ChatMessage>) -> Result<()>;

    /// Store a summary
    fn store_summary(&self, session_id: &str, summary: ConversationSummary) -> Result<()>;

    /// Get all summaries for a session
    fn get_summaries(&self, session_id: &str) -> Result<Vec<ConversationSummary>>;
}

/// Session compactor for managing context window
pub struct SessionCompactor {
    provider: Arc<dyn ModelProvider>,
    config: CompactionConfig,
    storage: Arc<dyn CompactionStorage>,
    tokenizer: Arc<dyn Tokenizer>,
}

impl SessionCompactor {
    pub fn new(
        provider: Arc<dyn ModelProvider>,
        config: CompactionConfig,
        storage: Arc<dyn CompactionStorage>,
        tokenizer: Arc<dyn Tokenizer>,
    ) -> Self {
        Self {
            provider,
            config,
            storage,
            tokenizer,
        }
    }

    /// Check if compaction is needed and perform it
    pub async fn compact_if_needed(&self, session_id: &str) -> Result<CompactionResult> {
        let history = self.storage.get_history(session_id)?;
        let total_tokens = self.count_tokens(&history);

        let model_capabilities = self.provider.capabilities();
        let context_window = model_capabilities.max_context_tokens as usize;
        let threshold = (context_window as f32 * self.config.threshold_pct) as usize;

        if total_tokens < threshold || total_tokens < self.config.min_tokens {
            return Ok(CompactionResult {
                session_id: session_id.to_string(),
                compacted: false,
                original_tokens: total_tokens,
                new_tokens: total_tokens,
                summaries_created: 0,
                summary_ids: vec![],
            });
        }

        self.compact_session(session_id, &history, total_tokens)
            .await
    }

    /// Force compaction of a session
    pub async fn force_compact(&self, session_id: &str) -> Result<CompactionResult> {
        let history = self.storage.get_history(session_id)?;
        let total_tokens = self.count_tokens(&history);
        self.compact_session(session_id, &history, total_tokens)
            .await
    }

    /// Perform the actual compaction
    async fn compact_session(
        &self,
        session_id: &str,
        history: &[ChatMessage],
        total_tokens: usize,
    ) -> Result<CompactionResult> {
        // Split history into segments for summarization
        let segments = self.split_into_segments(history);

        let mut summaries = Vec::new();
        let mut summary_ids = Vec::new();

        for segment in segments {
            if segment.is_empty() {
                continue;
            }

            let summary = self.summarize_segment(&segment).await?;
            let summary_id = Uuid::new_v4().to_string();
            let mut summary_with_id = summary;
            summary_with_id.id = summary_id.clone();

            self.storage
                .store_summary(session_id, summary_with_id.clone())?;
            summary_ids.push(summary_id);
            summaries.push(summary_with_id);
        }

        // Build new history: summaries + recent messages
        let recent_messages = self.get_recent_messages(history)?;
        let mut new_history = Vec::new();

        // Add summaries as system messages
        for summary in &summaries {
            new_history.push(ChatMessage::system(format!(
                "[CONVERSATION SUMMARY]\n{}\n[END SUMMARY]",
                summary.summary_text
            )));
        }

        // Add recent messages
        new_history.extend(recent_messages);

        // Store compacted history
        self.storage
            .replace_history(session_id, new_history.clone())?;

        let new_tokens = self.count_tokens(&new_history);

        Ok(CompactionResult {
            session_id: session_id.to_string(),
            compacted: true,
            original_tokens: total_tokens,
            new_tokens,
            summaries_created: summaries.len(),
            summary_ids,
        })
    }

    /// Split history into segments for summarization
    fn split_into_segments(&self, history: &[ChatMessage]) -> Vec<Vec<ChatMessage>> {
        let mut segments = Vec::new();
        let mut current_segment = Vec::new();
        let mut current_tokens = 0;
        let max_segment_tokens = self.config.max_retained_tokens / 2;

        for msg in history {
            let msg_tokens = self
                .tokenizer
                .count_tokens(msg.content.as_deref().unwrap_or(""));
            if current_tokens + msg_tokens > max_segment_tokens && !current_segment.is_empty() {
                segments.push(current_segment);
                current_segment = Vec::new();
                current_tokens = 0;
            }
            current_tokens += msg_tokens;
            current_segment.push(msg.clone());
        }

        if !current_segment.is_empty() {
            segments.push(current_segment);
        }

        segments
    }

    /// Summarize a conversation segment
    async fn summarize_segment(&self, segment: &[ChatMessage]) -> Result<ConversationSummary> {
        let prompt = self
            .config
            .summary_prompt
            .clone()
            .unwrap_or_else(|| STRUCTURED_SUMMARY_TEMPLATE.to_string());

        // Build messages for summarization
        let mut messages = vec![ChatMessage::system(prompt)];
        for msg in segment {
            messages.push(msg.clone());
        }

        let request = ChatRequest {
            model: self.provider.current_model().to_string(),
            messages,
            tools: None,
            tool_choice: None,
            temperature: Some(0.3),
            max_tokens: Some(1000),
            stream: false,
            reasoning_effort: None,
            extra_params: HashMap::new(),
        };

        let response = self.provider.chat(request).await?;

        // Extract key information
        let summary_text = response.message.content.unwrap_or_default();
        let key_decisions = self.extract_key_decisions(&summary_text);
        let files_modified = self.extract_files_modified(segment);
        let tools_used = self.extract_tools_used(segment);

        let original_tokens = self.count_tokens(segment);

        Ok(ConversationSummary {
            id: String::new(), // Will be set by caller
            original_turns: segment.len(),
            original_tokens,
            summary_tokens: self.tokenizer.count_tokens(&summary_text),
            summary_text,
            key_decisions,
            files_modified,
            tools_used,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Get recent messages to retain
    fn get_recent_messages(&self, history: &[ChatMessage]) -> Result<Vec<ChatMessage>> {
        let mut recent = Vec::new();
        let mut tokens = 0;
        let max_recent_tokens = self.config.max_retained_tokens / 2;

        for msg in history.iter().rev() {
            let msg_tokens = self
                .tokenizer
                .count_tokens(msg.content.as_deref().unwrap_or(""));
            if tokens + msg_tokens > max_recent_tokens && !recent.is_empty() {
                break;
            }
            tokens += msg_tokens;
            recent.insert(0, msg.clone());
        }

        Ok(recent)
    }

    /// Count total tokens in messages
    fn count_tokens(&self, messages: &[ChatMessage]) -> usize {
        messages
            .iter()
            .map(|m| {
                self.tokenizer
                    .count_tokens(m.content.as_deref().unwrap_or(""))
            })
            .sum()
    }

    /// Extract key decisions from summary text
    fn extract_key_decisions(&self, summary: &str) -> Vec<String> {
        summary
            .lines()
            .filter(|line| {
                line.contains("decision") || line.contains("decided") || line.contains("chose")
            })
            .map(|s| s.trim().to_string())
            .collect()
    }

    /// Extract files modified from segment
    fn extract_files_modified(&self, segment: &[ChatMessage]) -> Vec<String> {
        let mut files = Vec::new();
        for msg in segment {
            if let Some(tool_calls) = &msg.tool_calls {
                for call in tool_calls {
                    if call.function.name.contains("write")
                        || call.function.name.contains("edit")
                        || call.function.name.contains("patch")
                    {
                        // Try to extract file path from arguments
                        if let Ok(args) =
                            serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                            && let Some(path) = args.get("path").or_else(|| args.get("file_path"))
                            && let Some(p) = path.as_str()
                        {
                            files.push(p.to_string());
                        }
                    }
                }
            }
        }
        files.sort();
        files.dedup();
        files
    }

    /// Extract tools used from segment
    fn extract_tools_used(&self, segment: &[ChatMessage]) -> Vec<String> {
        let mut tools = Vec::new();
        for msg in segment {
            if let Some(tool_calls) = &msg.tool_calls {
                for call in tool_calls {
                    tools.push(call.function.name.clone());
                }
            }
        }
        tools.sort();
        tools.dedup();
        tools
    }
}

/// Simple tokenizer using character approximation
pub trait Tokenizer: Send + Sync {
    fn count_tokens(&self, text: &str) -> usize;
}

pub struct CharTokenizer;

impl Tokenizer for CharTokenizer {
    fn count_tokens(&self, text: &str) -> usize {
        // Rough approximation: 1 token ≈ 4 characters for English
        (text.len() as f32 / 4.0).ceil() as usize
    }
}

/// Advanced subword/BPE-aware tokenizer approximating BPE tokens for code, symbols, and prose.
pub struct SubwordTokenizer;

impl Tokenizer for SubwordTokenizer {
    fn count_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        let mut token_count = 0;
        let mut in_word = false;
        let mut word_len = 0;

        for ch in text.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                if !in_word {
                    in_word = true;
                    word_len = 1;
                } else {
                    word_len += 1;
                    if ch.is_uppercase() && word_len > 1 {
                        token_count += 1;
                        word_len = 1;
                    }
                }
            } else {
                if in_word {
                    token_count += (word_len as f32 / 3.6).ceil() as usize;
                    in_word = false;
                    word_len = 0;
                }
                if !ch.is_whitespace() || ch == '\n' {
                    token_count += 1;
                }
            }
        }

        if in_word {
            token_count += (word_len as f32 / 3.6).ceil() as usize;
        }

        token_count.max(1)
    }
}

/// Storage implementation for m31a-storage
impl CompactionStorage for m31a_storage::Storage {
    fn get_history(&self, session_id: &str) -> Result<Vec<ChatMessage>> {
        let items = self.list_items(session_id)?;
        let mut messages = Vec::new();
        for (item_type, content, item_id) in items {
            if item_type == "summary" {
                continue;
            }
            if let Ok(msg) = serde_json::from_str::<ChatMessage>(&content) {
                messages.push(msg);
            } else {
                let msg = match item_type.as_str() {
                    "user" | "user_message" | "user_queued" => ChatMessage::user(content),
                    "assistant" | "assistant_message" => {
                        ChatMessage::assistant(Some(content), None)
                    }
                    "tool" | "tool_result" => ChatMessage::tool_response(item_id, content),
                    "system" => ChatMessage::system(content),
                    _ => ChatMessage::system(content),
                };
                messages.push(msg);
            }
        }
        Ok(messages)
    }

    fn replace_history(&self, session_id: &str, messages: Vec<ChatMessage>) -> Result<()> {
        let mut records = Vec::with_capacity(messages.len());
        for msg in messages {
            let item_type = match msg.role {
                m31a_core::types::Role::User => "user",
                m31a_core::types::Role::Assistant => "assistant",
                m31a_core::types::Role::System => "system",
                m31a_core::types::Role::Tool => "tool",
            };
            let content = serde_json::to_string(&msg)?;
            records.push((item_type.to_string(), content));
        }
        self.replace_thread_items(session_id, &records)?;
        Ok(())
    }

    fn store_summary(&self, session_id: &str, summary: ConversationSummary) -> Result<()> {
        let summary_id = if summary.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            summary.id.clone()
        };
        let content = serde_json::to_string(&summary)?;
        self.save_item(&summary_id, session_id, None, "summary", &content)?;
        Ok(())
    }

    fn get_summaries(&self, session_id: &str) -> Result<Vec<ConversationSummary>> {
        let items = self.list_items(session_id)?;
        let mut summaries = Vec::new();
        for (item_type, content, _) in items {
            if item_type == "summary"
                && let Ok(summary) = serde_json::from_str::<ConversationSummary>(&content)
            {
                summaries.push(summary);
            }
        }
        Ok(summaries)
    }
}

/// Prunes verbose bodies of historic tool results and earlier reasoning traces,
/// reclaiming context tokens while preserving recent turns in full fidelity
/// (Bolt.diy `simplifyBoltActions` parity).
pub fn prune_historic_tool_actions(
    messages: &[ChatMessage],
    keep_recent_tool_count: usize,
) -> Vec<ChatMessage> {
    let tool_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == m31a_core::types::Role::Tool)
        .map(|(i, _)| i)
        .collect();

    let prune_threshold_idx = if tool_indices.len() > keep_recent_tool_count {
        tool_indices[tool_indices.len() - keep_recent_tool_count]
    } else {
        0
    };

    let mut pruned = Vec::with_capacity(messages.len());

    for (i, msg) in messages.iter().enumerate() {
        let mut new_msg = msg.clone();

        // If it's a tool message older than the keep threshold
        if msg.role == m31a_core::types::Role::Tool && i < prune_threshold_idx {
            if let Some(content) = &msg.content
                && content.len() > 160
            {
                let first_line = content.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                let prefix = if first_line.len() > 80 {
                    &first_line[..80]
                } else {
                    first_line
                };
                new_msg.content = Some(format!(
                    "{}\n[... {} characters of historic tool output pruned for context efficiency ...]",
                    prefix,
                    content.len().saturating_sub(prefix.len())
                ));
            }
        } else if msg.role == m31a_core::types::Role::Assistant && i < prune_threshold_idx {
            // Prune earlier reasoning/thinking content from older assistant messages
            if new_msg.reasoning_content.is_some() {
                new_msg.reasoning_content = None;
            }
        }

        pruned.push(new_msg);
    }

    pruned
}

/// Minimum recent tool execution records guaranteed never to be pruned (Codex invariant).
pub const MIN_RECENT_TOOL_ENTRIES: usize = 5;
/// Token quantum boundary for cache-stable chunked context eviction.
pub const CACHE_QUANTUM_TOKENS: usize = 512;

/// Chunked cache-stable context eviction matching OpenAI Codex transcript-window semantics.
///
/// Instead of evicting single turns (which destroys prefix KV-cache reuse on every turn),
/// eviction rounds up to stable chunk quanta using `div_ceil(2)` steps.
/// Guarantees that at least `MIN_RECENT_TOOL_ENTRIES = 5` tool outputs are preserved.
pub fn chunked_cache_stable_prune(
    messages: &[ChatMessage],
    max_tokens: usize,
    tokenizer: &dyn Tokenizer,
) -> Vec<ChatMessage> {
    let current_tokens: usize = messages
        .iter()
        .map(|m| tokenizer.count_tokens(m.content.as_deref().unwrap_or("")))
        .sum();

    if current_tokens <= max_tokens {
        return messages.to_vec();
    }

    let excess_tokens = current_tokens.saturating_sub(max_tokens);
    let chunk_eviction_target = excess_tokens.div_ceil(2).max(CACHE_QUANTUM_TOKENS);

    // 1. First apply historic tool pruning, strictly guaranteeing the 5 most recent tool entries
    let pruned = prune_historic_tool_actions(messages, MIN_RECENT_TOOL_ENTRIES);
    let after_tool_prune_tokens: usize = pruned
        .iter()
        .map(|m| tokenizer.count_tokens(m.content.as_deref().unwrap_or("")))
        .sum();

    if after_tool_prune_tokens <= max_tokens {
        return pruned;
    }

    // 2. If still over budget, evict historical message turns in stable chunks
    let system_prefix_count = pruned
        .iter()
        .take_while(|m| m.role == m31a_core::types::Role::System)
        .count();

    let mut remaining = Vec::with_capacity(pruned.len());
    remaining.extend_from_slice(&pruned[..system_prefix_count]);

    let rest = &pruned[system_prefix_count..];
    let mut evicted_tokens = 0;
    let mut evict_idx = 0;

    while evict_idx < rest.len() && evicted_tokens < chunk_eviction_target {
        let msg_tokens = tokenizer.count_tokens(rest[evict_idx].content.as_deref().unwrap_or(""));
        evicted_tokens += msg_tokens;
        evict_idx += 1;
    }

    // Find index of 5th most recent tool message to ensure we never evict recent tool outputs
    let tool_indices_in_rest: Vec<usize> = rest
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == m31a_core::types::Role::Tool)
        .map(|(i, _)| i)
        .collect();

    let tool_safe_limit = if tool_indices_in_rest.len() >= MIN_RECENT_TOOL_ENTRIES {
        tool_indices_in_rest[tool_indices_in_rest.len() - MIN_RECENT_TOOL_ENTRIES]
    } else {
        0
    };

    // Ensure we do not evict recent tool entries or the last turn
    let safe_evict_idx = evict_idx
        .min(tool_safe_limit)
        .min(rest.len().saturating_sub(2));
    remaining.extend_from_slice(&rest[safe_evict_idx..]);
    remaining
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_config_default() {
        let config = CompactionConfig::default();
        assert_eq!(config.threshold_pct, 0.8);
        assert_eq!(config.min_tokens, 1000);
    }

    #[test]
    fn test_char_tokenizer() {
        let tokenizer = CharTokenizer;
        assert_eq!(tokenizer.count_tokens("hello"), 2); // 5 chars / 4 = 1.25 -> 2
        assert_eq!(tokenizer.count_tokens(""), 0);
    }

    #[test]
    fn test_subword_tokenizer() {
        let tokenizer = SubwordTokenizer;
        let count =
            tokenizer.count_tokens("fn calculate_metrics(user_id: &str) -> Result<u64> { Ok(42) }");
        assert!(count >= 10);
        assert_eq!(tokenizer.count_tokens(""), 0);
    }

    #[test]
    fn test_storage_compaction_integration() {
        let storage = m31a_storage::Storage::open_in_memory().unwrap();
        let session_id = "test-compaction-session";
        let now = chrono::Utc::now();
        let meta = m31a_core::types::SessionMetadata {
            id: session_id.to_string(),
            title: "Test Session".to_string(),
            model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
            cwd: "/tmp".to_string(),
            created_at: now,
            updated_at: now,
            extra: std::collections::HashMap::new(),
        };
        storage
            .save_thread(&meta, "rollout.jsonl", "workspace-write", "on-request")
            .unwrap();

        let messages = vec![
            ChatMessage::user("How do I implement caching?"),
            ChatMessage::assistant(Some("Use an LRU cache with eviction.".to_string()), None),
        ];

        storage
            .replace_history(session_id, messages.clone())
            .unwrap();

        let retrieved = storage.get_history(session_id).unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(
            retrieved[0].content.as_deref(),
            Some("How do I implement caching?")
        );

        let summary = ConversationSummary {
            id: "sum-1".to_string(),
            original_turns: 2,
            original_tokens: 30,
            summary_tokens: 10,
            summary_text: "Discussed LRU caching mechanism.".to_string(),
            key_decisions: vec!["Adopt LRU".to_string()],
            files_modified: vec![],
            tools_used: vec![],
            timestamp: chrono::Utc::now(),
        };

        storage.store_summary(session_id, summary.clone()).unwrap();
        let summaries = storage.get_summaries(session_id).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(
            summaries[0].summary_text,
            "Discussed LRU caching mechanism."
        );
    }

    #[test]
    fn test_prune_historic_tool_actions() {
        let tool1 = ChatMessage::tool_response("tool-1", "A".repeat(500));
        let tool2 = ChatMessage::tool_response("tool-2", "B".repeat(500));
        let tool3 = ChatMessage::tool_response("tool-3", "C".repeat(500));

        let mut assistant1 = ChatMessage::assistant(Some("Step 1 done".into()), None);
        assistant1.reasoning_content = Some("Deep chain of thought...".repeat(20));

        let assistant2 = ChatMessage::assistant(Some("Step 2 done".into()), None);

        let messages = vec![
            ChatMessage::user("Do task"),
            assistant1,
            tool1,
            ChatMessage::user("Next"),
            tool2,
            ChatMessage::user("Final"),
            tool3,
            assistant2,
        ];

        // Keep 1 recent tool action
        let pruned = prune_historic_tool_actions(&messages, 1);
        assert_eq!(pruned.len(), 8);

        // tool1 and tool2 should be pruned
        assert!(
            pruned[2]
                .content
                .as_ref()
                .unwrap()
                .contains("pruned for context efficiency")
        );
        assert!(
            pruned[4]
                .content
                .as_ref()
                .unwrap()
                .contains("pruned for context efficiency")
        );

        // tool3 is the most recent tool and should be intact
        assert_eq!(pruned[6].content.as_ref().unwrap().len(), 500);

        // earlier assistant reasoning should be stripped
        assert!(pruned[1].reasoning_content.is_none());
    }

    #[test]
    fn test_chunked_cache_stable_prune() {
        let tokenizer = CharTokenizer;
        let mut messages = Vec::new();
        messages.push(ChatMessage::system("System instructions"));
        for i in 0..10 {
            messages.push(ChatMessage::user(format!(
                "User query {i}: {}",
                "X".repeat(200)
            )));
            messages.push(ChatMessage::tool_response(
                format!("tool-{i}"),
                format!("Result: {}", "Y".repeat(400)),
            ));
        }
        messages.push(ChatMessage::user("Final request"));

        // Prune to a budget of 500 tokens
        let pruned = chunked_cache_stable_prune(&messages, 500, &tokenizer);

        // System prompt must always be preserved
        assert_eq!(pruned[0].role, m31a_core::types::Role::System);
        // Minimum 5 recent tool entries are guaranteed preserved
        let tool_count = pruned
            .iter()
            .filter(|m| m.role == m31a_core::types::Role::Tool)
            .count();
        assert!(tool_count >= 5);
        // The final request should be preserved
        assert_eq!(
            pruned.last().unwrap().content.as_deref().unwrap(),
            "Final request"
        );
    }
}
