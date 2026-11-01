//! Dedicated integration tests for `m31a-composer` (src/paste_burst.rs).
//!
//! Validates:
//! - Terminal bracketed paste burst interception
//! - Line count threshold triggering (>= 4 lines)
//! - Character length threshold triggering (>= 160 characters)
//! - Pill metadata synthesis (title derivation, preview generation, full content preservation)
//! - Rejection of short inline pastes below thresholds

use m31a_composer::paste_burst::PasteBurstDetector;

#[test]
fn test_bracketed_paste_multiline_trigger() {
    let mut detector = PasteBurstDetector::new();
    detector.start_bracketed_paste();
    detector.push_bracketed_str("fn compute() {\n");
    detector.push_bracketed_str("    let x = 10;\n");
    detector.push_bracketed_str("    let y = 20;\n");
    detector.push_bracketed_str("    x + y\n");
    detector.push_bracketed_str("}\n");

    let pill = detector.end_bracketed_paste();
    assert!(pill.is_some(), "5 lines must trigger a pasted snippet pill");

    let p = pill.unwrap();
    assert_eq!(p.line_count, 5);
    assert_eq!(p.title, "fn compute() {");
    assert!(p.preview.contains("let x = 10;"));
    assert!(p.full_content.contains("x + y"));
    assert!(!p.id.is_empty());
}

#[test]
fn test_bracketed_paste_character_threshold_trigger() {
    let mut detector = PasteBurstDetector::new();
    detector.start_bracketed_paste();
    // 1 single line with 200 characters
    let long_line = "A".repeat(200);
    detector.push_bracketed_str(&long_line);

    let pill = detector.end_bracketed_paste();
    assert!(pill.is_some(), "200 characters on 1 line exceeds 160 char threshold");

    let p = pill.unwrap();
    assert_eq!(p.line_count, 1);
    assert_eq!(p.title.len(), 30, "Title is capped at 30 chars");
    assert_eq!(p.full_content, long_line);
}

#[test]
fn test_small_paste_does_not_trigger_pill() {
    let mut detector = PasteBurstDetector::new();
    detector.start_bracketed_paste();
    detector.push_bracketed_str("git status\n");

    let pill = detector.end_bracketed_paste();
    assert!(pill.is_none(), "Single short command must not create pill");
}

#[test]
fn test_flush_if_large_paste_direct() {
    let mut detector = PasteBurstDetector::new();
    let code = "Line 1\nLine 2\nLine 3\nLine 4\n";
    let pill = detector.flush_if_large_paste(code);
    assert!(pill.is_some());
    assert_eq!(pill.unwrap().line_count, 4);
}
