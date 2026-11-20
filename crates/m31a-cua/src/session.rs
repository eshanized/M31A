//! CUA Session Manager with lease controls, state_id tracking, and replay safety.

use crate::backend::{CuaBackend, SimulatedBackend};
use crate::error::CuaError;
use crate::model::{ActionResult, AppInfo, AppState, ElementAction, MouseAction};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages CUA session lifetime, state caching, and anti-replay guards.
#[derive(Clone)]
pub struct CuaSessionManager {
    backend: Arc<dyn CuaBackend>,
    active_state_id: Arc<RwLock<Option<String>>>,
    sent_actions_history: Arc<RwLock<HashSet<String>>>,
}

impl Default for CuaSessionManager {
    fn default() -> Self {
        Self::new(Arc::new(SimulatedBackend::default()))
    }
}

impl CuaSessionManager {
    /// Creates a new session manager with the provided accessibility backend.
    pub fn new(backend: Arc<dyn CuaBackend>) -> Self {
        Self {
            backend,
            active_state_id: Arc::new(RwLock::new(None)),
            sent_actions_history: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Enumerates running native applications.
    pub async fn list_apps(&self) -> Result<Vec<AppInfo>, CuaError> {
        self.backend.list_apps().await
    }

    /// Captures the accessible tree and updates the active session's state_id.
    pub async fn get_app_state(
        &self,
        app_id: Option<&str>,
        capture_background: bool,
    ) -> Result<AppState, CuaError> {
        let state = self.backend.get_app_state(app_id, capture_background).await?;
        let mut current_id = self.active_state_id.write().await;
        *current_id = Some(state.state_id.clone());
        Ok(state)
    }

    /// Dispatches an action with state_id validation and anti-replay guard.
    pub async fn perform_action(
        &self,
        app_id: &str,
        element_index: usize,
        action: ElementAction,
        state_id: &str,
    ) -> Result<ActionResult, CuaError> {
        // Anti-replay check
        let action_sig = format!("{}:{}:{:?}:{}", app_id, element_index, action, state_id);
        {
            let history = self.sent_actions_history.read().await;
            if history.contains(&action_sig) {
                return Err(CuaError::ReplayBlocked(action_sig));
            }
        }

        let result = self
            .backend
            .perform_element_action(app_id, element_index, &action, state_id)
            .await?;

        if result.success {
            let mut history = self.sent_actions_history.write().await;
            history.insert(action_sig);
            if let Some(new_id) = &result.new_state_id {
                let mut current_id = self.active_state_id.write().await;
                *current_id = Some(new_id.clone());
            }
        }

        Ok(result)
    }

    /// Types keys with state_id validation.
    pub async fn send_keys(
        &self,
        app_id: &str,
        keys: &str,
        state_id: &str,
    ) -> Result<ActionResult, CuaError> {
        let result = self.backend.send_keys(app_id, keys, state_id).await?;
        if result.success {
            if let Some(new_id) = &result.new_state_id {
                let mut current_id = self.active_state_id.write().await;
                *current_id = Some(new_id.clone());
            }
        }
        Ok(result)
    }

    /// Dispatches mouse actions by coordinate.
    pub async fn mouse_action(
        &self,
        action: MouseAction,
        x: i32,
        y: i32,
    ) -> Result<ActionResult, CuaError> {
        self.backend.mouse_action(&action, x, y).await
    }

    /// Retrieves the most recent observed state_id.
    pub async fn current_state_id(&self) -> Option<String> {
        self.active_state_id.read().await.clone()
    }
}
