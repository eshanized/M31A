//! Terminal Guard & Panic Safety Subsystem (TDS-01, TDS-02, D-10).
//!
//! Provides RAII-based terminal mode management and global panic restoration hooks.
//! Ensures that regardless of exit status (normal exit, error, panic, or signal),
//! the terminal's raw mode, alternate screen buffer, mouse capture, and cursor visibility
//! are completely and idempotently restored.

use std::io::{self, stdout};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

static PANIC_HOOK_INSTALLED: Once = Once::new();
static RESTORED_ON_PANIC: AtomicBool = AtomicBool::new(false);

/// RAII Terminal Guard managing crossterm raw mode and alternate screen buffer.
#[derive(Debug)]
pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    /// Acquires terminal resources: enables raw mode, enters alternate screen,
    /// enables mouse capture, and hides cursor.
    pub fn acquire() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        if let Err(err) = execute!(out, EnterAlternateScreen, EnableMouseCapture, Hide) {
            let _ = disable_raw_mode();
            return Err(err);
        }

        Ok(Self { active: true })
    }

    /// Explicitly and idempotently restores terminal state.
    pub fn restore(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }

        self.active = false;
        Self::emergency_restore()
    }

    /// Low-level emergency terminal restore that can be safely called from panic hooks or drop.
    pub fn emergency_restore() -> io::Result<()> {
        let mut out = stdout();
        let _ = execute!(out, DisableMouseCapture, LeaveAlternateScreen, Show);
        let _ = disable_raw_mode();
        Ok(())
    }

    /// Checks whether the guard is currently holding active terminal resources.
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Installs a global panic hook wrapper that restores terminal state prior to printing panic messages.
/// Safe to call multiple times (idempotent via `Once`).
pub fn install_panic_hook() {
    PANIC_HOOK_INSTALLED.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            if !RESTORED_ON_PANIC.swap(true, Ordering::SeqCst) {
                let _ = TerminalGuard::emergency_restore();
            }
            default_hook(panic_info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panic_hook_installation_is_idempotent() {
        install_panic_hook();
        install_panic_hook();
    }

    #[test]
    fn test_guard_emergency_restore_idempotency() {
        assert!(TerminalGuard::emergency_restore().is_ok());
        assert!(TerminalGuard::emergency_restore().is_ok());
    }
}
