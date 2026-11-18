//! Bubblewrap sandbox runner service.

use m31a_sandbox::SandboxRunner;

/// Initialize the bubblewrap sandbox runner service.
pub fn init_sandbox() -> SandboxRunner {
    SandboxRunner::new()
}
