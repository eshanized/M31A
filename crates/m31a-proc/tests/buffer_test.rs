//! Dedicated integration tests for `m31a-proc` (src/buffer.rs).
//!
//! Validates:
//! - Circular buffer capacity bounds and dropping of oldest lines
//! - Output stream classifications (`Stdout`, `Stderr`, `System`)
//! - Tail retrieval and index-based slicing mechanics
//! - Text serialization with stream prefix annotations

use m31a_proc::buffer::{CircularOutputBuffer, OutputStream};

#[test]
fn test_circular_buffer_capacity_and_eviction() {
    let mut buf = CircularOutputBuffer::new(3);
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());

    buf.push(OutputStream::Stdout, "Line 0".to_string());
    buf.push(OutputStream::Stdout, "Line 1".to_string());
    buf.push(OutputStream::Stdout, "Line 2".to_string());
    assert_eq!(buf.len(), 3);
    assert_eq!(buf.total_lines(), 3);

    // Evicts Line 0, retains Line 1, 2, 3
    buf.push(OutputStream::Stdout, "Line 3".to_string());
    assert_eq!(buf.len(), 3);
    assert_eq!(buf.total_lines(), 4);

    let tail = buf.tail(10);
    assert_eq!(tail.len(), 3);
    assert_eq!(tail[0].line, "Line 1");
    assert_eq!(tail[0].index, 1);
    assert_eq!(tail[1].line, "Line 2");
    assert_eq!(tail[2].line, "Line 3");
}

#[test]
fn test_buffer_slicing_with_offset() {
    let mut buf = CircularOutputBuffer::new(10);
    for i in 0..10 {
        buf.push(OutputStream::Stdout, format!("Log entry {:02}", i));
    }

    let (slice_from_5, total) = buf.slice(5, 3);
    assert_eq!(total, 10);
    assert_eq!(slice_from_5.len(), 3);
    assert_eq!(slice_from_5[0].line, "Log entry 05");
    assert_eq!(slice_from_5[1].line, "Log entry 06");
    assert_eq!(slice_from_5[2].line, "Log entry 07");

    // Offset beyond total lines
    let (slice_beyond, _) = buf.slice(20, 5);
    assert!(slice_beyond.is_empty());
}

#[test]
fn test_as_text_stream_formatting() {
    let mut buf = CircularOutputBuffer::new(10);
    buf.push(OutputStream::Stdout, "Normal stdout".to_string());
    buf.push(OutputStream::Stderr, "Warning on stderr".to_string());
    buf.push(OutputStream::System, "Process started".to_string());

    let text = buf.as_text(10);
    assert!(text.contains("Normal stdout\n"));
    assert!(text.contains("[stderr] Warning on stderr\n"));
    assert!(text.contains("[sys] Process started\n"));
}

#[test]
fn test_zero_capacity_fallback() {
    let buf = CircularOutputBuffer::new(0);
    assert_eq!(buf.len(), 0);
    // Should accept pushes without crashing or clamping to 0
    let mut b = buf;
    b.push(OutputStream::Stdout, "test".to_string());
    assert_eq!(b.len(), 1);
}
