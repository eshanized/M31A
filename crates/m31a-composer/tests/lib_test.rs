//! Dedicated integration tests for `m31a-composer` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports: `MentionsPopup`, `PasteBurstDetector`, `PastedSnippetPill`, `TextArea`, `CANONICAL_SLASH_COMMANDS`, `SlashCommand`, `SlashPopup`, `find_command`
//! - Cohesive editing workflow: textarea inputs triggering slash completion and paste burst detection

use m31a_composer::{
    CANONICAL_SLASH_COMMANDS, MentionsPopup, PasteBurstDetector, PastedSnippetPill, SlashPopup,
    TextArea, find_command,
};

#[test]
fn test_root_reexports() {
    assert!(!CANONICAL_SLASH_COMMANDS.is_empty());
    assert!(find_command("/help").is_some());

    let area = TextArea::new();
    assert_eq!(area.text(), "");

    let mentions = MentionsPopup::new();
    assert!(!mentions.is_active);

    let slash = SlashPopup::new();
    assert!(!slash.is_active);
}

#[test]
fn test_composer_interactive_pipeline() {
    let mut area = TextArea::new();
    let mut paste_detector = PasteBurstDetector::new();
    let mut slash_popup = SlashPopup::new();

    // 1. User types a slash command
    area.insert_str("/set");
    let current_line = &area.lines[area.cursor_row];
    slash_popup.update(current_line);
    assert!(slash_popup.is_active);
    assert!(!slash_popup.matches.is_empty());

    // 2. User pastes multiline code snippet into composer
    let code_snippet = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    let pill: Option<PastedSnippetPill> = paste_detector.flush_if_large_paste(code_snippet);
    assert!(pill.is_some());
    assert_eq!(pill.unwrap().line_count, 5);
}
