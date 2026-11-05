//! Platform-agnostic terminal clipboard integration.
//!
//! Provides support for:
//! - Terminal OSC 52 escape codes (supported by Alacritty, Kitty, iTerm2, WezTerm, tmux, etc.)
//! - Wayland (`wl-copy`)
//! - X11 (`xclip`, `xsel`)
//! - macOS (`pbcopy`)
//! - Windows (`clip.exe` / PowerShell)

use base64::Engine;
use std::io::Write;

/// Outcome of a clipboard copy operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardResult {
    /// Native OS clipboard CLI tool wrote content successfully.
    OsClipboardSuccess,
    /// Terminal OSC 52 escape sequence was emitted (terminal may or may not support/allow it).
    Osc52EmittedOnly,
    /// Write failed or text was empty.
    WriteFailed,
    /// Backend tool was unavailable.
    Unavailable,
}

impl ClipboardResult {
    pub fn is_os_success(&self) -> bool {
        matches!(self, ClipboardResult::OsClipboardSuccess)
    }

    pub fn is_any_success(&self) -> bool {
        matches!(
            self,
            ClipboardResult::OsClipboardSuccess | ClipboardResult::Osc52EmittedOnly
        )
    }

    pub fn user_notice(&self) -> (&'static str, crate::transcript::NoticeLevel) {
        match self {
            ClipboardResult::OsClipboardSuccess => (
                "Copied code block to system clipboard.",
                crate::transcript::NoticeLevel::Success,
            ),
            ClipboardResult::Osc52EmittedOnly => (
                "Emitted OSC 52 clipboard escape sequence (system clipboard CLI unavailable).",
                crate::transcript::NoticeLevel::Info,
            ),
            ClipboardResult::WriteFailed => (
                "Failed to copy to clipboard.",
                crate::transcript::NoticeLevel::Error,
            ),
            ClipboardResult::Unavailable => (
                "No clipboard backend available.",
                crate::transcript::NoticeLevel::Warning,
            ),
        }
    }
}

/// Formats an OSC 52 terminal clipboard escape sequence for the given text.
pub fn format_osc52(text: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(text);
    format!("\x1b]52;c;{}\x07", b64)
}

/// Copies the provided text to the system clipboard, distinguishing real OS clipboard success
/// from OSC 52 emission and failures.
pub fn copy_to_clipboard(text: &str) -> ClipboardResult {
    if text.is_empty() {
        return ClipboardResult::WriteFailed;
    }

    // 1. Try OS-specific clipboard CLI tools first
    #[cfg(target_os = "linux")]
    {
        // Try wl-copy first for Wayland sessions
        if std::env::var_os("WAYLAND_DISPLAY").is_some()
            && let Ok(mut child) = std::process::Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait()
                && status.success()
            {
                return ClipboardResult::OsClipboardSuccess;
            }
        }

        // Try xclip for X11 sessions
        if let Ok(mut child) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait()
                && status.success()
            {
                return ClipboardResult::OsClipboardSuccess;
            }
        }

        // Fallback to xsel
        if let Ok(mut child) = std::process::Command::new("xsel")
            .args(["--clipboard", "--input"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait()
                && status.success()
            {
                return ClipboardResult::OsClipboardSuccess;
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(mut child) = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait()
                && status.success()
            {
                return ClipboardResult::OsClipboardSuccess;
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(mut child) = std::process::Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait()
                && status.success()
            {
                return ClipboardResult::OsClipboardSuccess;
            }
        }
    }

    // 2. Fallback to emitting OSC 52 sequence to stdout
    let osc52 = format_osc52(text);
    let mut stdout = std::io::stdout();
    if stdout.write_all(osc52.as_bytes()).is_ok() && stdout.flush().is_ok() {
        ClipboardResult::Osc52EmittedOnly
    } else {
        ClipboardResult::Unavailable
    }
}

/// Copies text and returns true only if an OS or OSC 52 clipboard attempt was made successfully.
pub fn copy_to_clipboard_bool(text: &str) -> bool {
    copy_to_clipboard(text).is_any_success()
}

/// Reads the current system clipboard content using available platform CLI utilities.
pub fn read_from_clipboard() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some()
            && let Ok(output) = std::process::Command::new("wl-paste")
                .arg("--no-newline")
                .output()
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
            && !text.is_empty()
        {
            return Some(text);
        }

        if let Ok(output) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
            && !text.is_empty()
        {
            return Some(text);
        }

        if let Ok(output) = std::process::Command::new("xsel")
            .args(["--clipboard", "--output"])
            .output()
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
            && !text.is_empty()
        {
            return Some(text);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("pbpaste").output()
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
            && !text.is_empty()
        {
            return Some(text);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "Get-Clipboard"])
            .output()
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
            && !text.is_empty()
        {
            return Some(text);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osc52_formatting() {
        let osc52 = format_osc52("test content");
        assert_eq!(osc52, "\x1b]52;c;dGVzdCBjb250ZW50\x07");
    }

    #[test]
    fn test_clipboard_empty_fails() {
        let res = copy_to_clipboard("");
        assert_eq!(res, ClipboardResult::WriteFailed);
    }

    #[test]
    fn test_clipboard_result_notices() {
        let (msg, level) = ClipboardResult::OsClipboardSuccess.user_notice();
        assert_eq!(level, crate::transcript::NoticeLevel::Success);
        assert!(msg.contains("system clipboard"));

        let (msg, level) = ClipboardResult::Osc52EmittedOnly.user_notice();
        assert_eq!(level, crate::transcript::NoticeLevel::Info);
        assert!(msg.contains("OSC 52"));
    }
}
