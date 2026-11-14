//! App Server local test client and protocol harness.
//! Lightweight in-process client harness for exercising App Server RPC methods and event streams in integration tests.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

/// Command execution approval decision from client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandApprovalDecision {
    Allow,
    Deny,
    AllowAlways,
}

/// File change approval decision from client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeApprovalDecision {
    Allow,
    Deny,
}

type HandlerFn = Box<dyn Fn(Value) -> Result<Value> + Send + Sync>;
type HandlerMap = Arc<Mutex<HashMap<String, HandlerFn>>>;

/// In-memory direct session implementing JSON-RPC dispatch for testing.
pub struct LocalAppServerSession {
    handlers: HandlerMap,
    notifications_tx: mpsc::UnboundedSender<JsonRpcNotification>,
}

impl Default for LocalAppServerSession {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalAppServerSession {
    pub fn new() -> Self {
        let (tx, _) = mpsc::unbounded_channel();
        let session = Self {
            handlers: Arc::new(Mutex::new(HashMap::new())),
            notifications_tx: tx,
        };
        session.register_default_handlers();
        session
    }

    pub fn with_notifications(
        notifications_tx: mpsc::UnboundedSender<JsonRpcNotification>,
    ) -> Self {
        let session = Self {
            handlers: Arc::new(Mutex::new(HashMap::new())),
            notifications_tx,
        };
        session.register_default_handlers();
        session
    }

    fn register_default_handlers(&self) {
        let mut map = self.handlers.lock().unwrap();

        map.insert(
            "initialize".to_string(),
            Box::new(|params| {
                let client_name = params
                    .get("client_info")
                    .and_then(|c| c.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("test-client");
                Ok(json!({
                    "server_info": {
                        "name": "m31a-app-server",
                        "version": "0.1.15"
                    },
                    "session_id": format!("sess_{client_name}"),
                    "capabilities": {
                        "threads": true,
                        "turn_stream": true,
                        "approvals": true
                    }
                }))
            }),
        );

        map.insert(
            "thread/start".to_string(),
            Box::new(|params| {
                let title = params
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("New Thread");
                let thread_id = format!("th_{}", uuid::Uuid::new_v4().simple());
                Ok(json!({
                    "thread_id": thread_id,
                    "title": title,
                    "status": "active"
                }))
            }),
        );

        map.insert(
            "account/rate_limits".to_string(),
            Box::new(|_| {
                Ok(json!({
                    "tokens_remaining": 500_000,
                    "requests_remaining": 1_000,
                    "reset_seconds": 60
                }))
            }),
        );
    }

    pub fn register_handler<F>(&self, method: &str, handler: F)
    where
        F: Fn(Value) -> Result<Value> + Send + Sync + 'static,
    {
        let mut map = self.handlers.lock().unwrap();
        map.insert(method.to_string(), Box::new(handler));
    }

    /// Dispatches a raw JSON-RPC request to the local session.
    pub fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let map = self.handlers.lock().unwrap();
        let params_val = request.params.clone().unwrap_or(Value::Null);
        match map.get(&request.method) {
            Some(handler) => match handler(params_val) {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(result),
                    error: None,
                    id: request.id.clone(),
                },
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(crate::protocol::JsonRpcError {
                        code: -32603,
                        message: e.to_string(),
                        data: None,
                    }),
                    id: request.id.clone(),
                },
            },
            None => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(crate::protocol::JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
                id: request.id.clone(),
            },
        }
    }

    /// Emits a notification from the server to clients.
    pub fn emit_notification(&self, method: &str, params: Value) {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: Some(params),
        };
        let _ = self.notifications_tx.send(notif);
    }
}

/// Fluent client harness for testing App Server protocol interactions.
pub struct AppServerTestHarness {
    session: Arc<LocalAppServerSession>,
    next_id: u64,
}

impl Default for AppServerTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl AppServerTestHarness {
    pub fn new() -> Self {
        Self {
            session: Arc::new(LocalAppServerSession::new()),
            next_id: 1,
        }
    }

    pub fn with_session(session: Arc<LocalAppServerSession>) -> Self {
        Self {
            session,
            next_id: 1,
        }
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: Some(params),
            id: Some(json!(self.next_id)),
        };
        self.next_id += 1;

        let resp = self.session.handle_request(&req);
        if let Some(err) = resp.error {
            anyhow::bail!("JSON-RPC error {}: {}", err.code, err.message);
        }
        resp.result.context("Missing result in JSON-RPC response")
    }

    pub fn initialize(&mut self, client_name: &str, version: &str) -> Result<Value> {
        self.call(
            "initialize",
            json!({
                "client_info": {
                    "name": client_name,
                    "version": version
                }
            }),
        )
    }

    pub fn start_thread(&mut self, title: &str, cwd: &str, model: &str) -> Result<String> {
        let res = self.call(
            "thread/start",
            json!({
                "title": title,
                "cwd": cwd,
                "model": model
            }),
        )?;
        res.get("thread_id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .context("Missing thread_id")
    }

    pub fn get_rate_limits(&mut self) -> Result<Value> {
        self.call("account/rate_limits", json!({}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_app_server_initialize_and_thread_lifecycle() {
        let mut harness = AppServerTestHarness::new();

        let init = harness.initialize("vscode-codex", "1.2.0").unwrap();
        assert_eq!(
            init.get("server_info")
                .unwrap()
                .get("name")
                .unwrap()
                .as_str()
                .unwrap(),
            "m31a-app-server"
        );
        assert_eq!(
            init.get("session_id").unwrap().as_str().unwrap(),
            "sess_vscode-codex"
        );

        let thread_id = harness
            .start_thread(
                "Refactor API",
                "/tmp",
                "nvidia/nemotron-3.5-lightning-30b-a3b",
            )
            .unwrap();
        assert!(thread_id.starts_with("th_"));

        let limits = harness.get_rate_limits().unwrap();
        assert_eq!(
            limits.get("tokens_remaining").unwrap().as_i64().unwrap(),
            500_000
        );
    }

    #[test]
    fn test_custom_session_handler() {
        let session = Arc::new(LocalAppServerSession::new());
        session.register_handler("custom/echo", |params| {
            let val = params.get("msg").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json!({ "echo": val }))
        });

        let mut harness = AppServerTestHarness::with_session(session);
        let res = harness
            .call("custom/echo", json!({ "msg": "hello world" }))
            .unwrap();
        assert_eq!(res.get("echo").unwrap().as_str().unwrap(), "hello world");
    }
}
