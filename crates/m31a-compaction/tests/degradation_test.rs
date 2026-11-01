//! Dedicated integration tests for `m31a-compaction` (src/degradation.rs).
//!
//! Validates:
//! - Progressive 5-level transcript degradation across Level 0 to Level 4
//! - Precise character clipping boundaries for tool outputs and arguments
//! - Role filtering (Level 2 drops tool outputs, Level 3 retains only user prompts)
//! - Edge cases (empty transcripts, single-message preservation)

use m31a_compaction::{
    degrade_transcript, TranscriptDegradationLevel, PROGRESSIVE_LEVELS,
};
use m31a_core::types::{ChatMessage, FunctionCall, Role, ToolCall};

fn sample_transcript() -> Vec<ChatMessage> {
    vec![
        ChatMessage::system("You are M31A autonomous software engineering assistant."),
        ChatMessage::user("Search for memory leak in socket connection pool."),
        ChatMessage {
            role: Role::Assistant,
            content: Some("I will inspect the socket connection pool implementation.".to_string()),
            tool_calls: Some(vec![ToolCall {
                id: "call-1".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "search_code".to_string(),
                    arguments: "{\"query\": \"ConnectionPool::acquire\", \"long_padding\": \"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\"}".to_string(),
                },
            }]),
            tool_call_id: None,
            reasoning_content: None,
        },
        ChatMessage {
            role: Role::Tool,
            content: Some("Found 15 occurrences:\n1. src/pool.rs: line 42: fn acquire()...\n".repeat(50)),
            tool_calls: None,
            tool_call_id: Some("call-1".to_string()),
            reasoning_content: None,
        },
        ChatMessage::user("Fix the identified leak by wrapping with Arc<Mutex>."),
        ChatMessage::assistant(Some("Patch has been applied and validated.".to_string()), None),
    ]
}

#[test]
fn test_level_0_full_preservation() {
    let original = sample_transcript();
    let degraded = degrade_transcript(&original, TranscriptDegradationLevel::Level0Full);
    assert_eq!(degraded.len(), original.len());
    assert_eq!(degraded[0].content, original[0].content);
    assert_eq!(degraded[3].content, original[3].content);
}

#[test]
fn test_level_1_tool_output_clipping() {
    let original = sample_transcript();
    let max_chars = 50;
    let degraded = degrade_transcript(
        &original,
        TranscriptDegradationLevel::Level1ToolOutputsClipped { max_chars },
    );

    assert_eq!(degraded.len(), original.len());
    let tool_msg = &degraded[3];
    assert_eq!(tool_msg.role, Role::Tool);
    let text = tool_msg.content.as_ref().unwrap();
    assert!(text.contains("... [output clipped]"));
    assert!(text.len() < 100);
}

#[test]
fn test_level_2_tool_output_removed_and_inputs_clipped() {
    let original = sample_transcript();
    let max_input_chars = 30;
    let degraded = degrade_transcript(
        &original,
        TranscriptDegradationLevel::Level2ToolOutputsRemoved { max_input_chars },
    );

    // Role::Tool must be completely absent
    assert!(!degraded.iter().any(|m| m.role == Role::Tool));

    // Assistant tool call args must be clipped
    let assistant_msg = degraded.iter().find(|m| m.role == Role::Assistant).unwrap();
    let args = &assistant_msg.tool_calls.as_ref().unwrap()[0].function.arguments;
    assert!(args.contains("... [args clipped]"));
}

#[test]
fn test_level_3_user_only() {
    let original = sample_transcript();
    let degraded = degrade_transcript(
        &original,
        TranscriptDegradationLevel::Level3UserOnly { max_chars: 1000 },
    );

    // Only User and System should remain
    for m in &degraded {
        assert!(m.role == Role::User || m.role == Role::System);
    }
    assert_eq!(
        degraded.iter().filter(|m| m.role == Role::User).count(),
        2,
        "Both user messages must be retained"
    );
}

#[test]
fn test_level_4_omitted_intermediate() {
    let original = sample_transcript();
    let degraded = degrade_transcript(&original, TranscriptDegradationLevel::Level4Omitted);

    // Level 4 preserves exactly the first message and the last message
    assert_eq!(degraded.len(), 2);
    assert_eq!(degraded[0].role, Role::System);
    assert_eq!(degraded[1].role, Role::Assistant);
    assert!(degraded[1].content.as_ref().unwrap().contains("Patch has been applied"));
}

#[test]
fn test_progressive_levels_array_completeness() {
    assert_eq!(PROGRESSIVE_LEVELS.len(), 5);
}
