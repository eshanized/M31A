//! Windows Console Pseudo-console (ConPTY) backend surface.
//!
//! ConPTY is the native terminal facility: a pseudo-console object backs a
//! terminal session with dimensions, input, output, resize, and termination.
//! Raw handle plumbing is Windows-only; capability types and availability
//! reasoning compile everywhere so contract tests stay meaningful.

use crate::platform::capabilities::CapabilityState;

/// ConPTY session dimensions in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsoleSize {
    pub cols: u16,
    pub rows: u16,
}

impl ConsoleSize {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }

    /// Validate against the platform minimum (one cell each dimension).
    pub fn valid(&self) -> bool {
        self.cols > 0 && self.rows > 0
    }
}

/// Whether the ConPTY facility is present on the executing host.
pub fn conpty_available() -> bool {
    #[cfg(windows)]
    {
        native::conpty_present()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Terminal capability profile for Windows.
pub fn terminal_capabilities() -> crate::platform::terminal::TerminalCapabilities {
    use crate::platform::terminal::{TerminalBackend, TerminalCapabilities};
    if conpty_available() {
        TerminalCapabilities {
            backend: TerminalBackend::WindowsConPty,
            raw_mode: CapabilityState::Available,
            dimensions: CapabilityState::Available,
            pty: CapabilityState::Available,
        }
    } else {
        TerminalCapabilities {
            backend: TerminalBackend::WindowsConPty,
            raw_mode: CapabilityState::Degraded,
            dimensions: CapabilityState::Available,
            pty: CapabilityState::Unsupported,
        }
    }
}

#[cfg(windows)]
pub mod native {
    use super::ConsoleSize;

    pub type Hpc = *mut std::ffi::c_void;
    pub type HResult = i32;

    unsafe extern "system" {
        fn CreatePseudoConsole(
            size: u32,
            input: *mut std::ffi::c_void,
            output: *mut std::ffi::c_void,
            flags: u32,
            console: *mut Hpc,
        ) -> HResult;
        fn ResizePseudoConsole(console: Hpc, size: u32) -> HResult;
        fn ClosePseudoConsole(console: Hpc);
    }

    fn pack_size(size: ConsoleSize) -> u32 {
        ((size.cols as u32) << 16) | (size.rows as u32)
    }

    /// Probe for the ConPTY entry point without creating a console.
    /// Loads `kernel32` and resolves `CreatePseudoConsole` by name so the
    /// check never executes privileged or destructive behavior.
    pub fn conpty_present() -> bool {
        use std::ffi::CString;
        unsafe extern "system" {
            fn GetModuleHandleA(name: *const u8) -> *mut std::ffi::c_void;
            fn GetProcAddress(
                module: *mut std::ffi::c_void,
                name: *const u8,
            ) -> *mut std::ffi::c_void;
        }
        let _ = (CString::new(""), pack_size as fn(ConsoleSize) -> u32);
        #[allow(clippy::manual_c_str_literals)]
        let kernel = unsafe { GetModuleHandleA(b"kernel32.dll\0".as_ptr()) };
        if kernel.is_null() {
            return false;
        }
        let name = b"CreatePseudoConsole\0";
        #[allow(clippy::manual_c_str_literals)]
        let addr = unsafe { GetProcAddress(kernel, name.as_ptr()) };
        !addr.is_null()
    }

    /// Owned pseudo-console handle.
    pub struct PseudoConsole {
        raw: Hpc,
    }

    impl PseudoConsole {
        /// Create a pseudo-console of `size`. Callers supply connected pipe
        /// handles; this constructor performs no I/O beyond creation.
        ///
        /// # Safety
        /// Callers must ensure `input` and `output` are valid OS handle pointers.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub unsafe fn create(
            size: ConsoleSize,
            input: *mut std::ffi::c_void,
            output: *mut std::ffi::c_void,
        ) -> Result<Self, HResult> {
            let mut raw: Hpc = std::ptr::null_mut();
            let code = unsafe { CreatePseudoConsole(pack_size(size), input, output, 0, &mut raw) };
            if code == 0 && !raw.is_null() {
                Ok(Self { raw })
            } else {
                Err(code)
            }
        }

        /// Resize the console. Returns the raw result code.
        pub fn resize(&self, size: ConsoleSize) -> HResult {
            unsafe { ResizePseudoConsole(self.raw, pack_size(size)) }
        }
    }

    impl Drop for PseudoConsole {
        fn drop(&mut self) {
            unsafe { ClosePseudoConsole(self.raw) };
        }
    }

    #[allow(dead_code)]
    pub fn _touch_types(_c: ConsoleSize) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_size_validation() {
        assert!(ConsoleSize::new(80, 24).valid());
        assert!(!ConsoleSize::new(0, 24).valid());
    }

    #[test]
    fn off_windows_conpty_absent() {
        assert!(!conpty_available());
    }
}
