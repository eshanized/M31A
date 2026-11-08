//! Browser automation tools powered by `m31a-browser` (CDP client).
//!
//! Provides `browser_navigate`, `browser_snapshot`, `browser_click`, `browser_type`,
//! `browser_screenshot`, `browser_console`, `browser_network`, and `browser_evaluate`.

use anyhow::Result;
use m31a_browser::{BrowserConfig, BrowserSession};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

/// Manages multiple independent browser sessions keyed by session / agent identifier.
#[derive(Clone)]
pub struct BrowserSessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<BrowserSession>>>>,
}

impl Default for BrowserSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Acquires an existing browser session or launches a new isolated browser context.
    pub async fn get_or_launch(&self, session_id: &str) -> Result<Arc<BrowserSession>> {
        {
            let map = self.sessions.read().await;
            if let Some(session) = map.get(session_id) {
                return Ok(Arc::clone(session));
            }
        }
        let mut map = self.sessions.write().await;
        if let Some(session) = map.get(session_id) {
            return Ok(Arc::clone(session));
        }

        let config = BrowserConfig::default();
        let session = Arc::new(BrowserSession::launch(config).await?);
        map.insert(session_id.to_string(), Arc::clone(&session));
        Ok(session)
    }

    /// Closes and cleans up the browser context associated with `session_id`.
    /// Closing one session never impacts or corrupts other active agent sessions.
    pub async fn close(&self, session_id: &str) -> Result<bool> {
        let mut map = self.sessions.write().await;
        if let Some(session) = map.remove(session_id) {
            let _ = session.close().await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Returns the number of currently active browser sessions.
    pub async fn active_sessions_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Shuts down all active browser sessions cleanly.
    pub async fn close_all(&self) {
        let mut map = self.sessions.write().await;
        for (_, session) in map.drain() {
            let _ = session.close().await;
        }
    }
}

static GLOBAL_BROWSER_MANAGER: OnceLock<BrowserSessionManager> = OnceLock::new();

pub fn global_browser_manager() -> &'static BrowserSessionManager {
    GLOBAL_BROWSER_MANAGER.get_or_init(BrowserSessionManager::new)
}

/// Ensures an active default browser session exists or launches one.
pub async fn get_or_launch_browser() -> Result<Arc<BrowserSession>> {
    global_browser_manager().get_or_launch("default").await
}

/// Ensures an active browser session exists for the given agent or session ID.
pub async fn get_or_launch_browser_for_session(session_id: &str) -> Result<Arc<BrowserSession>> {
    global_browser_manager().get_or_launch(session_id).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigateArgs {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickArgs {
    #[serde(rename = "ref")]
    pub target_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeArgs {
    #[serde(rename = "ref")]
    pub target_ref: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotArgs {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateArgs {
    pub script: String,
}

pub async fn execute_browser_navigate_for_session(
    session_id: &str,
    args: &NavigateArgs,
) -> Result<String> {
    let session = get_or_launch_browser_for_session(session_id).await?;
    session.navigate(&args.url).await
}

pub async fn execute_browser_navigate(args: &NavigateArgs) -> Result<String> {
    execute_browser_navigate_for_session("default", args).await
}

pub async fn execute_browser_snapshot() -> Result<String> {
    execute_browser_snapshot_for_session("default").await
}

pub async fn execute_browser_snapshot_for_session(session_id: &str) -> Result<String> {
    let session = get_or_launch_browser_for_session(session_id).await?;
    let snapshot = session.snapshot().await?;
    Ok(snapshot.tree_text)
}

pub async fn execute_browser_click_for_session(
    session_id: &str,
    args: &ClickArgs,
) -> Result<String> {
    let session = get_or_launch_browser_for_session(session_id).await?;
    session.click(&args.target_ref).await
}

pub async fn execute_browser_click(args: &ClickArgs) -> Result<String> {
    execute_browser_click_for_session("default", args).await
}

pub async fn execute_browser_type_for_session(session_id: &str, args: &TypeArgs) -> Result<String> {
    let session = get_or_launch_browser_for_session(session_id).await?;
    session.type_text(&args.target_ref, &args.text).await
}

pub async fn execute_browser_type(args: &TypeArgs) -> Result<String> {
    execute_browser_type_for_session("default", args).await
}

pub async fn execute_browser_screenshot_for_session(
    session_id: &str,
    args: &ScreenshotArgs,
) -> Result<String> {
    let session = get_or_launch_browser_for_session(session_id).await?;
    let path = args.path.as_deref().map(std::path::Path::new);
    session.screenshot(path).await
}

pub async fn execute_browser_screenshot(args: &ScreenshotArgs) -> Result<String> {
    execute_browser_screenshot_for_session("default", args).await
}

pub async fn execute_browser_console() -> Result<String> {
    execute_browser_console_for_session("default").await
}

pub async fn execute_browser_console_for_session(session_id: &str) -> Result<String> {
    let session = get_or_launch_browser_for_session(session_id).await?;
    let logs = session.get_console_logs().await;
    if logs.is_empty() {
        return Ok("No console messages logged for current page.".to_string());
    }
    let mut out = String::new();
    for l in logs {
        out.push_str(&format!("[{}] {}\n", l.level.to_uppercase(), l.text));
    }
    Ok(out)
}

pub async fn execute_browser_network() -> Result<String> {
    execute_browser_network_for_session("default").await
}

pub async fn execute_browser_network_for_session(session_id: &str) -> Result<String> {
    let session = get_or_launch_browser_for_session(session_id).await?;
    let records = session.get_network_records().await;
    if records.is_empty() {
        return Ok("No network requests recorded for current page.".to_string());
    }
    let mut out = String::new();
    for r in records {
        let status = r
            .status
            .map(|s| s.to_string())
            .unwrap_or_else(|| "pending".to_string());
        out.push_str(&format!(
            "{} {} -> [{}] ({})\n",
            r.method, r.url, status, r.resource_type
        ));
    }
    Ok(out)
}

pub async fn execute_browser_evaluate_for_session(
    session_id: &str,
    args: &EvaluateArgs,
) -> Result<String> {
    let session = get_or_launch_browser_for_session(session_id).await?;
    session.evaluate(&args.script).await
}

pub async fn execute_browser_evaluate(args: &EvaluateArgs) -> Result<String> {
    execute_browser_evaluate_for_session("default", args).await
}

pub async fn close_browser_session(session_id: &str) -> Result<bool> {
    global_browser_manager().close(session_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_browser_tool_suite() {
        let nav_res = match execute_browser_navigate(&NavigateArgs {
            url: "https://crates.io".to_string(),
        })
        .await
        {
            Ok(res) => res,
            Err(e)
                if e.to_string().contains("no Chromium binary found")
                    || e.to_string().contains("CDP page target not found")
                    || e.to_string().contains("Chromium failed to start properly")
                    || e.to_string().contains("Failed to connect to CDP") =>
            {
                eprintln!("Skipping browser test: Chromium/CDP unavailable in CI ({e})");
                return;
            }
            Err(e) => panic!("Unexpected browser navigation error: {e}"),
        };
        assert!(nav_res.contains("https://crates.io"));

        let snap_res = execute_browser_snapshot().await.unwrap();
        assert!(snap_res.contains("@e1"));

        let click_res = execute_browser_click(&ClickArgs {
            target_ref: "@e1".to_string(),
        })
        .await
        .unwrap();
        assert!(click_res.contains("@e1"));

        let type_res = execute_browser_type(&TypeArgs {
            target_ref: "@e1".to_string(),
            text: "tokio".to_string(),
        })
        .await
        .unwrap();
        assert!(type_res.contains("tokio"));

        let console_res = execute_browser_console().await.unwrap();
        assert!(console_res.contains("INFO"));

        let net_res = execute_browser_network().await.unwrap();
        assert!(net_res.contains("GET"));
    }

    #[tokio::test]
    async fn test_execute_browser_navigate_ssrf_rejection() {
        let targets = [
            "http://127.0.0.1:8080",
            "http://localhost:3000",
            "http://169.254.169.254/latest/meta-data",
            "http://metadata.google.internal",
            "http://10.0.0.1",
            "http://192.168.1.1",
            "http://172.16.0.1",
            "http://[::1]:8080",
            "http://[::ffff:127.0.0.1]:8080",
        ];

        for target in targets {
            let res = execute_browser_navigate(&NavigateArgs {
                url: target.to_string(),
            })
            .await;
            assert!(
                res.is_err(),
                "Target '{target}' should have been blocked by SSRF/DNS-rebinding protection"
            );
        }
    }

    #[tokio::test]
    async fn test_browser_session_manager_isolation() {
        let manager = BrowserSessionManager::new();
        assert_eq!(manager.active_sessions_count().await, 0);

        // Closing nonexistent session returns false
        let closed_nonexistent = manager.close("unknown").await.unwrap();
        assert!(!closed_nonexistent);
    }
}
