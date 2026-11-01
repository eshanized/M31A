//! `m31a-composer`: Advanced multiline textarea, paste burst protection, and `@` mentions controller.

pub mod mentions;
pub mod paste_burst;
pub mod slash;
pub mod textarea;

pub use mentions::MentionsPopup;
pub use paste_burst::{PasteBurstDetector, PastedSnippetPill};
pub use slash::{
    BUILTIN_SLASH_COMMANDS, CANONICAL_SLASH_COMMANDS, CommandExecutionClass, ContextAvailability,
    ParsedSlashCommand, SlashCommand, SlashCompletionItem, SlashParseError, SlashPopup,
    find_command, get_slash_completions, matching_commands, parse_and_resolve, parse_slash_tokens,
    tokenize_slash_command,
};
pub use textarea::TextArea;
