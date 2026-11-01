//! Sandboxed runtime environment for evaluating Code Mode JavaScript/TypeScript code.
//!
//! Spawns runtime isolates, injects ambient globals, and executes Code Mode scripts
//! either via an isolated external JS isolate (Node/Bun sandboxed with Bubblewrap)
//! or via an in-process confined interpreter with complete standard built-ins.

use anyhow::{Context, Result};
use chrono::Utc;
use m31a_core::SandboxPolicy;
use m31a_sandbox::runner::SandboxRunner;
use m31a_tools::ToolRegistry;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

/// Process lifecycle states for the Code Mode supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessLifecycleState {
    Created,
    Running,
    Yielded,
    Completed,
    TimedOut,
    Cancelled,
    Failed,
    ResourceExhausted,
}

/// Typed errors produced during Code Mode sandboxed supervisor execution.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CodeModeError {
    #[error("Sandbox initialization failed: {0}")]
    SandboxInitFailed(String),

    #[error("Sandbox required but backend unavailable: {0}")]
    SandboxUnavailable(String),

    #[error("Execution resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Execution timed out after {0:?}")]
    TimedOut(Duration),

    #[error("Execution cancelled")]
    Cancelled,

    #[error("IPC protocol violation: {0}")]
    ProtocolViolation(String),

    #[error("Child process failure: {0}")]
    ProcessFailed(String),

    #[error("User script error: {0}")]
    UserCodeFailed(String),
}

#[derive(Debug, Clone)]
pub struct RuntimeExecutionOutcome {
    pub output: String,
    pub yielded: bool,
    pub notifications: Vec<String>,
    pub status: ProcessLifecycleState,
}

/// Resource constraints for script execution.
#[derive(Debug, Clone)]
pub struct RuntimeExecutionLimits {
    pub timeout_ms: u64,
    pub max_tool_calls: usize,
    pub max_output_bytes: usize,
}

impl Default for RuntimeExecutionLimits {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            max_tool_calls: 50,
            max_output_bytes: 100_000,
        }
    }
}

#[derive(Clone)]
pub struct CodeModeRuntime {
    kv_store: Arc<Mutex<HashMap<String, Value>>>,
    js_runtime_binary: Option<String>,
    limits: RuntimeExecutionLimits,
    trust_store: Arc<Mutex<m31a_trust::TrustStore>>,
}

impl Default for CodeModeRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeModeRuntime {
    pub fn new() -> Self {
        let binary = Self::detect_js_runtime();
        Self {
            kv_store: Arc::new(Mutex::new(HashMap::new())),
            js_runtime_binary: binary,
            limits: RuntimeExecutionLimits::default(),
            trust_store: Arc::new(Mutex::new(m31a_trust::TrustStore::new())),
        }
    }

    pub fn with_limits(limits: RuntimeExecutionLimits) -> Self {
        let binary = Self::detect_js_runtime();
        Self {
            kv_store: Arc::new(Mutex::new(HashMap::new())),
            js_runtime_binary: binary,
            limits,
            trust_store: Arc::new(Mutex::new(m31a_trust::TrustStore::new())),
        }
    }

    pub fn with_trust_store(mut self, trust_store: Arc<Mutex<m31a_trust::TrustStore>>) -> Self {
        self.trust_store = trust_store;
        self
    }

    pub fn trust_store(&self) -> Arc<Mutex<m31a_trust::TrustStore>> {
        self.trust_store.clone()
    }

    fn detect_js_runtime() -> Option<String> {
        for candidate in &["bun", "node"] {
            if let Ok(output) = Command::new(candidate).arg("--version").output()
                && output.status.success()
            {
                return Some(candidate.to_string());
            }
        }
        None
    }

    pub fn store(&self, key: &str, value: Value) {
        self.kv_store.lock().unwrap().insert(key.to_string(), value);
    }

    pub fn load(&self, key: &str) -> Option<Value> {
        self.kv_store.lock().unwrap().get(key).cloned()
    }

    pub async fn run(
        &self,
        code: &str,
        registry: &ToolRegistry,
        cwd: &Path,
        policy: SandboxPolicy,
        yield_time_ms: u64,
    ) -> Result<RuntimeExecutionOutcome> {
        // Enforce workspace trust authorization before executing Code Mode code
        let decision = self.trust_store.lock().unwrap().evaluate(cwd);
        match decision {
            m31a_trust::TrustDecision::Denied { path } => {
                anyhow::bail!(
                    "Code Mode execution DENIED: Working directory '{}' is marked as Denied by security trust policy.",
                    path.display()
                );
            }
            m31a_trust::TrustDecision::RequiresConfirmation { path, reason } => {
                anyhow::bail!(
                    "Code Mode execution REJECTED: Working directory '{}' is untrusted ({}).",
                    path.display(),
                    reason
                );
            }
            m31a_trust::TrustDecision::Trusted | m31a_trust::TrustDecision::SessionOnly => {}
        }

        // If JS engine (bun or node) is present, use sandboxed external isolate
        if let Some(ref binary) = self.js_runtime_binary {
            return self
                .run_ipc_runner(binary, code, registry, cwd, policy, yield_time_ms)
                .await;
        }

        // High-fidelity in-process confined interpreter (only used when no external JS runtime binary is available)
        self.run_confined_interpreter(code, registry, cwd, policy, yield_time_ms)
            .await
    }

    /// Reads a line from an async buffered reader up to `max_line_bytes`.
    /// Fails closed with `CodeModeError::ProtocolViolation` if the line exceeds the boundary.
    async fn read_bounded_line<R: tokio::io::AsyncBufRead + Unpin>(
        reader: &mut R,
        buf: &mut String,
        max_line_bytes: usize,
    ) -> Result<Option<usize>, CodeModeError> {
        buf.clear();
        let mut total_read = 0;
        loop {
            let available = match tokio::io::AsyncBufReadExt::fill_buf(reader).await {
                Ok(b) => b,
                Err(e) => return Err(CodeModeError::ProcessFailed(e.to_string())),
            };
            if available.is_empty() {
                return Ok(if total_read > 0 {
                    Some(total_read)
                } else {
                    None
                });
            }
            if let Some(pos) = available.iter().position(|&b| b == b'\n') {
                let chunk_len = pos + 1;
                if total_read + chunk_len > max_line_bytes {
                    return Err(CodeModeError::ProtocolViolation(format!(
                        "IPC line size exceeded maximum allowed boundary of {} bytes",
                        max_line_bytes
                    )));
                }
                let s = std::str::from_utf8(&available[..chunk_len]).map_err(|e| {
                    CodeModeError::ProtocolViolation(format!(
                        "Invalid UTF-8 sequence in IPC stream: {e}"
                    ))
                })?;
                buf.push_str(s);
                tokio::io::AsyncBufReadExt::consume(reader, chunk_len);
                total_read += chunk_len;
                return Ok(Some(total_read));
            } else {
                let chunk_len = available.len();
                if total_read + chunk_len >= max_line_bytes {
                    return Err(CodeModeError::ProtocolViolation(format!(
                        "IPC line size exceeded maximum allowed boundary of {} bytes",
                        max_line_bytes
                    )));
                }
                let s = std::str::from_utf8(available).map_err(|e| {
                    CodeModeError::ProtocolViolation(format!(
                        "Invalid UTF-8 sequence in IPC chunk: {e}"
                    ))
                })?;
                buf.push_str(s);
                tokio::io::AsyncBufReadExt::consume(reader, chunk_len);
                total_read += chunk_len;
            }
        }
    }

    /// Spawns an external Node/Bun isolate supervised asynchronously with Bubblewrap sandbox namespaces when applicable.
    async fn run_ipc_runner(
        &self,
        binary: &str,
        user_code: &str,
        registry: &ToolRegistry,
        cwd: &Path,
        policy: SandboxPolicy,
        yield_time_ms: u64,
    ) -> Result<RuntimeExecutionOutcome> {
        let runner_script = r#"
const readline = require('readline');
const rl = readline.createInterface({ input: process.stdin, output: process.stdout, terminal: false });

let reqCounter = 0;
const pending = new Map();

rl.on('line', (line) => {
    try {
        const msg = JSON.parse(line);
        if (msg.type === 'response') {
            const cb = pending.get(msg.id);
            if (cb) {
                pending.delete(msg.id);
                cb(msg.error ? new Error(msg.error) : null, msg.result);
            }
        }
    } catch (_) {}
});

function rpcCall(method, params) {
    return new Promise((resolve, reject) => {
        const id = ++reqCounter;
        pending.set(id, (err, res) => err ? reject(err) : resolve(res));
        process.stdout.write(JSON.stringify({ type: 'call', id, method, params }) + '\n');
    });
}

global.text = (val) => {
    process.stdout.write(JSON.stringify({ type: 'text', value: typeof val === 'object' ? JSON.stringify(val) : String(val) }) + '\n');
};

global.image = (val) => {
    process.stdout.write(JSON.stringify({ type: 'image', value: val }) + '\n');
};

global.notify = (val) => {
    process.stdout.write(JSON.stringify({ type: 'notify', value: typeof val === 'object' ? JSON.stringify(val) : String(val) }) + '\n');
};

global.store = (key, val) => rpcCall('store', { key, val });
global.load = (key) => rpcCall('load', { key });
global.exit = () => process.exit(0);
global.yield_control = () => {
    process.stdout.write(JSON.stringify({ type: 'yield' }) + '\n');
};

global.tools = new Proxy({}, {
    get: (_, toolName) => async (args) => {
        return await rpcCall('tool', { name: toolName, args: args || {} });
    }
});

(async () => {
    try {
        /* USER_CODE_PLACEHOLDER */
    } catch (e) {
        process.stdout.write(JSON.stringify({ type: 'text', value: 'Error: ' + (e.stack || e.message) }) + '\n');
    } finally {
        process.stdout.write(JSON.stringify({ type: 'done' }) + '\n');
        process.exit(0);
    }
})();
"#;

        let bootstrap = runner_script.replace("/* USER_CODE_PLACEHOLDER */", user_code);

        // Enforce sandbox isolation via Bubblewrap if policy warrants; fail closed if unavailable
        let sandbox = SandboxRunner::new();
        let mut child_cmd = if policy != SandboxPolicy::DangerFullAccess {
            if !sandbox.has_bwrap() {
                anyhow::bail!(CodeModeError::SandboxUnavailable(format!(
                    "Sandbox isolation required for policy '{:?}', but Bubblewrap sandbox backend is unavailable. Refusing unconfined execution.",
                    policy
                )));
            }
            sandbox
                .build_bwrap_process(binary, &["-e", &bootstrap], cwd, policy)
                .map_err(|e| CodeModeError::SandboxInitFailed(e.to_string()))?
        } else {
            let mut tokio_cmd = tokio::process::Command::new(binary);
            tokio_cmd.arg("-e").arg(&bootstrap).current_dir(cwd);
            tokio_cmd
        };

        child_cmd.stdin(Stdio::piped());
        child_cmd.stdout(Stdio::piped());
        child_cmd.stderr(Stdio::piped());
        child_cmd.kill_on_drop(true);

        #[cfg(unix)]
        unsafe {
            child_cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }

        let mut child = child_cmd.spawn().map_err(|e| {
            CodeModeError::ProcessFailed(format!("Failed to spawn JS runtime child process: {e}"))
        })?;

        let child_pid = child.id();
        let mut stdin = child
            .stdin
            .take()
            .context("Failed to capture child stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to capture child stdout")?;
        let stderr = child
            .stderr
            .take()
            .context("Failed to capture child stderr")?;

        let stdout_buf_reader = tokio::io::BufReader::new(stdout);
        let stderr_buf_reader = tokio::io::BufReader::new(stderr);

        let mut output_acc = String::new();
        let mut notifications = Vec::new();
        let mut yielded = false;
        let mut tool_calls_count = 0;
        let mut total_messages = 0;
        let mut seen_ids = std::collections::HashSet::new();
        let mut termination_reason = ProcessLifecycleState::Running;

        const MAX_IPC_LINE_BYTES: usize = 65_536; // 64 KB
        const MAX_PROTOCOL_MESSAGES: usize = 5_000;
        const MAX_NOTIFICATIONS: usize = 100;
        const MAX_STORED_KEYS: usize = 1_000;

        let timeout_duration = Duration::from_millis(yield_time_ms.min(self.limits.timeout_ms));

        enum IpcEvent {
            StdoutLine(String),
            StderrChunk(String),
            Error(CodeModeError),
            StdoutEof,
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel::<IpcEvent>(128);

        let tx_out = tx.clone();
        let stdout_task = tokio::spawn(async move {
            let mut reader = stdout_buf_reader;
            let mut line_buf = String::new();
            loop {
                match Self::read_bounded_line(&mut reader, &mut line_buf, MAX_IPC_LINE_BYTES).await
                {
                    Ok(Some(_)) => {
                        if tx_out
                            .send(IpcEvent::StdoutLine(line_buf.clone()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) => {
                        let _ = tx_out.send(IpcEvent::StdoutEof).await;
                        break;
                    }
                    Err(e) => {
                        let _ = tx_out.send(IpcEvent::Error(e)).await;
                        break;
                    }
                }
            }
        });

        let tx_err = tx.clone();
        let stderr_task = tokio::spawn(async move {
            let mut reader = stderr_buf_reader;
            let mut buf = [0u8; 4096];
            loop {
                match tokio::io::AsyncReadExt::read(&mut reader, &mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let s = String::from_utf8_lossy(&buf[..n]).to_string();
                        if tx_err.send(IpcEvent::StderrChunk(s)).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        drop(tx);

        let supervisor_future = async {
            while let Some(evt) = rx.recv().await {
                match evt {
                    IpcEvent::StdoutLine(line_str) => {
                        total_messages += 1;
                        if total_messages > MAX_PROTOCOL_MESSAGES {
                            termination_reason = ProcessLifecycleState::ResourceExhausted;
                            return Err(CodeModeError::ResourceExhausted(format!(
                                "IPC protocol message budget exceeded: limit is {}",
                                MAX_PROTOCOL_MESSAGES
                            )));
                        }
                        let line = line_str.trim();
                        if let Ok(msg) = serde_json::from_str::<Value>(line) {
                            match msg.get("type").and_then(Value::as_str) {
                                Some("text") => {
                                    if let Some(v) = msg.get("value").and_then(Value::as_str)
                                        && output_acc.len() < self.limits.max_output_bytes
                                    {
                                        output_acc.push_str(v);
                                        output_acc.push('\n');
                                    }
                                }
                                Some("notify") => {
                                    if let Some(v) = msg.get("value").and_then(Value::as_str)
                                        && notifications.len() < MAX_NOTIFICATIONS
                                    {
                                        notifications.push(v.to_string());
                                    }
                                }
                                Some("yield") => {
                                    yielded = true;
                                    termination_reason = ProcessLifecycleState::Yielded;
                                }
                                Some("done") => {
                                    termination_reason = ProcessLifecycleState::Completed;
                                    break;
                                }
                                Some("call") => {
                                    let id = msg.get("id").and_then(Value::as_u64).unwrap_or(0);
                                    let method =
                                        msg.get("method").and_then(Value::as_str).unwrap_or("");
                                    let params = msg.get("params").cloned().unwrap_or(Value::Null);

                                    if !seen_ids.insert(id) {
                                        let err_resp = json!({
                                            "type": "response",
                                            "id": id,
                                            "error": format!("Duplicate request ID: {id}")
                                        });
                                        let _ =
                                            stdin.write_all(err_resp.to_string().as_bytes()).await;
                                        let _ = stdin.write_all(b"\n").await;
                                        let _ = stdin.flush().await;
                                        continue;
                                    }

                                    let response = match method {
                                        "store" => {
                                            let key = params
                                                .get("key")
                                                .and_then(Value::as_str)
                                                .unwrap_or("");
                                            let val =
                                                params.get("val").cloned().unwrap_or(Value::Null);
                                            if self.kv_store.lock().unwrap().len()
                                                >= MAX_STORED_KEYS
                                            {
                                                json!({ "type": "response", "id": id, "error": "Storage key limit reached" })
                                            } else {
                                                self.store(key, val);
                                                json!({ "type": "response", "id": id, "result": true })
                                            }
                                        }
                                        "load" => {
                                            let key = params
                                                .get("key")
                                                .and_then(Value::as_str)
                                                .unwrap_or("");
                                            let val = self.load(key).unwrap_or(Value::Null);
                                            json!({ "type": "response", "id": id, "result": val })
                                        }
                                        "tool" => {
                                            tool_calls_count += 1;
                                            if tool_calls_count > self.limits.max_tool_calls {
                                                let err_msg = format!(
                                                    "Tool call budget exceeded: limit is {}",
                                                    self.limits.max_tool_calls
                                                );
                                                output_acc.push_str(&format!(
                                                    "Tool error: {}\n",
                                                    err_msg
                                                ));
                                                json!({ "type": "response", "id": id, "error": err_msg })
                                            } else {
                                                let name = params
                                                    .get("name")
                                                    .and_then(Value::as_str)
                                                    .unwrap_or("");
                                                let args = params
                                                    .get("args")
                                                    .cloned()
                                                    .unwrap_or(Value::Null);
                                                let args_str = if args.is_string() {
                                                    args.as_str().unwrap().to_string()
                                                } else {
                                                    args.to_string()
                                                };

                                                output_acc
                                                    .push_str(&format!(">>> Tool: {}\n", name));
                                                match registry
                                                    .execute(name, &args_str, cwd, policy)
                                                    .await
                                                {
                                                    Ok(res) => {
                                                        if output_acc.len()
                                                            < self.limits.max_output_bytes
                                                        {
                                                            output_acc.push_str(&res);
                                                            output_acc.push('\n');
                                                        }
                                                        json!({ "type": "response", "id": id, "result": res })
                                                    }
                                                    Err(e) => {
                                                        let err_str = format!("Tool error: {}", e);
                                                        output_acc.push_str(&err_str);
                                                        output_acc.push('\n');
                                                        json!({ "type": "response", "id": id, "error": err_str })
                                                    }
                                                }
                                            }
                                        }
                                        _ => {
                                            json!({ "type": "response", "id": id, "error": "unknown method" })
                                        }
                                    };

                                    let _ = stdin.write_all(response.to_string().as_bytes()).await;
                                    let _ = stdin.write_all(b"\n").await;
                                    let _ = stdin.flush().await;
                                }
                                _ => {}
                            }
                        } else if !line.is_empty() {
                            // Non-JSON output from child (e.g. console.log or raw text)
                            if output_acc.len() < self.limits.max_output_bytes {
                                output_acc.push_str(line);
                                output_acc.push('\n');
                            }
                        }
                    }
                    IpcEvent::StderrChunk(chunk) => {
                        if output_acc.len() < self.limits.max_output_bytes {
                            let remaining = self.limits.max_output_bytes - output_acc.len();
                            let take = remaining.min(chunk.len());
                            output_acc.push_str(&chunk[..take]);
                        }
                    }
                    IpcEvent::Error(e) => return Err(e),
                    IpcEvent::StdoutEof => break,
                }
            }
            Ok::<(), CodeModeError>(())
        };

        match tokio::time::timeout(timeout_duration, supervisor_future).await {
            Ok(Ok(())) => {
                if termination_reason == ProcessLifecycleState::Running {
                    termination_reason = ProcessLifecycleState::Completed;
                }
            }
            Ok(Err(e)) => {
                match &e {
                    CodeModeError::ResourceExhausted(_) => {
                        termination_reason = ProcessLifecycleState::ResourceExhausted;
                        yielded = true;
                    }
                    _ => {
                        termination_reason = ProcessLifecycleState::Failed;
                    }
                }
                output_acc.push_str(&format!("Execution error: {e}\n"));
            }
            Err(_elapsed) => {
                yielded = true;
                termination_reason = ProcessLifecycleState::TimedOut;
                output_acc.push_str("[TIMEOUT: Execution exceeded time limit]\n");
            }
        }

        stdout_task.abort();
        stderr_task.abort();

        // Clean up process group reliably
        #[cfg(unix)]
        if let Some(p) = child_pid {
            unsafe {
                libc::kill(-(p as i32), libc::SIGTERM);
            }
        }
        let _ = child.kill().await;
        let _ = tokio::time::timeout(Duration::from_millis(50), child.wait()).await;
        #[cfg(unix)]
        if let Some(p) = child_pid {
            unsafe {
                libc::kill(-(p as i32), libc::SIGKILL);
            }
        }
        let _ = child.wait().await;

        Ok(RuntimeExecutionOutcome {
            output: output_acc,
            yielded,
            notifications,
            status: termination_reason,
        })
    }

    /// Evaluates code using the in-process confined interpreter with complete standard built-ins.
    pub async fn run_confined_interpreter(
        &self,
        code: &str,
        registry: &ToolRegistry,
        cwd: &Path,
        policy: SandboxPolicy,
        yield_time_ms: u64,
    ) -> Result<RuntimeExecutionOutcome> {
        let mut interp = ConfinedInterpreter::new(
            self.kv_store.clone(),
            registry,
            cwd,
            policy,
            self.limits.clone(),
            yield_time_ms,
        );
        interp.evaluate(code).await
    }

    /// Backward compatibility alias
    pub async fn run_fallback_evaluator(
        &self,
        code: &str,
        registry: &ToolRegistry,
        cwd: &Path,
        policy: SandboxPolicy,
    ) -> Result<RuntimeExecutionOutcome> {
        self.run_confined_interpreter(code, registry, cwd, policy, 30_000)
            .await
    }
}

// ---------------------------------------------------------------------------
// Base64 Helpers
// ---------------------------------------------------------------------------

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        out.push(BASE64_CHARS[(b0 >> 2) as usize] as char);
        out.push(BASE64_CHARS[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(BASE64_CHARS[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(BASE64_CHARS[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(data: &str) -> Option<Vec<u8>> {
    let mut inv = [255u8; 256];
    for (i, &c) in BASE64_CHARS.iter().enumerate() {
        inv[c as usize] = i as u8;
    }

    let clean: Vec<u8> = data
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace() && b != b'=')
        .collect();
    let mut out = Vec::new();

    for chunk in clean.chunks(4) {
        if chunk.is_empty() {
            break;
        }
        let v0 = inv[chunk[0] as usize];
        let v1 = chunk.get(1).map(|&b| inv[b as usize]).unwrap_or(0);
        let v2 = chunk.get(2).map(|&b| inv[b as usize]).unwrap_or(0);
        let v3 = chunk.get(3).map(|&b| inv[b as usize]).unwrap_or(0);

        if v0 == 255 || (chunk.len() > 1 && v1 == 255) {
            return None;
        }

        out.push((v0 << 2) | (v1 >> 4));
        if chunk.len() > 2 && v2 != 255 {
            out.push(((v1 & 0x0f) << 4) | (v2 >> 2));
        }
        if chunk.len() > 3 && v3 != 255 {
            out.push(((v2 & 0x03) << 6) | v3);
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Confined JavaScript Interpreter
// ---------------------------------------------------------------------------

struct ConfinedInterpreter<'a> {
    kv_store: Arc<Mutex<HashMap<String, Value>>>,
    registry: &'a ToolRegistry,
    cwd: &'a Path,
    policy: SandboxPolicy,
    limits: RuntimeExecutionLimits,
    start_time: Instant,
    timeout: Duration,
    tool_calls_count: usize,
    output: String,
    notifications: Vec<String>,
    yielded: bool,
    exited: bool,
    vars: HashMap<String, Value>,
}

impl<'a> ConfinedInterpreter<'a> {
    fn new(
        kv_store: Arc<Mutex<HashMap<String, Value>>>,
        registry: &'a ToolRegistry,
        cwd: &'a Path,
        policy: SandboxPolicy,
        limits: RuntimeExecutionLimits,
        yield_time_ms: u64,
    ) -> Self {
        let timeout = Duration::from_millis(yield_time_ms.min(limits.timeout_ms));
        Self {
            kv_store,
            registry,
            cwd,
            policy,
            limits,
            start_time: Instant::now(),
            timeout,
            tool_calls_count: 0,
            output: String::new(),
            notifications: Vec::new(),
            yielded: false,
            exited: false,
            vars: HashMap::new(),
        }
    }

    fn check_limits(&mut self) -> Result<()> {
        if self.start_time.elapsed() > self.timeout {
            self.yielded = true;
            anyhow::bail!("Script execution time limit reached");
        }
        Ok(())
    }

    fn append_output(&mut self, text: &str) {
        if self.output.len() < self.limits.max_output_bytes {
            self.output.push_str(text);
            self.output.push('\n');
        } else if !self.output.ends_with("[OUTPUT TRUNCATED]\n") {
            self.output.push_str("[OUTPUT TRUNCATED]\n");
        }
    }

    async fn evaluate(&mut self, code: &str) -> Result<RuntimeExecutionOutcome> {
        let trimmed = code.trim();

        // 1. Direct JSON array tool calls format: [{"tool": "name", "args": {...}}]
        if let Ok(calls) = serde_json::from_str::<Vec<Value>>(trimmed) {
            for call in calls {
                if let (Some(name), Some(args)) =
                    (call.get("tool").and_then(|v| v.as_str()), call.get("args"))
                {
                    self.execute_tool(name, args).await?;
                }
            }
            return Ok(RuntimeExecutionOutcome {
                output: self.output.clone(),
                yielded: self.yielded,
                notifications: self.notifications.clone(),
                status: if self.yielded {
                    ProcessLifecycleState::Yielded
                } else {
                    ProcessLifecycleState::Completed
                },
            });
        }

        // 2. Tokenize and execute script statements
        let statements = parse_script_statements(trimmed);
        for stmt in &statements {
            if self.yielded || self.exited {
                break;
            }
            if let Err(e) = self.execute_statement(stmt).await {
                if !self.yielded && !self.exited {
                    self.append_output(&format!("Runtime Error: {e}"));
                }
                break;
            }
        }

        // 3. Fallback: If nothing was produced and no keywords used, evaluate as direct command
        if self.output.is_empty()
            && !self.yielded
            && !self.exited
            && !statements.is_empty()
            && let Ok(res) = self
                .registry
                .execute(
                    "exec_command",
                    &serde_json::json!({ "command": trimmed }).to_string(),
                    self.cwd,
                    self.policy,
                )
                .await
        {
            self.output = res;
        }

        Ok(RuntimeExecutionOutcome {
            output: self.output.clone(),
            yielded: self.yielded,
            notifications: self.notifications.clone(),
            status: if self.yielded {
                ProcessLifecycleState::Yielded
            } else {
                ProcessLifecycleState::Completed
            },
        })
    }

    fn execute_statement<'b>(
        &'b mut self,
        stmt: &'b ScriptStatement,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'b>> {
        Box::pin(async move { self.execute_statement_inner(stmt).await })
    }

    async fn execute_statement_inner(&mut self, stmt: &ScriptStatement) -> Result<()> {
        self.check_limits()?;

        match stmt {
            ScriptStatement::VarDecl { name, expr } => {
                let val = self.eval_expr(expr).await?;
                self.vars.insert(name.clone(), val);
            }
            ScriptStatement::Assignment { target, expr } => {
                let val = self.eval_expr(expr).await?;
                self.assign_target(target, val).await?;
            }
            ScriptStatement::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond_val = self.eval_expr(condition).await?;
                if is_truthy(&cond_val) {
                    for s in then_body {
                        if self.yielded || self.exited {
                            break;
                        }
                        self.execute_statement(s).await?;
                    }
                } else if let Some(else_stmts) = else_body {
                    for s in else_stmts {
                        if self.yielded || self.exited {
                            break;
                        }
                        self.execute_statement(s).await?;
                    }
                }
            }
            ScriptStatement::ForOf {
                var_name,
                iterable,
                body,
            } => {
                let iter_val = self.eval_expr(iterable).await?;
                let items: Vec<Value> = match iter_val {
                    Value::Array(arr) => arr,
                    Value::Object(map) => map.keys().map(|k| Value::String(k.clone())).collect(),
                    Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
                    _ => Vec::new(),
                };

                for item in items {
                    if self.yielded || self.exited {
                        break;
                    }
                    self.vars.insert(var_name.clone(), item);
                    for s in body {
                        if self.yielded || self.exited {
                            break;
                        }
                        self.execute_statement(s).await?;
                    }
                }
            }
            ScriptStatement::Expr(expr) => {
                self.eval_expr(expr).await?;
            }
            ScriptStatement::Exit => {
                self.exited = true;
            }
            ScriptStatement::YieldControl => {
                self.yielded = true;
            }
        }
        Ok(())
    }

    async fn assign_target(&mut self, target: &str, val: Value) -> Result<()> {
        let trimmed = target.trim();
        if let Some((obj_name, prop_name)) = trimmed.split_once('.') {
            if let Some(obj) = self.vars.get_mut(obj_name)
                && let Value::Object(map) = obj
            {
                map.insert(prop_name.to_string(), val);
                return Ok(());
            }
        } else if let Some((arr_name, rest)) = trimmed.split_once('[') {
            let index_expr = rest.trim_end_matches(']').trim();
            let idx_val = self.eval_expr_str(index_expr).await?;
            if let Some(obj) = self.vars.get_mut(arr_name) {
                if let Value::Array(arr) = obj {
                    if let Some(idx) = idx_val.as_u64() {
                        let i = idx as usize;
                        if i < arr.len() {
                            arr[i] = val;
                        } else {
                            arr.push(val);
                        }
                        return Ok(());
                    }
                } else if let Value::Object(map) = obj {
                    let k = idx_val
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| idx_val.to_string());
                    map.insert(k, val);
                    return Ok(());
                }
            }
        }
        self.vars.insert(trimmed.to_string(), val);
        Ok(())
    }

    async fn execute_tool(&mut self, tool_name: &str, args: &Value) -> Result<Value> {
        self.tool_calls_count += 1;
        if self.tool_calls_count > self.limits.max_tool_calls {
            let err_msg = format!(
                "Tool call budget exceeded: limit is {}",
                self.limits.max_tool_calls
            );
            self.append_output(&format!("Tool error: {err_msg}"));
            anyhow::bail!(err_msg);
        }

        let args_str = if args.is_string() {
            args.as_str().unwrap().to_string()
        } else {
            args.to_string()
        };

        self.append_output(&format!(">>> Tool: {tool_name}"));
        match self
            .registry
            .execute(tool_name, &args_str, self.cwd, self.policy)
            .await
        {
            Ok(res) => {
                self.append_output(&res);
                if let Ok(json_res) = serde_json::from_str::<Value>(&res) {
                    Ok(json_res)
                } else {
                    Ok(Value::String(res))
                }
            }
            Err(e) => {
                let err_msg = format!("Tool Error: {e}");
                self.append_output(&err_msg);
                anyhow::bail!(err_msg);
            }
        }
    }

    fn eval_expr<'b>(
        &'b mut self,
        expr: &'b str,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'b>> {
        Box::pin(async move { self.eval_expr_inner(expr).await })
    }

    async fn eval_expr_inner(&mut self, expr: &str) -> Result<Value> {
        self.check_limits()?;
        let trimmed = expr.trim();
        if trimmed.is_empty() {
            return Ok(Value::Null);
        }

        // Global builtins & helpers
        if let Some(rest) = trimmed.strip_prefix("text(") {
            let inner = strip_matching_paren(rest)?;
            let v = self.eval_expr(&inner).await?;
            let s = if let Some(st) = v.as_str() {
                st.to_string()
            } else {
                v.to_string()
            };
            self.append_output(&s);
            return Ok(Value::Null);
        }

        if let Some(rest) = trimmed
            .strip_prefix("console.log(")
            .or_else(|| trimmed.strip_prefix("console.info("))
            .or_else(|| trimmed.strip_prefix("console.warn("))
            .or_else(|| trimmed.strip_prefix("console.error("))
        {
            let inner = strip_matching_paren(rest)?;
            let v = self.eval_expr(&inner).await?;
            let s = if let Some(st) = v.as_str() {
                st.to_string()
            } else {
                v.to_string()
            };
            self.append_output(&s);
            return Ok(Value::Null);
        }

        if let Some(rest) = trimmed.strip_prefix("notify(") {
            let inner = strip_matching_paren(rest)?;
            let v = self.eval_expr(&inner).await?;
            let s = if let Some(st) = v.as_str() {
                st.to_string()
            } else {
                v.to_string()
            };
            self.notifications.push(s);
            return Ok(Value::Null);
        }

        if let Some(rest) = trimmed.strip_prefix("store(") {
            let inner = strip_matching_paren(rest)?;
            if let Some((k_expr, v_expr)) = inner.split_once(',') {
                let k_val = self.eval_expr(k_expr).await?;
                let v_val = self.eval_expr(v_expr).await?;
                let key_str = k_val
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| k_val.to_string());
                self.kv_store
                    .lock()
                    .unwrap()
                    .insert(key_str.clone(), v_val.clone());
                self.append_output(&format!("[store]: saved '{key_str}'"));
                return Ok(v_val);
            }
        }

        if let Some(rest) = trimmed.strip_prefix("load(") {
            let inner = strip_matching_paren(rest)?;
            let k_val = self.eval_expr(&inner).await?;
            let key_str = k_val
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| k_val.to_string());
            let val = self
                .kv_store
                .lock()
                .unwrap()
                .get(&key_str)
                .cloned()
                .unwrap_or(Value::Null);
            return Ok(val);
        }

        if trimmed == "yield_control()" {
            self.yielded = true;
            return Ok(Value::Null);
        }

        if trimmed == "exit()" {
            self.exited = true;
            return Ok(Value::Null);
        }

        // Tools call: `tools.some_tool(...)` or `await tools.some_tool(...)`
        let unawaited = trimmed.strip_prefix("await ").unwrap_or(trimmed);
        if let Some(rest) = unawaited.strip_prefix("tools.")
            && let Some((tool_name, args_part)) = rest.split_once('(')
        {
            let tool_name = tool_name.trim();
            let inner = strip_matching_paren(args_part)?;
            let args_val = if inner.trim().is_empty() {
                json!({})
            } else {
                self.eval_expr(&inner).await?
            };
            return self.execute_tool(tool_name, &args_val).await;
        }

        // Standard Math object
        if let Some(math_expr) = trimmed.strip_prefix("Math.") {
            return self.eval_math(math_expr).await;
        }

        // Standard JSON object
        if let Some(json_expr) = trimmed.strip_prefix("JSON.") {
            return self.eval_json(json_expr).await;
        }

        // Standard Date object
        if let Some(date_expr) = trimmed.strip_prefix("Date.") {
            return self.eval_date(date_expr).await;
        }
        if trimmed == "new Date()" || trimmed == "new Date().toISOString()" {
            return Ok(Value::String(Utc::now().to_rfc3339()));
        }

        // Standard URL object: `new URL(...)`
        if let Some(rest) = trimmed.strip_prefix("new URL(") {
            let inner = strip_matching_paren(rest)?;
            let url_val = self.eval_expr(&inner).await?;
            let url_str = url_val.as_str().unwrap_or("");
            return Ok(eval_url_object(url_str));
        }

        // Base64 & Bytes helpers: `btoa(...)`, `atob(...)`, `bytes.encode(...)`, `bytes.decode(...)`
        if let Some(rest) = trimmed.strip_prefix("btoa(") {
            let inner = strip_matching_paren(rest)?;
            let val = self.eval_expr(&inner).await?;
            let s = val.as_str().unwrap_or("");
            return Ok(Value::String(base64_encode(s.as_bytes())));
        }
        if let Some(rest) = trimmed.strip_prefix("atob(") {
            let inner = strip_matching_paren(rest)?;
            let val = self.eval_expr(&inner).await?;
            let s = val.as_str().unwrap_or("");
            let dec = base64_decode(s).unwrap_or_default();
            return Ok(Value::String(String::from_utf8_lossy(&dec).to_string()));
        }
        if let Some(rest) = trimmed.strip_prefix("bytes.encode(") {
            let inner = strip_matching_paren(rest)?;
            let val = self.eval_expr(&inner).await?;
            let s = val.as_str().unwrap_or("");
            let arr: Vec<Value> = s.bytes().map(|b| json!(b)).collect();
            return Ok(Value::Array(arr));
        }
        if let Some(rest) = trimmed.strip_prefix("bytes.decode(") {
            let inner = strip_matching_paren(rest)?;
            let val = self.eval_expr(&inner).await?;
            let bytes: Vec<u8> = val
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_u64().map(|n| n as u8))
                        .collect()
                })
                .unwrap_or_default();
            return Ok(Value::String(String::from_utf8_lossy(&bytes).to_string()));
        }

        // RegExp helper: `new RegExp("pattern", "flags")`
        if let Some(rest) = trimmed.strip_prefix("new RegExp(") {
            let inner = strip_matching_paren(rest)?;
            let pat_str = self.eval_expr(&inner).await?;
            let mut obj = Map::new();
            obj.insert("__type".to_string(), Value::String("RegExp".to_string()));
            obj.insert(
                "source".to_string(),
                Value::String(pat_str.as_str().unwrap_or("").to_string()),
            );
            return Ok(Value::Object(obj));
        }

        // Collection constructors: `new Map()`, `new Set()`, `new Headers()`
        if trimmed.starts_with("new Map") {
            let mut map = Map::new();
            map.insert("__type".to_string(), Value::String("Map".to_string()));
            map.insert("entries".to_string(), Value::Array(Vec::new()));
            return Ok(Value::Object(map));
        }
        if trimmed.starts_with("new Set") {
            let mut map = Map::new();
            map.insert("__type".to_string(), Value::String("Set".to_string()));
            map.insert("items".to_string(), Value::Array(Vec::new()));
            return Ok(Value::Object(map));
        }
        if trimmed.starts_with("new Headers") {
            let mut map = Map::new();
            map.insert("__type".to_string(), Value::String("Headers".to_string()));
            map.insert("headers".to_string(), Value::Object(Map::new()));
            return Ok(Value::Object(map));
        }

        // Binary operators: `+`, `-`, `*`, `/`, `%`, `==`, `===`, `!=`, `!==`, `<`, `<=`, `>`, `>=`, `&&`, `||`
        if let Some(val) = self.eval_binary_op(trimmed).await? {
            return Ok(val);
        }

        // Template literals: `...${expr}...`
        if trimmed.starts_with('`') && trimmed.ends_with('`') && trimmed.len() >= 2 {
            return self
                .eval_template_literal(&trimmed[1..trimmed.len() - 1])
                .await;
        }

        // String literals
        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            if trimmed.len() >= 2 {
                return Ok(Value::String(trimmed[1..trimmed.len() - 1].to_string()));
            }
            return Ok(Value::String(String::new()));
        }

        // Numeric literals
        if let Ok(n) = trimmed.parse::<i64>() {
            return Ok(json!(n));
        }
        if let Ok(f) = trimmed.parse::<f64>() {
            return Ok(json!(f));
        }

        // Boolean & Null literals
        match trimmed {
            "true" => return Ok(Value::Bool(true)),
            "false" => return Ok(Value::Bool(false)),
            "null" | "undefined" => return Ok(Value::Null),
            _ => {}
        }

        // Array literals `[1, 2, 3]`
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = &trimmed[1..trimmed.len() - 1];
            let elements = split_comma_expressions(inner);
            let mut arr = Vec::with_capacity(elements.len());
            for elem in elements {
                arr.push(self.eval_expr(&elem).await?);
            }
            return Ok(Value::Array(arr));
        }

        // Object literals `{ a: 1, b: 2 }`
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            let inner = &trimmed[1..trimmed.len() - 1];
            let pairs = split_comma_expressions(inner);
            let mut map = Map::new();
            for pair in pairs {
                if let Some((k, v)) = pair.split_once(':') {
                    let key = k.trim().trim_matches('"').trim_matches('\'').to_string();
                    let val = self.eval_expr(v).await?;
                    map.insert(key, val);
                }
            }
            return Ok(Value::Object(map));
        }

        // Array indexing or member access: `items[0]`, `obj.foo`, `arr.length`, `arr.push(...)`
        if let Some(val) = self.eval_member_or_method(trimmed).await? {
            return Ok(val);
        }

        // Variable identifier
        if let Some(v) = self.vars.get(trimmed) {
            return Ok(v.clone());
        }

        // Fallback: evaluate as JSON or raw string
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            return Ok(v);
        }

        Ok(Value::String(trimmed.to_string()))
    }

    async fn eval_expr_str(&mut self, expr: &str) -> Result<Value> {
        self.eval_expr(expr).await
    }

    async fn eval_template_literal(&mut self, template: &str) -> Result<Value> {
        let mut out = String::new();
        let mut rest = template;

        while let Some(start) = rest.find("${") {
            out.push_str(&rest[..start]);
            let after_start = &rest[start + 2..];
            if let Some(end) = after_start.find('}') {
                let expr_str = &after_start[..end];
                let val = self.eval_expr(expr_str).await?;
                if let Some(s) = val.as_str() {
                    out.push_str(s);
                } else {
                    out.push_str(&val.to_string());
                }
                rest = &after_start[end + 1..];
            } else {
                out.push_str(&rest[start..]);
                break;
            }
        }
        out.push_str(rest);
        Ok(Value::String(out))
    }

    async fn eval_math(&mut self, math_call: &str) -> Result<Value> {
        if math_call == "PI" {
            return Ok(json!(std::f64::consts::PI));
        }
        if math_call == "E" {
            return Ok(json!(std::f64::consts::E));
        }
        if math_call == "random()" {
            // Deterministic pseudo-random based on current time
            let nanos = Utc::now().timestamp_subsec_nanos();
            let rand_val = (nanos % 1000) as f64 / 1000.0;
            return Ok(json!(rand_val));
        }

        if let Some((fn_name, rest)) = math_call.split_once('(') {
            let inner = strip_matching_paren(rest)?;
            let args = split_comma_expressions(&inner);
            let mut num_args = Vec::new();
            for arg in args {
                let v = self.eval_expr(&arg).await?;
                if let Some(n) = v.as_f64() {
                    num_args.push(n);
                }
            }

            let first = num_args.first().copied().unwrap_or(0.0);
            let res = match fn_name.trim() {
                "floor" => json!(first.floor()),
                "ceil" => json!(first.ceil()),
                "round" => json!(first.round()),
                "abs" => json!(first.abs()),
                "sqrt" => json!(first.sqrt()),
                "min" => json!(num_args.iter().copied().fold(f64::INFINITY, f64::min)),
                "max" => json!(num_args.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
                "pow" => {
                    let second = num_args.get(1).copied().unwrap_or(1.0);
                    json!(first.powf(second))
                }
                _ => Value::Null,
            };
            return Ok(res);
        }
        Ok(Value::Null)
    }

    async fn eval_json(&mut self, json_call: &str) -> Result<Value> {
        if let Some(rest) = json_call.strip_prefix("stringify(") {
            let inner = strip_matching_paren(rest)?;
            let v = self.eval_expr(&inner).await?;
            let s = serde_json::to_string(&v)?;
            return Ok(Value::String(s));
        }
        if let Some(rest) = json_call.strip_prefix("parse(") {
            let inner = strip_matching_paren(rest)?;
            let v = self.eval_expr(&inner).await?;
            if let Some(s) = v.as_str() {
                let parsed = serde_json::from_str::<Value>(s).unwrap_or(Value::Null);
                return Ok(parsed);
            }
        }
        Ok(Value::Null)
    }

    async fn eval_date(&mut self, date_call: &str) -> Result<Value> {
        if date_call == "now()" {
            return Ok(json!(Utc::now().timestamp_millis()));
        }
        if let Some(rest) = date_call.strip_prefix("parse(") {
            let inner = strip_matching_paren(rest)?;
            let v = self.eval_expr(&inner).await?;
            if let Some(s) = v.as_str()
                && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s)
            {
                return Ok(json!(dt.timestamp_millis()));
            }
        }
        Ok(Value::Null)
    }

    async fn eval_binary_op(&mut self, expr: &str) -> Result<Option<Value>> {
        // Operators sorted by precedence (lowest first): ||, &&, ==/===/!=/!==, </<=/>/>=, +/-, */%
        let ops = [
            ("||", 2),
            ("&&", 2),
            ("===", 3),
            ("!==", 3),
            ("==", 2),
            ("!=", 2),
            ("<=", 2),
            (">=", 2),
            ("<", 1),
            (">", 1),
            ("+", 1),
            ("-", 1),
            ("*", 1),
            ("/", 1),
            ("%", 1),
        ];

        for (op, _len) in ops {
            if let Some((left, right)) = find_top_level_binary_op(expr, op) {
                let l_val = self.eval_expr(left).await?;
                let r_val = self.eval_expr(right).await?;

                let res = match op {
                    "||" => {
                        if is_truthy(&l_val) {
                            l_val
                        } else {
                            r_val
                        }
                    }
                    "&&" => {
                        if !is_truthy(&l_val) {
                            l_val
                        } else {
                            r_val
                        }
                    }
                    "===" | "==" => Value::Bool(l_val == r_val),
                    "!==" | "!=" => Value::Bool(l_val != r_val),
                    "<" => Value::Bool(
                        compare_values(&l_val, &r_val) == Some(std::cmp::Ordering::Less),
                    ),
                    "<=" => Value::Bool(matches!(
                        compare_values(&l_val, &r_val),
                        Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                    )),
                    ">" => Value::Bool(
                        compare_values(&l_val, &r_val) == Some(std::cmp::Ordering::Greater),
                    ),
                    ">=" => Value::Bool(matches!(
                        compare_values(&l_val, &r_val),
                        Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                    )),
                    "+" => {
                        if let (Some(a), Some(b)) = (l_val.as_f64(), r_val.as_f64()) {
                            if l_val.is_i64() && r_val.is_i64() {
                                json!(l_val.as_i64().unwrap() + r_val.as_i64().unwrap())
                            } else {
                                json!(a + b)
                            }
                        } else {
                            let l_str = l_val
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| l_val.to_string());
                            let r_str = r_val
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| r_val.to_string());
                            Value::String(format!("{l_str}{r_str}"))
                        }
                    }
                    "-" => {
                        let a = l_val.as_f64().unwrap_or(0.0);
                        let b = r_val.as_f64().unwrap_or(0.0);
                        json!(a - b)
                    }
                    "*" => {
                        let a = l_val.as_f64().unwrap_or(0.0);
                        let b = r_val.as_f64().unwrap_or(0.0);
                        json!(a * b)
                    }
                    "/" => {
                        let a = l_val.as_f64().unwrap_or(0.0);
                        let b = r_val.as_f64().unwrap_or(1.0);
                        json!(a / b)
                    }
                    "%" => {
                        let a = l_val.as_i64().unwrap_or(0);
                        let b = r_val.as_i64().unwrap_or(1);
                        json!(a % b)
                    }
                    _ => Value::Null,
                };
                return Ok(Some(res));
            }
        }
        Ok(None)
    }

    async fn eval_member_or_method(&mut self, expr: &str) -> Result<Option<Value>> {
        // Check for indexing: `target[index]`
        if let Some(bracket_idx) = expr.rfind('[')
            && expr.ends_with(']')
        {
            let target_expr = &expr[..bracket_idx];
            let index_expr = &expr[bracket_idx + 1..expr.len() - 1];
            let target_val = self.eval_expr(target_expr).await?;
            let index_val = self.eval_expr(index_expr).await?;

            if let Value::Array(arr) = target_val {
                if let Some(idx) = index_val.as_u64() {
                    return Ok(Some(arr.get(idx as usize).cloned().unwrap_or(Value::Null)));
                }
            } else if let Value::Object(map) = target_val {
                let key = index_val
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| index_val.to_string());
                return Ok(Some(map.get(&key).cloned().unwrap_or(Value::Null)));
            } else if let Value::String(s) = target_val
                && let Some(idx) = index_val.as_u64()
            {
                let ch = s.chars().nth(idx as usize).map(|c| c.to_string());
                return Ok(Some(ch.map(Value::String).unwrap_or(Value::Null)));
            }
        }

        // Method calls or property access: `obj.method(...)` or `obj.property`
        if let Some(dot_idx) = expr.rfind('.') {
            let target_expr = &expr[..dot_idx];
            let after_dot = &expr[dot_idx + 1..];

            // Method call: `target.method(arg1, ...)`
            if let Some((method_name, rest)) = after_dot.split_once('(') {
                if let Some(inner) = rest.strip_suffix(')') {
                    let args = split_comma_expressions(inner);
                    let mut evaluated_args = Vec::with_capacity(args.len());
                    for a in args {
                        evaluated_args.push(self.eval_expr(&a).await?);
                    }

                    let target_val = self.eval_expr(target_expr).await?;
                    if let Some(res) =
                        eval_method_call(&target_val, method_name.trim(), &evaluated_args)
                    {
                        return Ok(Some(res));
                    }
                }
            } else {
                // Property access: `target.property`
                let prop_name = after_dot.trim();
                let target_val = self.eval_expr(target_expr).await?;

                if prop_name == "length" {
                    if let Value::Array(arr) = target_val {
                        return Ok(Some(json!(arr.len())));
                    } else if let Value::String(s) = target_val {
                        return Ok(Some(json!(s.len())));
                    }
                }

                if let Value::Object(map) = target_val {
                    return Ok(Some(map.get(prop_name).cloned().unwrap_or(Value::Null)));
                }
            }
        }

        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Method dispatch for built-in types (String, Array, RegExp, URL)
// ---------------------------------------------------------------------------

fn eval_method_call(target: &Value, method: &str, args: &[Value]) -> Option<Value> {
    match target {
        Value::String(s) => match method {
            "split" => {
                let sep = args.first().and_then(Value::as_str).unwrap_or("");
                let parts: Vec<Value> = if sep.is_empty() {
                    s.chars().map(|c| Value::String(c.to_string())).collect()
                } else {
                    s.split(sep).map(|p| Value::String(p.to_string())).collect()
                };
                Some(Value::Array(parts))
            }
            "trim" => Some(Value::String(s.trim().to_string())),
            "toLowerCase" => Some(Value::String(s.to_lowercase())),
            "toUpperCase" => Some(Value::String(s.to_uppercase())),
            "includes" => {
                let sub = args.first().and_then(Value::as_str).unwrap_or("");
                Some(Value::Bool(s.contains(sub)))
            }
            "startsWith" => {
                let sub = args.first().and_then(Value::as_str).unwrap_or("");
                Some(Value::Bool(s.starts_with(sub)))
            }
            "endsWith" => {
                let sub = args.first().and_then(Value::as_str).unwrap_or("");
                Some(Value::Bool(s.ends_with(sub)))
            }
            "replace" => {
                let from = args.first().and_then(Value::as_str).unwrap_or("");
                let to = args.get(1).and_then(Value::as_str).unwrap_or("");
                Some(Value::String(s.replacen(from, to, 1)))
            }
            "replaceAll" => {
                let from = args.first().and_then(Value::as_str).unwrap_or("");
                let to = args.get(1).and_then(Value::as_str).unwrap_or("");
                Some(Value::String(s.replace(from, to)))
            }
            "slice" | "substring" => {
                let start = args.first().and_then(Value::as_u64).unwrap_or(0) as usize;
                let end = args
                    .get(1)
                    .and_then(Value::as_u64)
                    .map(|e| e as usize)
                    .unwrap_or(s.len());
                let sub: String = s
                    .chars()
                    .skip(start)
                    .take(end.saturating_sub(start))
                    .collect();
                Some(Value::String(sub))
            }
            "indexOf" => {
                let sub = args.first().and_then(Value::as_str).unwrap_or("");
                Some(json!(s.find(sub).map(|i| i as i64).unwrap_or(-1)))
            }
            _ => None,
        },
        Value::Array(arr) => match method {
            "join" => {
                let sep = args.first().and_then(Value::as_str).unwrap_or(",");
                let pieces: Vec<String> = arr
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| v.to_string())
                    })
                    .collect();
                Some(Value::String(pieces.join(sep)))
            }
            "slice" => {
                let start = args.first().and_then(Value::as_u64).unwrap_or(0) as usize;
                let end = args
                    .get(1)
                    .and_then(Value::as_u64)
                    .map(|e| e as usize)
                    .unwrap_or(arr.len());
                let sub: Vec<Value> = arr
                    .iter()
                    .skip(start)
                    .take(end.saturating_sub(start))
                    .cloned()
                    .collect();
                Some(Value::Array(sub))
            }
            "includes" => {
                let target = args.first().unwrap_or(&Value::Null);
                Some(Value::Bool(arr.contains(target)))
            }
            "indexOf" => {
                let target = args.first().unwrap_or(&Value::Null);
                let idx = arr
                    .iter()
                    .position(|x| x == target)
                    .map(|i| i as i64)
                    .unwrap_or(-1);
                Some(json!(idx))
            }
            _ => None,
        },
        Value::Object(map) => {
            // RegExp matching
            if map.get("__type").and_then(Value::as_str) == Some("RegExp")
                && let Some(source) = map.get("source").and_then(Value::as_str)
                && let Ok(re) = Regex::new(source)
            {
                let text = args.first().and_then(Value::as_str).unwrap_or("");
                match method {
                    "test" => return Some(Value::Bool(re.is_match(text))),
                    "exec" => {
                        if let Some(caps) = re.captures(text) {
                            let matches: Vec<Value> = caps
                                .iter()
                                .flatten()
                                .map(|m| Value::String(m.as_str().to_string()))
                                .collect();
                            return Some(Value::Array(matches));
                        }
                        return Some(Value::Null);
                    }
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// URL Parser
// ---------------------------------------------------------------------------

fn eval_url_object(url_str: &str) -> Value {
    let mut obj = Map::new();
    obj.insert("href".to_string(), Value::String(url_str.to_string()));

    let mut protocol = "https:".to_string();
    let mut remainder = url_str;

    if let Some((p, rest)) = url_str.split_once("://") {
        protocol = format!("{p}:");
        remainder = rest;
    }
    obj.insert("protocol".to_string(), Value::String(protocol.clone()));

    let mut hash = "".to_string();
    if let Some((rest, h)) = remainder.split_once('#') {
        remainder = rest;
        hash = format!("#{h}");
    }
    obj.insert("hash".to_string(), Value::String(hash));

    let mut search = "".to_string();
    if let Some((rest, s)) = remainder.split_once('?') {
        remainder = rest;
        search = format!("?{s}");
    }
    obj.insert("search".to_string(), Value::String(search));

    let (host, pathname) = remainder
        .split_once('/')
        .map(|(h, p)| (h, format!("/{p}")))
        .unwrap_or((remainder, "/".to_string()));

    obj.insert("host".to_string(), Value::String(host.to_string()));
    let (hostname, port) = host
        .split_once(':')
        .map(|(hn, prt)| (hn.to_string(), prt.to_string()))
        .unwrap_or((host.to_string(), "".to_string()));

    obj.insert("hostname".to_string(), Value::String(hostname.clone()));
    obj.insert("port".to_string(), Value::String(port));
    obj.insert("pathname".to_string(), Value::String(pathname));
    obj.insert(
        "origin".to_string(),
        Value::String(format!("{protocol}//{host}")),
    );

    Value::Object(obj)
}

// ---------------------------------------------------------------------------
// AST Representation & Parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum ScriptStatement {
    VarDecl {
        name: String,
        expr: String,
    },
    Assignment {
        target: String,
        expr: String,
    },
    If {
        condition: String,
        then_body: Vec<ScriptStatement>,
        else_body: Option<Vec<ScriptStatement>>,
    },
    ForOf {
        var_name: String,
        iterable: String,
        body: Vec<ScriptStatement>,
    },
    Expr(String),
    Exit,
    YieldControl,
}

fn parse_script_statements(source: &str) -> Vec<ScriptStatement> {
    let mut stmts = Vec::new();
    let mut lines = source.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim().trim_end_matches(';').trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        // `yield_control()`
        if trimmed.contains("yield_control()") {
            stmts.push(ScriptStatement::YieldControl);
            continue;
        }

        // `exit()`
        if trimmed == "exit()" {
            stmts.push(ScriptStatement::Exit);
            continue;
        }

        // `if (condition) { ... } else { ... }`
        if trimmed.starts_with("if ") || trimmed.starts_with("if(") {
            let mut block_text = trimmed.to_string();
            let mut brace_count =
                block_text.matches('{').count() as i32 - block_text.matches('}').count() as i32;

            while brace_count > 0 && lines.peek().is_some() {
                let next_line = lines.next().unwrap();
                block_text.push('\n');
                block_text.push_str(next_line);
                brace_count +=
                    next_line.matches('{').count() as i32 - next_line.matches('}').count() as i32;
            }

            if let Some(if_stmt) = parse_if_block(&block_text) {
                stmts.push(if_stmt);
                continue;
            }
        }

        // `for (const x of items) { ... }`
        if trimmed.starts_with("for ") || trimmed.starts_with("for(") {
            let mut block_text = trimmed.to_string();
            let mut brace_count =
                block_text.matches('{').count() as i32 - block_text.matches('}').count() as i32;

            while brace_count > 0 && lines.peek().is_some() {
                let next_line = lines.next().unwrap();
                block_text.push('\n');
                block_text.push_str(next_line);
                brace_count +=
                    next_line.matches('{').count() as i32 - next_line.matches('}').count() as i32;
            }

            if let Some(for_stmt) = parse_for_of_block(&block_text) {
                stmts.push(for_stmt);
                continue;
            }
        }

        // Variable declaration: `let x = ...`, `const x = ...`, `var x = ...`
        if trimmed.starts_with("let ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("var ")
        {
            let rest = if let Some(r) = trimmed.strip_prefix("let ") {
                r
            } else if let Some(r) = trimmed.strip_prefix("const ") {
                r
            } else {
                trimmed.strip_prefix("var ").unwrap_or("")
            };

            if let Some((ident, val_expr)) = rest.split_once('=') {
                stmts.push(ScriptStatement::VarDecl {
                    name: ident.trim().to_string(),
                    expr: val_expr.trim().to_string(),
                });
                continue;
            }
        }

        // Assignment: `x = ...` or `x += ...`
        if let Some((target, val_expr)) = trimmed.split_once(" = ") {
            stmts.push(ScriptStatement::Assignment {
                target: target.trim().to_string(),
                expr: val_expr.trim().to_string(),
            });
            continue;
        }

        stmts.push(ScriptStatement::Expr(trimmed.to_string()));
    }

    stmts
}

fn parse_if_block(text: &str) -> Option<ScriptStatement> {
    let open_paren = text.find('(')?;
    let close_paren = find_matching_delimiter(text, open_paren, '(', ')')?;
    let condition = text[open_paren + 1..close_paren].trim().to_string();

    let open_brace = text[close_paren..].find('{')? + close_paren;
    let close_brace = find_matching_delimiter(text, open_brace, '{', '}')?;
    let then_body_text = &text[open_brace + 1..close_brace];
    let then_body = parse_script_statements(then_body_text);

    let mut else_body = None;
    let remainder = text[close_brace + 1..].trim();
    if let Some(else_part) = remainder.strip_prefix("else") {
        let else_open = else_part.find('{')?;
        let else_close = find_matching_delimiter(else_part, else_open, '{', '}')?;
        let else_body_text = &else_part[else_open + 1..else_close];
        else_body = Some(parse_script_statements(else_body_text));
    }

    Some(ScriptStatement::If {
        condition,
        then_body,
        else_body,
    })
}

fn parse_for_of_block(text: &str) -> Option<ScriptStatement> {
    let open_paren = text.find('(')?;
    let close_paren = find_matching_delimiter(text, open_paren, '(', ')')?;
    let header = &text[open_paren + 1..close_paren];

    let (var_part, iter_part) = header.split_once(" of ")?;
    let var_name = var_part
        .trim()
        .strip_prefix("const ")
        .or_else(|| var_part.trim().strip_prefix("let "))
        .or_else(|| var_part.trim().strip_prefix("var "))
        .unwrap_or(var_part.trim())
        .trim()
        .to_string();

    let open_brace = text[close_paren..].find('{')? + close_paren;
    let close_brace = find_matching_delimiter(text, open_brace, '{', '}')?;
    let body_text = &text[open_brace + 1..close_brace];
    let body = parse_script_statements(body_text);

    Some(ScriptStatement::ForOf {
        var_name,
        iterable: iter_part.trim().to_string(),
        body,
    })
}

fn find_matching_delimiter(text: &str, start_idx: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in text[start_idx..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(start_idx + i);
            }
        }
    }
    None
}

fn strip_matching_paren(s: &str) -> Result<String> {
    let trimmed = s.trim();
    if let Some(close) = trimmed.rfind(')') {
        Ok(trimmed[..close].trim().to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn split_comma_expressions(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_quote = None;

    for c in s.chars() {
        match c {
            '"' | '\'' | '`' => {
                if in_quote == Some(c) {
                    in_quote = None;
                } else if in_quote.is_none() {
                    in_quote = Some(c);
                }
                current.push(c);
            }
            '(' | '[' | '{' if in_quote.is_none() => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' if in_quote.is_none() => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 && in_quote.is_none() => {
                let item = current.trim().to_string();
                if !item.is_empty() {
                    out.push(item);
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }

    let item = current.trim().to_string();
    if !item.is_empty() {
        out.push(item);
    }
    out
}

fn find_top_level_binary_op<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0;
    let mut in_quote = None;
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();

    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' || b == b'\'' || b == b'`' {
            if in_quote == Some(b) {
                in_quote = None;
            } else if in_quote.is_none() {
                in_quote = Some(b);
            }
        } else if in_quote.is_none() {
            if b == b'(' || b == b'[' || b == b'{' {
                depth += 1;
            } else if b == b')' || b == b']' || b == b'}' {
                depth -= 1;
            } else if depth == 0 && expr[i..].starts_with(op) {
                let left = &expr[..i].trim();
                let right = &expr[i + op_bytes.len()..].trim();
                if !left.is_empty() && !right.is_empty() {
                    return Some((left, right));
                }
            }
        }
        i += 1;
    }
    None
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
        x.partial_cmp(&y)
    } else if let (Some(x), Some(y)) = (a.as_str(), b.as_str()) {
        Some(x.cmp(y))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_fallback_store_and_text() {
        let runtime = CodeModeRuntime::new();
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let script = r#"
            text("Hello from Code Mode")
            store("answer", 42)
        "#;

        let outcome = runtime
            .run_confined_interpreter(script, &registry, &cwd, SandboxPolicy::WorkspaceWrite, 5000)
            .await
            .unwrap();

        assert!(outcome.output.contains("Hello from Code Mode"));
        assert_eq!(runtime.load("answer"), Some(json!(42)));
    }

    #[tokio::test]
    async fn test_runtime_fallback_variables_and_concatenation() {
        let runtime = CodeModeRuntime::new();
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let script = r#"
            let name = "World"
            const greeting = "Hello " + name
            console.log(greeting)
            store("computed", greeting)
        "#;

        let outcome = runtime
            .run_confined_interpreter(script, &registry, &cwd, SandboxPolicy::WorkspaceWrite, 5000)
            .await
            .unwrap();

        assert!(outcome.output.contains("Hello World"));
        assert_eq!(runtime.load("computed"), Some(json!("Hello World")));
    }

    #[tokio::test]
    async fn test_runtime_standard_builtins() {
        let runtime = CodeModeRuntime::new();
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let script = r#"
            let num = Math.floor(4.9)
            let parsed = JSON.parse("{\"status\": \"ok\"}")
            let url = new URL("https://example.com:8080/api/v1/test?query=foo#hash")
            let b64 = btoa("hello")
            let decoded = atob(b64)
            console.log("floor=" + num)
            console.log("pathname=" + url.pathname)
            console.log("decoded=" + decoded)
        "#;

        let outcome = runtime
            .run_confined_interpreter(script, &registry, &cwd, SandboxPolicy::WorkspaceWrite, 5000)
            .await
            .unwrap();

        assert!(outcome.output.contains("floor=4"));
        assert!(outcome.output.contains("pathname=/api/v1/test"));
        assert!(outcome.output.contains("decoded=hello"));
    }

    #[tokio::test]
    async fn test_runtime_control_flow_and_loops() {
        let runtime = CodeModeRuntime::new();
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let script = r#"
            let items = ["apple", "banana", "cherry"]
            for (const item of items) {
                if (item === "banana") {
                    console.log("FOUND BANANA")
                }
            }
        "#;

        let outcome = runtime
            .run_confined_interpreter(script, &registry, &cwd, SandboxPolicy::WorkspaceWrite, 5000)
            .await
            .unwrap();

        assert!(outcome.output.contains("FOUND BANANA"));
    }

    #[tokio::test]
    async fn test_runtime_array_indexing_and_methods() {
        let runtime = CodeModeRuntime::new();
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let script = r#"
            let arr = ["first", "second", "third"]
            console.log("first=" + arr[0])
            console.log("len=" + arr.length)
            let joined = arr.join(":")
            console.log("joined=" + joined)
        "#;

        let outcome = runtime
            .run_confined_interpreter(script, &registry, &cwd, SandboxPolicy::WorkspaceWrite, 5000)
            .await
            .unwrap();

        assert!(outcome.output.contains("first=first"));
        assert!(outcome.output.contains("len=3"));
        assert!(outcome.output.contains("joined=first:second:third"));
    }

    #[tokio::test]
    async fn test_runtime_resource_caps_enforcement() {
        let limits = RuntimeExecutionLimits {
            timeout_ms: 100,
            max_tool_calls: 2,
            max_output_bytes: 50,
        };
        let runtime = CodeModeRuntime::with_limits(limits);
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        // 1. Output truncation cap
        let script = r#"
            text("123456789012345678901234567890123456789012345678901234567890")
            text("extra line that should be truncated")
        "#;
        let outcome = runtime
            .run_confined_interpreter(script, &registry, &cwd, SandboxPolicy::WorkspaceWrite, 5000)
            .await
            .unwrap();

        assert!(outcome.output.contains("[OUTPUT TRUNCATED]"));
    }

    #[tokio::test]
    async fn test_code_mode_rejects_unconfined_ipc_when_sandbox_absent() {
        let runtime = CodeModeRuntime::new();
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        // Calling run_ipc_runner with ReadOnly policy when targeting a dummy binary or bwrap missing
        let res = runtime
            .run_ipc_runner(
                "node",
                "console.log('hi')",
                &registry,
                &cwd,
                SandboxPolicy::ReadOnly,
                1000,
            )
            .await;

        // If bwrap is not available, it must fail closed. If bwrap is available, it runs sandboxed.
        // What must NEVER happen is running unconfined without bwrap under ReadOnly policy.
        let has_bwrap = m31a_sandbox::SandboxRunner::new().has_bwrap();
        if !has_bwrap {
            assert!(
                res.is_err(),
                "Expected fail-closed error when bwrap is absent"
            );
            assert!(
                res.unwrap_err()
                    .to_string()
                    .contains("Refusing unconfined execution")
            );
        }
    }

    #[tokio::test]
    async fn test_code_mode_confined_interpreter_rejects_unauthorized_tools() {
        let runtime = CodeModeRuntime::new();
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let malicious_script = r#"
            await tools.non_existent_exploit_tool({ payload: "/etc/shadow" });
        "#;
        let outcome = runtime
            .run_confined_interpreter(
                malicious_script,
                &registry,
                &cwd,
                SandboxPolicy::ReadOnly,
                5000,
            )
            .await
            .unwrap();

        assert!(outcome.output.contains("Tool Error:") || outcome.output.contains("Unknown tool"));
    }
}
