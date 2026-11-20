//! Accessibility-First Computer Use Agent (CUA) tool suite powered by `m31a-cua`.
//!
//! Exposes `cua_list_apps`, `cua_get_state`, `cua_action`, `cua_key`, and `cua_mouse`.

use anyhow::{anyhow, Result};
use m31a_cua::{
    CuaSessionManager, ElementAction, MouseAction, SimulatedBackend,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

static GLOBAL_CUA_MANAGER: OnceLock<CuaSessionManager> = OnceLock::new();

/// Returns the global CUA session manager initialized with the active platform backend.
/// Uses native Linux backend on Linux by default; falls back to simulated backend
/// if explicitly requested via `M31A_SIMULATED_CUA=1`.
pub fn global_cua_manager() -> &'static CuaSessionManager {
    GLOBAL_CUA_MANAGER.get_or_init(|| {
        #[cfg(target_os = "linux")]
        {
            if std::env::var("M31A_SIMULATED_CUA").as_deref() != Ok("1") {
                return CuaSessionManager::new(Arc::new(m31a_cua::LinuxNativeBackend::new()));
            }
        }
        CuaSessionManager::new(Arc::new(SimulatedBackend::default()))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuaGetStateArgs {
    pub app_id: Option<String>,
    #[serde(default)]
    pub capture_background: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuaActionArgs {
    pub app_id: String,
    pub target_index: usize,
    pub action_type: String,
    pub value: Option<String>,
    pub state_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuaKeyArgs {
    pub app_id: String,
    pub keys: String,
    pub state_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuaMouseArgs {
    pub action: String,
    pub x: i32,
    pub y: i32,
    pub button: Option<String>,
}

/// Enumerate running desktop applications and window titles.
pub async fn execute_cua_list_apps() -> Result<String> {
    let manager = global_cua_manager();
    let apps = manager.list_apps().await.map_err(|e| anyhow!("{e}"))?;
    serde_json::to_string_pretty(&apps).map_err(|e| anyhow!("{e}"))
}

/// Fetch the hierarchical accessible element tree for the focused or selected application.
pub async fn execute_cua_get_state(args: &CuaGetStateArgs) -> Result<String> {
    let manager = global_cua_manager();
    let state = manager
        .get_app_state(args.app_id.as_deref(), args.capture_background)
        .await
        .map_err(|e| anyhow!("{e}"))?;
    serde_json::to_string_pretty(&state).map_err(|e| anyhow!("{e}"))
}

/// Dispatches a semantic action (click, double_click, set_value, select, toggle) to an element.
pub async fn execute_cua_action(args: &CuaActionArgs) -> Result<String> {
    let manager = global_cua_manager();
    let action = match args.action_type.to_lowercase().as_str() {
        "click" => ElementAction::Click,
        "double_click" | "doubleclick" => ElementAction::DoubleClick,
        "right_click" | "rightclick" => ElementAction::RightClick,
        "set_value" | "setvalue" | "type" => {
            let val = args.value.clone().unwrap_or_default();
            ElementAction::SetValue(val)
        }
        "select" => ElementAction::Select,
        "toggle" => ElementAction::Toggle,
        other => ElementAction::PerformAction(other.to_string()),
    };

    let result = manager
        .perform_action(
            &args.app_id,
            args.target_index,
            action,
            &args.state_id,
        )
        .await
        .map_err(|e| anyhow!("{e}"))?;

    serde_json::to_string_pretty(&result).map_err(|e| anyhow!("{e}"))
}

/// Dispatches keystrokes or shortcuts to the active application.
pub async fn execute_cua_key(args: &CuaKeyArgs) -> Result<String> {
    let manager = global_cua_manager();
    let result = manager
        .send_keys(&args.app_id, &args.keys, &args.state_id)
        .await
        .map_err(|e| anyhow!("{e}"))?;
    serde_json::to_string_pretty(&result).map_err(|e| anyhow!("{e}"))
}

/// Dispatches coordinate-based mouse interactions.
pub async fn execute_cua_mouse(args: &CuaMouseArgs) -> Result<String> {
    let manager = global_cua_manager();
    let button = args.button.clone().unwrap_or_else(|| "left".to_string());
    let action = match args.action.to_lowercase().as_str() {
        "click" => MouseAction::Click { button },
        "double_click" | "doubleclick" => MouseAction::DoubleClick { button },
        "move" => MouseAction::Move,
        "drag" => MouseAction::Drag {
            to_x: args.x,
            to_y: args.y,
        },
        "scroll" => MouseAction::Scroll {
            dx: 0,
            dy: args.y,
        },
        other => MouseAction::Click {
            button: other.to_string(),
        },
    };

    let result = manager
        .mouse_action(action, args.x, args.y)
        .await
        .map_err(|e| anyhow!("{e}"))?;
    serde_json::to_string_pretty(&result).map_err(|e| anyhow!("{e}"))
}
