//! Dedicated integration tests for `m31a-compaction` (src/lib.rs).
//!
//! Validates:
//! - Tokenizers (CharTokenizer approximation and SubwordTokenizer CamelCase/symbol parsing)
//! - Historic tool action pruning retaining recent outputs and stripping stale reasoning
//! - Chunked cache stable pruning protecting system prompts and recent tool actions
//! - Real SQLite storage compaction lifecycle (Zero-Mock via m31a-storage)

use m31a_compaction::{
    chunked_cache_stable_prune, prune_historic_tool_actions, CharTokenizer, CompactionStorage,
    ConversationSummary, SubwordTokenizer, Tokenizer,
};
use m31a_core::types::{ChatMessage, Role, SessionMetadata};
use m31a_storage::Storage;
use std::collections::HashMap;

#[test]
fn test_tokenizers_accuracy_and_bounds() {
    let char_tok = CharTokenizer;
    assert_eq!(char_tok.count_tokens(""), 0);
    assert_eq!(char_tok.count_tokens("1234"), 1);
    assert_eq!(char_tok.count_tokens("12345"), 2);

    let subword_tok = SubwordTokenizer;
    assert_eq!(subword_tok.count_tokens(""), 0);
    let code_snippet = "fn process_data(user_id: &str) -> Result<Vec<u8>> { Ok(vec![]) }";
    let token_count = subword_tok.count_tokens(code_snippet);
    assert!(
        token_count >= 12,
        "SubwordTokenizer should identify symbols and keywords, got: {}",
        token_count
    );
}

#[test]
fn test_prune_historic_tool_actions_retains_recent() {
    let mut assistant1 = ChatMessage::assistant(Some("Planning to search index.".to_string()), None);
    assistant1.reasoning_content = Some("Need to verify query syntax carefully...".to_string());

    let mut assistant2 = ChatMessage::assistant(Some("Planning to write results.".to_string()), None);
    assistant2.reasoning_content = Some("Need to make sure directory exists...".to_string());

    let tool1 = ChatMessage::tool_response("tool-call-1", "Old massive output ".repeat(100));
    let tool2 = ChatMessage::tool_response("tool-call-2", "Recent essential output");

    let messages = vec![
        ChatMessage::system("System instructions"),
        ChatMessage::user("Search files"),
        assistant1,
        tool1,
        ChatMessage::user("Write output"),
        assistant2,
        tool2,
        ChatMessage::assistant(Some("Done!".to_string()), None),
    ];

    // Retain only 1 recent tool action
    let pruned = prune_historic_tool_actions(&messages, 1);
    assert_eq!(pruned.len(), 8);

    // Oldest tool response must be replaced with stub
    let pruned_tool1 = &pruned[3];
    assert_eq!(pruned_tool1.role, Role::Tool);
    assert!(
        pruned_tool1.content.as_ref().unwrap().contains("pruned for context efficiency")
    );

    // Most recent tool response must be retained in full
    let preserved_tool2 = &pruned[6];
    assert_eq!(preserved_tool2.role, Role::Tool);
    assert_eq!(preserved_tool2.content.as_ref().unwrap(), "Recent essential output");

    // Earlier assistant reasoning must be cleared, while latest is kept
    assert!(pruned[2].reasoning_content.is_none());
}

#[test]
fn test_chunked_cache_stable_prune_safety_invariants() {
    let tokenizer = CharTokenizer;
    let mut messages = Vec::new();
    messages.push(ChatMessage::system("Core system instructions"));

    for i in 0..12 {
        messages.push(ChatMessage::user(format!("Turn {i}: {}", "Query".repeat(20))));
        messages.push(ChatMessage::tool_response(
            format!("tool-{i}"),
            format!("Output: {}", "Data".repeat(30)),
        ));
    }
    messages.push(ChatMessage::user("Final request that must not be evicted"));

    // Target eviction of 300 tokens
    let pruned = chunked_cache_stable_prune(&messages, 300, &tokenizer);

    // Invariant 1: System prompt is preserved
    assert_eq!(pruned[0].role, Role::System);
    assert_eq!(pruned[0].content.as_deref().unwrap(), "Core system instructions");

    // Invariant 2: At least 5 recent tool entries are preserved
    let tool_count = pruned.iter().filter(|m| m.role == Role::Tool).count();
    assert!(tool_count >= 5, "Expected >= 5 tool outputs, found {}", tool_count);

    // Invariant 3: The final turn is preserved
    assert_eq!(
        pruned.last().unwrap().content.as_deref().unwrap(),
        "Final request that must not be evicted"
    );
}

#[test]
fn test_real_sqlite_storage_compaction_integration() {
    let storage = Storage::open_in_memory().unwrap();
    let session_id = "sess-compaction-verify-01";
    let now = chrono::Utc::now();

    let meta = SessionMetadata {
        id: session_id.to_string(),
        title: "Session for Compaction Test".to_string(),
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        cwd: "/workspace".to_string(),
        created_at: now,
        updated_at: now,
        extra: HashMap::new(),
    };

    storage
        .save_thread(&meta, "rollout.jsonl", "workspace-write", "on-request")
        .unwrap();

    let history = vec![
        ChatMessage::user("What is the speed of light?"),
        ChatMessage::assistant(Some("299,792,458 m/s.".to_string()), None),
    ];

    storage.replace_history(session_id, history.clone()).unwrap();

    let retrieved = storage.get_history(session_id).unwrap();
    assert_eq!(retrieved.len(), 2);
    assert_eq!(retrieved[0].content.as_deref().unwrap(), "What is the speed of light?");

    // Store conversation summary
    let summary = ConversationSummary {
        id: "sum-speed-of-light".to_string(),
        original_turns: 2,
        original_tokens: 30,
        summary_tokens: 8,
        summary_text: "User asked for and received the speed of light in vacuum.".to_string(),
        key_decisions: vec!["Reference SI units".to_string()],
        files_modified: vec![],
        tools_used: vec![],
        timestamp: chrono::Utc::now(),
    };

    storage.store_summary(session_id, summary).unwrap();

    let summaries = storage.get_summaries(session_id).unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "sum-speed-of-light");
    assert!(summaries[0].summary_text.contains("speed of light"));
}
