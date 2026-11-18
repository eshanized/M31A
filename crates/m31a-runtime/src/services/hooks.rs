//! Lifecycle hooks runtime discovery and dispatch.

use m31a_hooks::{HookConfig, HookRuntime};
use std::path::Path;
use std::sync::Arc;

/// Discover and initialize the hook runtime for the given workspace directory.
pub fn init_hooks(cwd: &Path) -> Arc<HookRuntime> {
    let config = HookConfig::discover(cwd);
    Arc::new(HookRuntime::new(config))
}
