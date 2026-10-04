//! Interaction Input Parser and Deterministic Intent Classifier (PRD §01, CLI-01).
//!
//! "Input -> Interaction Parser -> Typed Application Action -> Application Command -> Runtime -> Events / State -> CLI/TUI projection."
//!
//! "Do NOT use an LLM to classify simple slash commands. Deterministic parser first."

use std::path::Path;

use crate::interaction::action::{ApplicationAction, InteractionIntent};
use crate::interaction::commands::SlashCommandRegistry;
use crate::interaction::mentions::MentionParser;
use crate::interaction::state::SessionPromptState;
use crate::tui::approval::ApprovalDecision;

/// Deterministic parser and classifier converting raw developer input into typed application actions.
pub struct InteractionParser {
    command_registry: std::sync::Arc<SlashCommandRegistry>,
    snapshot_handle: Option<crate::interaction::user_commands::CommandSnapshotHandle>,
}

impl Default for InteractionParser {
    fn default() -> Self {
        Self::new(SlashCommandRegistry::new_standard())
    }
}

impl InteractionParser {
    pub fn new(command_registry: impl Into<std::sync::Arc<SlashCommandRegistry>>) -> Self {
        Self {
            command_registry: command_registry.into(),
            snapshot_handle: None,
        }
    }

    pub fn with_snapshot_handle(
        handle: crate::interaction::user_commands::CommandSnapshotHandle,
    ) -> Self {
        let reg = handle.current_registry();
        Self {
            command_registry: reg,
            snapshot_handle: Some(handle),
        }
    }

    pub fn set_snapshot_handle(
        &mut self,
        handle: crate::interaction::user_commands::CommandSnapshotHandle,
    ) {
        self.command_registry = handle.current_registry();
        self.snapshot_handle = Some(handle);
    }

    pub fn set_command_registry(&mut self, registry: std::sync::Arc<SlashCommandRegistry>) {
        self.command_registry = registry;
        self.snapshot_handle = None;
    }

    /// Resolve active slash command registry (from snapshot handle if present, else fallback).
    pub fn active_registry(&self) -> std::sync::Arc<SlashCommandRegistry> {
        if let Some(ref handle) = self.snapshot_handle {
            handle.current_registry()
        } else {
            self.command_registry.clone()
        }
    }

    /// Access the underlying slash command registry.
    pub fn command_registry(&self) -> &SlashCommandRegistry {
        &self.command_registry
    }

    /// Access the shared slash command registry Arc.
    pub fn command_registry_arc(&self) -> &std::sync::Arc<SlashCommandRegistry> {
        &self.command_registry
    }

    /// Classifies user input into an explicit interaction intent.
    pub fn classify_intent(
        &self,
        input: &str,
        prompt_state: SessionPromptState,
        has_active_mission: bool,
    ) -> InteractionIntent {
        let trimmed = input.trim();

        // 1. Pending approval resolution takes priority when in AwaitingApproval state
        if prompt_state == SessionPromptState::AwaitingApproval {
            let lower = trimmed.to_lowercase();
            if lower == "y" || lower == "yes" || lower == "approve" {
                return InteractionIntent::Approval {
                    approved: true,
                    reason: None,
                };
            }
            if lower == "n" || lower == "no" || lower == "deny" {
                return InteractionIntent::Approval {
                    approved: false,
                    reason: Some("Operator denied action".to_string()),
                };
            }
            if lower == "c" || lower == "cancel" {
                return InteractionIntent::Cancellation;
            }
        }

        // 2. Slash command detection
        if trimmed.starts_with('/')
            && let Some((name, args)) = self.active_registry().parse_input(trimmed)
        {
            return InteractionIntent::SlashCommand { name, args };
        }

        // 3. Inspection phrasing detection
        let lower = trimmed.to_lowercase();
        let stripped = lower
            .trim_end_matches('.')
            .trim_end_matches('!')
            .trim_end_matches('?');

        if stripped == "show me the diff"
            || stripped == "show diff"
            || stripped == "diff"
            || stripped == "git diff"
        {
            return InteractionIntent::Inspection {
                target: "diff".to_string(),
            };
        }

        if stripped == "status" || stripped == "show status" || stripped == "git status" {
            return InteractionIntent::Inspection {
                target: "status".to_string(),
            };
        }

        if stripped == "commit it" || stripped == "commit" || stripped == "please commit" {
            return InteractionIntent::Commit { message: None };
        }

        if stripped == "cancel" || stripped == "stop" || stripped == "abort" {
            return InteractionIntent::Cancellation;
        }

        if stripped == "clear" {
            return InteractionIntent::SessionControl {
                action: "clear".to_string(),
            };
        }

        if stripped == "exit" || stripped == "quit" {
            return InteractionIntent::SessionControl {
                action: "exit".to_string(),
            };
        }

        // 4. Conversational inquiries
        if stripped == "hi"
            || stripped == "hello"
            || stripped == "help"
            || stripped == "who are you"
            || stripped == "what can you do"
        {
            return InteractionIntent::Conversation {
                text: trimmed.to_string(),
            };
        }

        // 5. Follow-up vs new mission boundary
        if has_active_mission {
            InteractionIntent::FollowUp {
                text: trimmed.to_string(),
            }
        } else {
            InteractionIntent::NewMission {
                prompt: trimmed.to_string(),
            }
        }
    }

    /// Parse user input string into a typed ApplicationAction for consumption by runtime/UI.
    pub fn parse(
        &self,
        input: &str,
        workspace_root: &Path,
        prompt_state: SessionPromptState,
        active_approval_id: Option<&str>,
        has_active_mission: bool,
    ) -> Option<ApplicationAction> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }

        let intent = self.classify_intent(trimmed, prompt_state, has_active_mission);

        match intent {
            InteractionIntent::Approval { approved, .. } => {
                let req_id = active_approval_id.unwrap_or("").to_string();
                let decision = if approved {
                    ApprovalDecision::ApproveOnce
                } else {
                    ApprovalDecision::Reject
                };
                Some(ApplicationAction::ApprovalDecision {
                    request_id: req_id,
                    decision,
                })
            }

            InteractionIntent::SlashCommand { name, args } => {
                let lower = name.to_lowercase();
                match lower.as_str() {
                    "diff" | "d" => Some(ApplicationAction::DiffRequested),
                    "commit" => {
                        let msg = if args.is_empty() {
                            None
                        } else {
                            Some(args.join(" "))
                        };
                        Some(ApplicationAction::CommitRequested { message: msg })
                    }
                    "status" | "st" => Some(ApplicationAction::StatusRequested),
                    "cancel" => Some(ApplicationAction::CancelRequested),
                    "clear" => Some(ApplicationAction::ClearRequested),
                    "clear-session" => Some(ApplicationAction::ClearSessionRequested),
                    "exit" | "quit" => Some(ApplicationAction::ExitRequested),
                    "resume" => {
                        let sid = args.first().cloned().unwrap_or_default();
                        Some(ApplicationAction::SessionResumeRequested { session_id: sid })
                    }
                    _ => Some(ApplicationAction::SlashCommandSubmitted {
                        command: name,
                        args,
                    }),
                }
            }

            InteractionIntent::Inspection { target } if target == "diff" => {
                Some(ApplicationAction::DiffRequested)
            }
            InteractionIntent::Inspection { target } if target == "status" => {
                Some(ApplicationAction::StatusRequested)
            }
            InteractionIntent::Commit { message } => {
                Some(ApplicationAction::CommitRequested { message })
            }
            InteractionIntent::Cancellation => Some(ApplicationAction::CancelRequested),

            InteractionIntent::SessionControl { action } if action == "clear" => {
                Some(ApplicationAction::ClearRequested)
            }
            InteractionIntent::SessionControl { action } if action == "exit" => {
                Some(ApplicationAction::ExitRequested)
            }

            InteractionIntent::FollowUp { text }
            | InteractionIntent::NewMission { prompt: text }
            | InteractionIntent::Conversation { text } => {
                let parsed = MentionParser::parse(&text, workspace_root);
                Some(ApplicationAction::UserTextSubmitted(parsed))
            }

            _ => {
                let parsed = MentionParser::parse(trimmed, workspace_root);
                Some(ApplicationAction::UserTextSubmitted(parsed))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_classify_slash_commands() {
        let parser = InteractionParser::default();
        let intent = parser.classify_intent("/diff", SessionPromptState::Idle, false);
        assert!(matches!(
            intent,
            InteractionIntent::SlashCommand { name, .. } if name == "diff"
        ));

        let intent2 = parser.classify_intent("/commit fix bug", SessionPromptState::Idle, false);
        assert!(matches!(
            intent2,
            InteractionIntent::SlashCommand { name, args } if name == "commit" && args == vec!["fix", "bug"]
        ));
    }

    #[test]
    fn test_classify_phrasal_inspection_and_commit() {
        let parser = InteractionParser::default();
        assert_eq!(
            parser.classify_intent("Show me the diff.", SessionPromptState::Idle, false),
            InteractionIntent::Inspection {
                target: "diff".to_string()
            }
        );
        assert_eq!(
            parser.classify_intent("commit it", SessionPromptState::Idle, false),
            InteractionIntent::Commit { message: None }
        );
    }

    #[test]
    fn test_classify_approval_in_awaiting_state() {
        let parser = InteractionParser::default();
        assert_eq!(
            parser.classify_intent("y", SessionPromptState::AwaitingApproval, true),
            InteractionIntent::Approval {
                approved: true,
                reason: None
            }
        );
        assert!(matches!(
            parser.classify_intent("n", SessionPromptState::AwaitingApproval, true),
            InteractionIntent::Approval {
                approved: false,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_into_typed_application_action() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let parser = InteractionParser::default();

        let action = parser
            .parse(
                "Show me the diff.",
                ws,
                SessionPromptState::Idle,
                None,
                false,
            )
            .unwrap();
        assert_eq!(action, ApplicationAction::DiffRequested);

        let action2 = parser
            .parse("Commit it.", ws, SessionPromptState::Idle, None, false)
            .unwrap();
        assert_eq!(
            action2,
            ApplicationAction::CommitRequested { message: None }
        );

        let action3 = parser
            .parse(
                "Fix the parser in @src/parser.rs",
                ws,
                SessionPromptState::Idle,
                None,
                false,
            )
            .unwrap();
        match action3 {
            ApplicationAction::UserTextSubmitted(parsed) => {
                assert_eq!(parsed.mentions.len(), 1);
                assert_eq!(parsed.mentions[0].raw_path, "src/parser.rs");
            }
            other => panic!("expected UserTextSubmitted, got {:?}", other),
        }
    }
}
