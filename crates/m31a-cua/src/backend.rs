//! CUA Backend interface and simulated reference implementation.

use crate::error::CuaError;
use crate::model::{
    AccessibleElement, ActionResult, AppInfo, AppState, ElementAction, ElementRole, MouseAction,
    Rect, WindowInfo,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Core interface for native and simulated accessibility backends.
#[async_trait]
pub trait CuaBackend: Send + Sync {
    /// Enumerates currently running applications and active windows.
    async fn list_apps(&self) -> Result<Vec<AppInfo>, CuaError>;

    /// Captures the complete hierarchical accessibility tree of the target app.
    /// If `app_id` is None, targets the currently focused window/application.
    /// If `capture_background` is true, interrogates background windows without stealing focus.
    async fn get_app_state(
        &self,
        app_id: Option<&str>,
        capture_background: bool,
    ) -> Result<AppState, CuaError>;

    /// Dispatches a semantic action to a specific element by its index in the active tree.
    /// Enforces `state_id` check: if the UI state mutated since observation, returns `StateMismatch`.
    async fn perform_element_action(
        &self,
        app_id: &str,
        element_index: usize,
        action: &ElementAction,
        state_id: &str,
    ) -> Result<ActionResult, CuaError>;

    /// Dispatches keyboard characters or shortcuts to the active element / window.
    async fn send_keys(
        &self,
        app_id: &str,
        keys: &str,
        state_id: &str,
    ) -> Result<ActionResult, CuaError>;

    /// Fallback coordinate-based mouse interaction for canvas or unexposed controls.
    async fn mouse_action(
        &self,
        action: &MouseAction,
        x: i32,
        y: i32,
    ) -> Result<ActionResult, CuaError>;
}

/// In-memory synthetic desktop backend for testing and deterministic CI runs.
pub struct SimulatedBackend {
    apps: Arc<RwLock<Vec<AppInfo>>>,
    states: Arc<RwLock<HashMap<String, AppState>>>,
    active_app_id: Arc<RwLock<String>>,
}

impl Default for SimulatedBackend {
    fn default() -> Self {
        Self::new_with_standard_apps()
    }
}

impl SimulatedBackend {
    /// Creates a simulated desktop environment pre-populated with standard applications.
    pub fn new_with_standard_apps() -> Self {
        let app1_id = "app_editor".to_string();
        let app2_id = "app_calc".to_string();

        let editor_app = AppInfo {
            id: app1_id.clone(),
            name: "M31A Text Editor".to_string(),
            pid: 1042,
            is_active: true,
            windows: vec![WindowInfo {
                id: "win_1".to_string(),
                title: "Document1.rs - M31A Editor".to_string(),
                bounds: Rect::new(50, 50, 1024, 768),
                is_focused: true,
            }],
        };

        let calc_app = AppInfo {
            id: app2_id.clone(),
            name: "Calculator".to_string(),
            pid: 1088,
            is_active: false,
            windows: vec![WindowInfo {
                id: "win_2".to_string(),
                title: "Calculator".to_string(),
                bounds: Rect::new(1100, 100, 320, 480),
                is_focused: false,
            }],
        };

        // Construct editor accessibility tree
        let editor_tree = AccessibleElement {
            index: 0,
            role: ElementRole::Window,
            name: "Document1.rs - M31A Editor".to_string(),
            value: None,
            bounds: Some(Rect::new(50, 50, 1024, 768)),
            states: vec!["active".to_string(), "resizable".to_string()],
            actions: vec![
                "close".to_string(),
                "minimize".to_string(),
                "maximize".to_string(),
            ],
            children: vec![
                AccessibleElement {
                    index: 1,
                    role: ElementRole::Menu,
                    name: "MenuBar".to_string(),
                    value: None,
                    bounds: Some(Rect::new(50, 50, 1024, 30)),
                    states: vec!["visible".to_string()],
                    actions: vec![],
                    children: vec![
                        AccessibleElement {
                            index: 2,
                            role: ElementRole::MenuItem,
                            name: "File".to_string(),
                            value: None,
                            bounds: Some(Rect::new(55, 52, 40, 24)),
                            states: vec!["enabled".to_string()],
                            actions: vec!["press".to_string()],
                            children: vec![],
                        },
                        AccessibleElement {
                            index: 3,
                            role: ElementRole::MenuItem,
                            name: "Edit".to_string(),
                            value: None,
                            bounds: Some(Rect::new(100, 52, 40, 24)),
                            states: vec!["enabled".to_string()],
                            actions: vec!["press".to_string()],
                            children: vec![],
                        },
                    ],
                },
                AccessibleElement {
                    index: 4,
                    role: ElementRole::TextArea,
                    name: "Code Editor Canvas".to_string(),
                    value: Some("fn main() {\n    println!(\"Hello World\");\n}".to_string()),
                    bounds: Some(Rect::new(60, 90, 1000, 650)),
                    states: vec!["focused".to_string(), "editable".to_string()],
                    actions: vec!["activate".to_string(), "set_value".to_string()],
                    children: vec![],
                },
                AccessibleElement {
                    index: 5,
                    role: ElementRole::Button,
                    name: "Save Document".to_string(),
                    value: None,
                    bounds: Some(Rect::new(950, 745, 110, 32)),
                    states: vec!["enabled".to_string()],
                    actions: vec!["click".to_string()],
                    children: vec![],
                },
            ],
        };

        let initial_state_id = "state_editor_v1".to_string();
        let editor_state = AppState {
            state_id: initial_state_id,
            timestamp: chrono::Utc::now(),
            app_id: app1_id.clone(),
            app_name: "M31A Text Editor".to_string(),
            window_title: "Document1.rs - M31A Editor".to_string(),
            element_count: editor_tree.total_nodes(),
            root: editor_tree,
        };

        let mut states = HashMap::new();
        states.insert(app1_id.clone(), editor_state);

        Self {
            apps: Arc::new(RwLock::new(vec![editor_app, calc_app])),
            states: Arc::new(RwLock::new(states)),
            active_app_id: Arc::new(RwLock::new(app1_id)),
        }
    }
}

#[async_trait]
impl CuaBackend for SimulatedBackend {
    async fn list_apps(&self) -> Result<Vec<AppInfo>, CuaError> {
        let apps = self.apps.read().await;
        Ok(apps.clone())
    }

    async fn get_app_state(
        &self,
        app_id: Option<&str>,
        _capture_background: bool,
    ) -> Result<AppState, CuaError> {
        let target_id = match app_id {
            Some(id) => id.to_string(),
            None => self.active_app_id.read().await.clone(),
        };

        let states = self.states.read().await;
        if let Some(state) = states.get(&target_id) {
            Ok(state.clone())
        } else {
            Err(CuaError::AppNotFound(target_id))
        }
    }

    async fn perform_element_action(
        &self,
        app_id: &str,
        element_index: usize,
        action: &ElementAction,
        state_id: &str,
    ) -> Result<ActionResult, CuaError> {
        let mut states = self.states.write().await;
        let state = states
            .get_mut(app_id)
            .ok_or_else(|| CuaError::AppNotFound(app_id.to_string()))?;

        // Enforce state_id consistency
        if state.state_id != state_id {
            return Err(CuaError::StateMismatch {
                expected: state_id.to_string(),
                actual: state.state_id.clone(),
            });
        }

        // Locate target element
        let target = state
            .root
            .find_by_index_mut(element_index)
            .ok_or(CuaError::ElementNotFound(element_index))?;

        let message = match action {
            ElementAction::Click => {
                format!("Clicked '{}' ({:?})", target.name, target.role)
            }
            ElementAction::DoubleClick => {
                format!("Double-clicked '{}' ({:?})", target.name, target.role)
            }
            ElementAction::RightClick => {
                format!("Right-clicked '{}' ({:?})", target.name, target.role)
            }
            ElementAction::SetValue(val) => {
                target.value = Some(val.clone());
                format!("Set value of '{}' to: {}", target.name, val)
            }
            ElementAction::Select => {
                if !target.states.contains(&"selected".to_string()) {
                    target.states.push("selected".to_string());
                }
                format!("Selected element '{}'", target.name)
            }
            ElementAction::Toggle => {
                let is_checked = target.states.contains(&"checked".to_string());
                if is_checked {
                    target.states.retain(|s| s != "checked");
                    format!("Unchecked '{}'", target.name)
                } else {
                    target.states.push("checked".to_string());
                    format!("Checked '{}'", target.name)
                }
            }
            ElementAction::PerformAction(act) => {
                format!("Executed action '{}' on '{}'", act, target.name)
            }
        };

        // State has changed, advance state_id
        let next_state_id = format!("state_{}_{}", app_id, uuid::Uuid::new_v4().simple());
        state.state_id = next_state_id.clone();
        state.timestamp = chrono::Utc::now();

        Ok(ActionResult {
            success: true,
            message,
            new_state_id: Some(next_state_id),
            action_sent: true,
        })
    }

    async fn send_keys(
        &self,
        app_id: &str,
        keys: &str,
        state_id: &str,
    ) -> Result<ActionResult, CuaError> {
        let mut states = self.states.write().await;
        let state = states
            .get_mut(app_id)
            .ok_or_else(|| CuaError::AppNotFound(app_id.to_string()))?;

        if state.state_id != state_id {
            return Err(CuaError::StateMismatch {
                expected: state_id.to_string(),
                actual: state.state_id.clone(),
            });
        }

        // In simulation, append keys to any currently focused text element
        let mut target_name = "active window".to_string();
        if let Some(target) = state.root.find_by_index_mut(4)
            && let Some(existing) = &mut target.value
        {
            existing.push_str(keys);
            target_name = target.name.clone();
        }

        let next_state_id = format!("state_{}_{}", app_id, uuid::Uuid::new_v4().simple());
        state.state_id = next_state_id.clone();
        state.timestamp = chrono::Utc::now();

        Ok(ActionResult {
            success: true,
            message: format!("Typed keys into '{}': {}", target_name, keys),
            new_state_id: Some(next_state_id),
            action_sent: true,
        })
    }

    async fn mouse_action(
        &self,
        action: &MouseAction,
        x: i32,
        y: i32,
    ) -> Result<ActionResult, CuaError> {
        Ok(ActionResult {
            success: true,
            message: format!("Dispatched mouse action {:?} at ({}, {})", action, x, y),
            new_state_id: None,
            action_sent: true,
        })
    }
}
