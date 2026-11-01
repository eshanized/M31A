//! Hook dispatcher runtime managing synchronous and asynchronous process execution.
//!
//! Runs matched handlers for an event in order and folds their outcomes into
//! one disposition. Handler execution contract:
//! - The event payload is piped to the handler command on stdin as JSON.
//! - Exit 0 with no parseable stdout → allow.
//! - Exit 0 with JSON stdout → `{"decision": "block", "reason": "…"}` blocks;
//!   `{"updated_input": …}` (PreToolUse) rewrites the tool input.
//! - Exit 2 → block with stderr as the reason.
//! - Any other exit → non-blocking; logged, never fatal.
//! - Handler timeout (default 10 s) kills the process and continues as
//!   non-blocking (logged).

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::config::{HandlerKind, HookConfig};
use crate::events::{HookDisposition, HookEvent};

/// Function signature for dispatching MCP tool calls from hook runtime.
pub type McpHandlerFn = Arc<
    dyn Fn(
            String,
            String,
            serde_json::Value,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send>>
        + Send
        + Sync,
>;

/// A dispatchable hook runtime: config + executor.
#[derive(Clone)]
pub struct HookRuntime {
    config: Arc<HookConfig>,
    mcp_invoker: Option<McpHandlerFn>,
}

/// Raw handler decision JSON (best-effort parse of stdout).
#[derive(Debug, Default, Deserialize)]
struct HandlerDecision {
    #[serde(default)]
    decision: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    updated_input: Option<serde_json::Value>,
}

/// One handler's observable outcome.
#[derive(Debug)]
struct HandlerOutcome {
    blocked: Option<String>,
    updated_input: Option<String>,
}

impl HookRuntime {
    pub fn new(config: HookConfig) -> Self {
        Self {
            config: Arc::new(config),
            mcp_invoker: None,
        }
    }

    pub fn with_mcp_invoker(mut self, invoker: McpHandlerFn) -> Self {
        self.mcp_invoker = Some(invoker);
        self
    }

    pub fn set_mcp_invoker(&mut self, invoker: McpHandlerFn) {
        self.mcp_invoker = Some(invoker);
    }

    pub fn config(&self) -> &HookConfig {
        &self.config
    }

    /// Dispatches an event to every matched handler, folding to a disposition.
    /// Sequential execution keeps outcomes deterministic.
    pub async fn dispatch(
        &self,
        event: HookEvent,
        subject: &str,
        payload: serde_json::Value,
    ) -> HookDisposition {
        let rules = self.config.matched_rules(event, subject);
        if rules.is_empty() {
            return HookDisposition::Allow;
        }

        let mut updated_input: Option<String> = None;

        for rule in rules {
            for handler in &rule.handlers {
                let outcome_res = match handler.kind {
                    HandlerKind::Mcp {
                        ref server,
                        ref tool,
                    } => self.run_mcp_handler(server, tool, event, &payload).await,
                    HandlerKind::Command => {
                        self.run_command_handler(handler, event, &payload).await
                    }
                };

                match outcome_res {
                    Ok(outcome) => {
                        if let Some(reason) = outcome.blocked {
                            debug!("Hook {} blocked: {reason}", event.config_key());
                            return HookDisposition::Block(reason);
                        }
                        if let Some(input) = outcome.updated_input
                            && updated_input.is_none()
                        {
                            updated_input = Some(input);
                        }
                    }
                    Err(e) => {
                        // Non-fatal by contract: a broken hook never blocks
                        // legitimate engine work; it is logged only.
                        warn!("Hook handler failed ({}): {e}", event.config_key());
                    }
                }
            }
        }

        if let Some(input) = updated_input {
            HookDisposition::Rewrite { input }
        } else {
            HookDisposition::Allow
        }
    }

    /// Runs an MCP hook handler: passes event payload to MCP tool and parses decision.
    async fn run_mcp_handler(
        &self,
        server: &str,
        tool: &str,
        event: HookEvent,
        payload: &serde_json::Value,
    ) -> Result<HandlerOutcome> {
        let resp_val = if let Some(ref invoker) = self.mcp_invoker {
            invoker(server.to_string(), tool.to_string(), payload.clone()).await?
        } else {
            // Attempt invoking server executable via stdio JSON-RPC if in PATH
            let mut cmd = Command::new(server);
            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = cmd
                .spawn()
                .context(format!("Failed to spawn MCP server '{}'", server))?;

            let rpc_req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": tool,
                    "arguments": payload
                }
            });

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(rpc_req.to_string().as_bytes()).await?;
            }

            let timeout_dur = Duration::from_secs(10);
            let output = tokio::time::timeout(timeout_dur, child.wait_with_output())
                .await
                .context("MCP tool call timed out")??;

            if !output.status.success() {
                let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                bail!("MCP server '{}' failed: {}", server, err_msg);
            }

            let stdout_str = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str(&stdout_str).unwrap_or(serde_json::Value::Null)
        };

        // Parse outcome from response JSON
        let decision: HandlerDecision = serde_json::from_value(resp_val.clone())
            .or_else(|_| {
                if let Some(res) = resp_val.get("result") {
                    serde_json::from_value(res.clone())
                } else {
                    Ok(HandlerDecision::default())
                }
            })
            .unwrap_or_default();

        let blocked = match decision.decision.as_deref() {
            Some("block") | Some("deny") => Some(
                decision
                    .reason
                    .unwrap_or_else(|| "Blocked by MCP hook".into()),
            ),
            _ => None,
        };

        let updated_input = if event == HookEvent::PreToolUse {
            decision.updated_input.map(|v| match v {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            })
        } else {
            None
        };

        Ok(HandlerOutcome {
            blocked,
            updated_input,
        })
    }

    /// Runs one command handler: JSON payload → stdin, decision ← stdout.
    async fn run_command_handler(
        &self,
        handler: &crate::config::HookHandler,
        _event: HookEvent,
        payload: &serde_json::Value,
    ) -> Result<HandlerOutcome> {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&handler.command);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn hook command: {}", handler.command))?;

        if let Some(mut stdin) = child.stdin.take() {
            let bytes = serde_json::to_vec(payload)?;
            let _ = stdin.write_all(&bytes).await;
            drop(stdin);
        }

        let timeout = Duration::from_secs(handler.timeout_seconds.max(1));
        let output = tokio::time::timeout(timeout, child.wait_with_output()).await;

        let output = match output {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return Err(e).context("Failed waiting on hook child process"),
            Err(_) => {
                // Timeout exceeded
                bail!(
                    "Hook command timed out after {}s: {}",
                    handler.timeout_seconds,
                    handler.command
                );
            }
        };

        let status = output.status;
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);

        // Exit 2: block unconditionally with stderr as the reason.
        if status.code() == Some(2) {
            let reason = stderr_str.trim().to_string();
            return Ok(HandlerOutcome {
                blocked: Some(if reason.is_empty() {
                    "Blocked by hook".to_string()
                } else {
                    reason
                }),
                updated_input: None,
            });
        }

        if !status.success() {
            return Err(anyhow::anyhow!(
                "Hook handler exited {} (non-blocking): {}",
                status.code().unwrap_or(-1),
                stderr_str.trim()
            ));
        }

        // Exit 0: parse optional decision JSON.
        let decision: Option<HandlerDecision> = if stdout_str.trim().is_empty() {
            None
        } else {
            serde_json::from_str(stdout_str.trim()).ok().or_else(|| {
                stdout_str
                    .lines()
                    .find(|l| l.trim_start().starts_with('{'))
                    .and_then(|l| serde_json::from_str(l).ok())
            })
        };

        let decision = decision.unwrap_or_default();
        let blocked = if decision.decision.as_deref() == Some("block") {
            Some(
                decision
                    .reason
                    .clone()
                    .unwrap_or_else(|| "Blocked by hook".to_string()),
            )
        } else {
            None
        };

        let updated_input = decision.updated_input.and_then(|v| {
            if v.is_null() {
                None
            } else {
                v.to_string().into()
            }
        });

        Ok(HandlerOutcome {
            blocked,
            updated_input,
        })
    }
}
