//! Integration tests for terminal capability detection, OSC 8 hyperlinks, and ANSI stripping.

use m31a_term::{
    PtyRunner, TerminalCapabilities, expand_tabs, format_file_hyperlink, format_hyperlink,
    has_hyperlink, strip_ansi,
};
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_strip_ansi_all_sequences() {
    // SGR colors and styles
    let colored = "\x1b[31;1mRed Bold\x1b[0m normal \x1b[32mGreen\x1b[0m";
    assert_eq!(strip_ansi(colored), "Red Bold normal Green");

    // OSC sequence
    let osc = "\x1b]0;My Window Title\x07Text content";
    assert_eq!(strip_ansi(osc), "Text content");

    // CSI cursor movement
    let csi = "\x1b[2J\x1b[HScreen Cleared";
    assert_eq!(strip_ansi(csi), "Screen Cleared");

    // Carriage return stripping
    let cr = "Line 1\r\nLine 2\r\n";
    assert_eq!(strip_ansi(cr), "Line 1\nLine 2\n");
}

#[test]
fn test_expand_tabs_custom_widths() {
    let input = "\tlet x = 1;\n\t\tlet y = 2;";

    // Width 4
    let expanded4 = expand_tabs(input, 4);
    assert_eq!(expanded4, "    let x = 1;\n        let y = 2;");

    // Width 2
    let expanded2 = expand_tabs(input, 2);
    assert_eq!(expanded2, "  let x = 1;\n    let y = 2;");
}

#[test]
fn test_terminal_capabilities_probe() {
    let caps = TerminalCapabilities::detect();
    assert!(!caps.name.is_empty());
    // Should detect valid color level
    let _color = caps.color_level;

    // Default trait
    let def = TerminalCapabilities::default();
    assert_eq!(def.name, caps.name);

    // Serialization
    let json = serde_json::to_string(&caps).unwrap();
    let deser: TerminalCapabilities = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.name, caps.name);
}

#[test]
fn test_hyperlinks_formatting_and_detection() {
    let link = format_hyperlink("https://m31a.dev/docs", "M31A Documentation");
    assert!(has_hyperlink(&link));
    assert!(link.contains("M31A Documentation"));

    let file_link = format_file_hyperlink(Path::new("src/lib.rs"), Some(10));
    assert!(has_hyperlink(&file_link));
    assert!(file_link.contains("src/lib.rs:10"));
    assert!(file_link.contains("#L10"));
}

#[test]
fn test_pty_runner_real_command() {
    let tmp = tempdir().unwrap();
    let res =
        PtyRunner::run_command("echo 'TERM_TEST_LINE'", tmp.path(), &[]).expect("run command");

    assert!(res.success);
    assert_eq!(res.exit_code, 0);
    assert!(res.stdout.contains("TERM_TEST_LINE"));
}
