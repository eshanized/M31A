//! Browser automation tools powered by `m31a-browser` (CDP client).
//!
//! Provides `browser_navigate`, `browser_snapshot`, `browser_click`, `browser_type`,
//! `browser_screenshot`, `browser_console`, `browser_network`, and `browser_evaluate`.

use anyhow::Result;
use m31a_browser::{BrowserConfig, BrowserSession};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

static BROWSER_SESSION: Mutex<Option<Arc<BrowserSession>>> = Mutex::const_new(None);

/// Ensures an active browser session exists or launches one.
pub async fn get_or_launch_browser() -> Result<Arc<BrowserSession>> {
    let mut lock = BROWSER_SESSION.lock().await;
    if let Some(ref session) = *lock {
        return Ok(Arc::clone(session));
    }

    let config = BrowserConfig::default();
    let session = Arc::new(BrowserSession::launch(config).await?);
    *lock = Some(Arc::clone(&session));
    Ok(session)
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

pub async fn execute_browser_navigate(args: &NavigateArgs) -> Result<String> {
    let session = get_or_launch_browser().await?;
    session.navigate(&args.url).await
}

pub async fn execute_browser_snapshot() -> Result<String> {
    let session = get_or_launch_browser().await?;
    let snapshot = session.snapshot().await?;
    Ok(snapshot.tree_text)
}

pub async fn execute_browser_click(args: &ClickArgs) -> Result<String> {
    let session = get_or_launch_browser().await?;
    session.click(&args.target_ref).await
}

pub async fn execute_browser_type(args: &TypeArgs) -> Result<String> {
    let session = get_or_launch_browser().await?;
    session.type_text(&args.target_ref, &args.text).await
}

pub async fn execute_browser_screenshot(args: &ScreenshotArgs) -> Result<String> {
    let session = get_or_launch_browser().await?;
    let path = args.path.as_deref().map(std::path::Path::new);
    session.screenshot(path).await
}

pub async fn execute_browser_console() -> Result<String> {
    let session = get_or_launch_browser().await?;
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
    let session = get_or_launch_browser().await?;
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

pub async fn execute_browser_evaluate(args: &EvaluateArgs) -> Result<String> {
    let session = get_or_launch_browser().await?;
    session.evaluate(&args.script).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_browser_tool_suite() {
        let nav_res = execute_browser_navigate(&NavigateArgs {
            url: "https://crates.io".to_string(),
        })
        .await
        .unwrap();
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
}
