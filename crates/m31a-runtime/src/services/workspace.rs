//! Workspace trust evaluation, directory boundary authorization, and file watching.

use crate::error::RuntimeError;
use m31a_trust::{TrustDecision, TrustStore};
use m31a_watcher::WorkspaceWatcher;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Initialize a thread-safe trust store.
pub fn init_trust_store() -> Arc<Mutex<TrustStore>> {
    Arc::new(Mutex::new(TrustStore::new()))
}

/// Verify workspace directory trust authorization against security policy.
pub fn verify_workspace_trust(
    trust_store: &Arc<Mutex<TrustStore>>,
    cwd: &Path,
    allow_session_auto_auth: bool,
) -> Result<(), RuntimeError> {
    let mut store = trust_store
        .lock()
        .map_err(|_| RuntimeError::Security("Failed to acquire trust store lock".to_string()))?;

    let decision = store.evaluate(cwd);
    match decision {
        TrustDecision::Denied { path } => Err(RuntimeError::Security(format!(
            "Execution DENIED: Working directory '{}' is explicitly marked as Denied by security trust policy.",
            path.display()
        ))),
        TrustDecision::Trusted | TrustDecision::SessionOnly => Ok(()),
        TrustDecision::RequiresConfirmation { path, reason } => {
            if allow_session_auto_auth {
                store
                    .authorize(cwd, false, true)
                    .map_err(|e| RuntimeError::Security(e.to_string()))?;
                Ok(())
            } else {
                Err(RuntimeError::Security(format!(
                    "Execution REJECTED: Working directory '{}' is untrusted ({reason}). Explicit authorization is required.",
                    path.display()
                )))
            }
        }
    }
}

/// Initialize the workspace watcher if requested.
pub fn init_workspace_watcher(
    cwd: &Path,
    enabled: bool,
) -> Result<Option<Arc<WorkspaceWatcher>>, RuntimeError> {
    if !enabled {
        return Ok(None);
    }

    let watcher = WorkspaceWatcher::new(cwd.to_path_buf(), None);
    Ok(Some(Arc::new(watcher)))
}
