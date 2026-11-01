//! Dedicated integration tests for `m31a-shell` (src/buffer.rs).
//!
//! Validates:
//! - Default buffer configuration invariants (lines and byte limits)
//! - Passthrough behavior when content satisfies line and byte constraints
//! - Head-tail line truncation with accurate truncation count announcements
//! - Byte budget clipping and boundary notifications
//! - Preservation of trailing newline ergonomics

use m31a_shell::buffer::HeadTailBuffer;

#[test]
fn test_buffer_passthrough_within_budget() {
    let buffer = HeadTailBuffer::default();

    let text = "First line\nSecond line\nThird line\n";
    let processed = buffer.process(text);
    assert_eq!(processed, text);

    let empty = "";
    assert_eq!(buffer.process(empty), "");
}

#[test]
fn test_head_tail_line_truncation_fidelity() {
    let buffer = HeadTailBuffer::new(3, 2, 1024 * 1024);
    let mut input = String::new();
    for i in 1..=20 {
        input.push_str(&format!("Line {:02}\n", i));
    }

    let output = buffer.process(&input);

    // Verify head lines (first 3)
    assert!(output.contains("Line 01"));
    assert!(output.contains("Line 02"));
    assert!(output.contains("Line 03"));

    // Verify middle lines are excluded
    assert!(!output.contains("Line 04"));
    assert!(!output.contains("Line 10"));
    assert!(!output.contains("Line 18"));

    // Verify tail lines (last 2: 19, 20)
    assert!(output.contains("Line 19"));
    assert!(output.contains("Line 20"));

    // Verify announcement specifies 15 lines truncated (20 - 3 - 2 = 15)
    assert!(output.contains("15 lines truncated to preserve token context"));
    assert!(output.ends_with('\n'), "Must preserve trailing newline");
}

#[test]
fn test_byte_budget_truncation() {
    // 10 lines allowed, but byte budget capped at 50 bytes
    let buffer = HeadTailBuffer::new(5, 5, 50);
    let input = "0123456789\n0123456789\n0123456789\n0123456789\n0123456789\n0123456789\n";

    let output = buffer.process(input);
    assert!(output.contains("output truncated by byte budget"));
    assert!(output.len() <= 120); // 50 bytes plus message
}

#[test]
fn test_exact_budget_boundaries() {
    let buffer = HeadTailBuffer::new(2, 2, 1024);
    let input = "Line 1\nLine 2\nLine 3\nLine 4\n";
    // Total lines = 4, exactly 2 + 2 = 4, no truncation should happen
    let output = buffer.process(input);
    assert_eq!(output, input);
}
