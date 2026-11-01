//! Progressive 5-level transcript degradation for context compaction (inspired by OpenCode's `TRANSCRIPT_LEVELS`).
//!
//! When compressing massive histories, the conversation transcript itself can overflow
//! the summarizer model's context window. This module degrades transcripts progressively across 5 levels:
//! - Level 0: Full transcript intact.
//! - Level 1: Tool outputs clipped to 2,000 characters.
//! - Level 2: Tool outputs omitted entirely; tool inputs clipped to 2,000 characters.
//! - Level 3: Only user messages retained, clipped to 2,000 characters.
//! - Level 4: History omitted, preserving only base instructions and latest request.

use m31a_core::types::{ChatMessage, Role};

pub const DEFAULT_CLIP_CHARS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TranscriptDegradationLevel {
    /// Level 0: Unmodified conversation transcript.
    #[default]
    Level0Full,
    /// Level 1: Tool execution outputs clipped to `max_chars`.
    Level1ToolOutputsClipped { max_chars: usize },
    /// Level 2: Tool outputs omitted entirely; tool inputs/call arguments clipped.
    Level2ToolOutputsRemoved { max_input_chars: usize },
    /// Level 3: Tool calls/outputs removed; only user messages retained, clipped to `max_chars`.
    Level3UserOnly { max_chars: usize },
    /// Level 4: All intermediate turns dropped, keeping only initial instructions and the latest user prompt.
    Level4Omitted,
}

pub const PROGRESSIVE_LEVELS: [TranscriptDegradationLevel; 5] = [
    TranscriptDegradationLevel::Level0Full,
    TranscriptDegradationLevel::Level1ToolOutputsClipped {
        max_chars: DEFAULT_CLIP_CHARS,
    },
    TranscriptDegradationLevel::Level2ToolOutputsRemoved {
        max_input_chars: DEFAULT_CLIP_CHARS,
    },
    TranscriptDegradationLevel::Level3UserOnly {
        max_chars: DEFAULT_CLIP_CHARS,
    },
    TranscriptDegradationLevel::Level4Omitted,
];

/// Degrades a message transcript according to the requested degradation tier.
pub fn degrade_transcript(
    messages: &[ChatMessage],
    level: TranscriptDegradationLevel,
) -> Vec<ChatMessage> {
    if messages.is_empty() {
        return Vec::new();
    }

    match level {
        TranscriptDegradationLevel::Level0Full => messages.to_vec(),

        TranscriptDegradationLevel::Level1ToolOutputsClipped { max_chars } => messages
            .iter()
            .map(|m| {
                if m.role == Role::Tool {
                    let mut truncated = m.clone();
                    if let Some(ref text) = truncated.content
                        && text.chars().count() > max_chars
                    {
                        let clipped: String = text.chars().take(max_chars).collect();
                        truncated.content = Some(format!("{}... [output clipped]", clipped));
                    }
                    truncated
                } else {
                    m.clone()
                }
            })
            .collect(),

        TranscriptDegradationLevel::Level2ToolOutputsRemoved { max_input_chars } => messages
            .iter()
            .filter(|m| m.role != Role::Tool)
            .map(|m| {
                if m.role == Role::Assistant {
                    let mut clipped_assistant = m.clone();
                    if let Some(ref mut calls) = clipped_assistant.tool_calls {
                        for call in calls {
                            if call.function.arguments.chars().count() > max_input_chars {
                                let clipped: String = call
                                    .function
                                    .arguments
                                    .chars()
                                    .take(max_input_chars)
                                    .collect();
                                call.function.arguments = format!("{}... [args clipped]", clipped);
                            }
                        }
                    }
                    clipped_assistant
                } else {
                    m.clone()
                }
            })
            .collect(),

        TranscriptDegradationLevel::Level3UserOnly { max_chars } => messages
            .iter()
            .filter(|m| m.role == Role::User)
            .map(|m| {
                let mut clipped_user = m.clone();
                if let Some(ref text) = clipped_user.content
                    && text.chars().count() > max_chars
                {
                    let clipped: String = text.chars().take(max_chars).collect();
                    clipped_user.content = Some(format!("{}... [user text clipped]", clipped));
                }
                clipped_user
            })
            .collect(),

        TranscriptDegradationLevel::Level4Omitted => {
            let mut minimal = Vec::new();
            if let Some(first) = messages.first() {
                minimal.push(first.clone());
            }
            if messages.len() > 1
                && let Some(last) = messages.last()
            {
                minimal.push(last.clone());
            }
            minimal
        }
    }
}

/// Standardized structured summary template matching OpenCode's `SUMMARY_TEMPLATE`.
pub const STRUCTURED_SUMMARY_TEMPLATE: &str = r#"Summarize the conversation history concisely for context window compaction.
Strictly format your response with the following Markdown sections:

## Objective
[1-2 sentences stating the user's primary goal]

## Requirements & Scope Boundaries
[Non-negotiable constraints, rules, or requirements]

## Key Decisions Made
[Architectural choices and why alternatives were rejected]

## Work State
### Completed
- [x] Tasks completed
### Active / In-Flight
- [ ] Currently active work
### Blocked
- [ ] Any current blockers

## Next Moves
1. Explicit next action items

## Relevant Files
- `path/to/file` (Brief note on its role)

## Important Context
[Critical domain facts, environment variables, or discovered configurations needed to continue]"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level1_clipping() {
        let long_output = "a".repeat(5000);
        let msgs = vec![
            ChatMessage::user("run cargo build"),
            ChatMessage::tool_response("call_1", long_output),
        ];

        let degraded = degrade_transcript(
            &msgs,
            TranscriptDegradationLevel::Level1ToolOutputsClipped { max_chars: 100 },
        );
        assert_eq!(degraded.len(), 2);
        assert!(
            degraded[1]
                .content
                .as_ref()
                .unwrap()
                .contains("[output clipped]")
        );
        assert!(degraded[1].content.as_ref().unwrap().len() < 200);
    }

    #[test]
    fn test_level2_tool_removal() {
        let msgs = vec![
            ChatMessage::user("find file"),
            ChatMessage::tool_response("call_1", "found it"),
            ChatMessage::assistant(Some("Done".into()), None),
        ];

        let degraded = degrade_transcript(
            &msgs,
            TranscriptDegradationLevel::Level2ToolOutputsRemoved {
                max_input_chars: 100,
            },
        );
        assert_eq!(degraded.len(), 2);
        assert_eq!(degraded[0].role, Role::User);
        assert_eq!(degraded[1].role, Role::Assistant);
    }

    #[test]
    fn test_level3_user_only() {
        let msgs = vec![
            ChatMessage::system("instructions"),
            ChatMessage::user("build project"),
            ChatMessage::assistant(Some("Building...".into()), None),
            ChatMessage::tool_response("c1", "ok"),
            ChatMessage::user("test project"),
        ];

        let degraded = degrade_transcript(
            &msgs,
            TranscriptDegradationLevel::Level3UserOnly { max_chars: 50 },
        );
        assert_eq!(degraded.len(), 2);
        assert_eq!(degraded[0].content.as_deref(), Some("build project"));
        assert_eq!(degraded[1].content.as_deref(), Some("test project"));
    }

    #[test]
    fn test_level4_omitted() {
        let msgs = vec![
            ChatMessage::system("base prompt"),
            ChatMessage::user("step 1"),
            ChatMessage::assistant(Some("done 1".into()), None),
            ChatMessage::user("step 2"),
        ];

        let degraded = degrade_transcript(&msgs, TranscriptDegradationLevel::Level4Omitted);
        assert_eq!(degraded.len(), 2);
        assert_eq!(degraded[0].content.as_deref(), Some("base prompt"));
        assert_eq!(degraded[1].content.as_deref(), Some("step 2"));
    }
}
