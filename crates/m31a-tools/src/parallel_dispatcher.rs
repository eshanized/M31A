//! Sovereign parallel tool dispatch engine and concurrency governor.
//!
//! Concurrently dispatches non-conflicting read and probe tools to maximize execution throughput.
//! Executes batches of tool calls emitted in a single model turn with:
//! - Concurrency governance via asynchronous semaphores.
//! - State mutability classification (parallel read-only vs serialized mutating operations).
//! - Deterministic result order preservation matching original model call sequence.
//! - Per-tool timeout enforcement and failure isolation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// Classification of tool execution mutability and safety for concurrent scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolConcurrencyCategory {
    /// Pure read-only operation: safe to execute concurrently with any number of other reads.
    ReadOnlyConcurrent,
    /// Mutating operation modifying files, git, or process states: must be serialized.
    MutatingExclusive,
    /// Interactive or terminal command execution: strictly serialized with dedicated PTY access.
    InteractiveCommand,
}

impl ToolConcurrencyCategory {
    /// Classify a tool name into its concurrency category.
    pub fn classify(tool_name: &str) -> Self {
        match tool_name {
            // Read-only tools
            "read_file"
            | "outline_file"
            | "find_definition"
            | "find_references"
            | "file_search"
            | "grep_files"
            | "list_directory"
            | "web_search"
            | "get_history_notes"
            | "list_history_notes"
            | "search_history_notes"
            | "list_checkpoints"
            | "list_active_locks"
            | "discover_capabilities"
            | "discover_audio_devices"
            | "diagnose_compiler_errors"
            | "list_connectors"
            | "list_discoverable_tools"
            | "notes_read"
            | "notes_list"
            | "notes_search"
            | "get_recap" => Self::ReadOnlyConcurrent,

            // Mutating tools
            "write_file"
            | "apply_patch"
            | "delete_file"
            | "notes_write"
            | "notes_append"
            | "create_checkpoint"
            | "restore_checkpoint"
            | "acquire_path_lock"
            | "release_path_lock"
            | "save_history_notes"
            | "git_commit"
            | "git_checkout"
            | "revert_thread"
            | "multi_env_copy"
            | "generate_image"
            | "request_plugin_install" => Self::MutatingExclusive,

            // Interactive / execution commands
            "execute_command" | "write_stdin" | "run_interactive_terminal" | "spawn_subagent" => {
                Self::InteractiveCommand
            }

            // Default to exclusive for unknown tools to remain safe
            _ => Self::MutatingExclusive,
        }
    }

    pub fn is_concurrent_safe(&self) -> bool {
        matches!(self, Self::ReadOnlyConcurrent)
    }
}

/// A single tool call request dispatched from the model completion turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallRequest {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

impl ToolCallRequest {
    pub fn new(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            arguments,
        }
    }
}

/// The outcome of an individual tool call execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallResult {
    pub call_id: String,
    pub tool_name: String,
    pub output: Result<String, String>,
    pub duration_us: u64,
    pub category: ToolConcurrencyCategory,
    pub timed_out: bool,
}

impl ToolCallResult {
    pub fn is_success(&self) -> bool {
        self.output.is_ok()
    }

    pub fn unwrap_output(&self) -> &str {
        match &self.output {
            Ok(v) => v.as_str(),
            Err(e) => e.as_str(),
        }
    }
}

/// Configuration options for the parallel tool dispatcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelDispatcherConfig {
    /// Maximum concurrent read tool tasks executing in parallel.
    pub max_concurrency: usize,
    /// Execution timeout per tool call before cancelling.
    pub default_timeout: Duration,
    /// Whether to continue executing subsequent batch items if one fails.
    pub continue_on_error: bool,
}

impl Default for ParallelDispatcherConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 8,
            default_timeout: Duration::from_secs(30),
            continue_on_error: true,
        }
    }
}

/// Handler function signature for tool execution.
pub type BoxedToolFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
pub type ToolExecutionHandler =
    Arc<dyn Fn(String, serde_json::Value) -> BoxedToolFuture + Send + Sync>;

/// Sovereign parallel tool dispatcher.
pub struct ParallelDispatcher {
    config: ParallelDispatcherConfig,
    concurrency_semaphore: Arc<Semaphore>,
}

impl Default for ParallelDispatcher {
    fn default() -> Self {
        Self::new(ParallelDispatcherConfig::default())
    }
}

impl ParallelDispatcher {
    pub fn new(config: ParallelDispatcherConfig) -> Self {
        let max_concurrency = config.max_concurrency.max(1);
        Self {
            config,
            concurrency_semaphore: Arc::new(Semaphore::new(max_concurrency)),
        }
    }

    /// Dispatch a batch of tool call requests.
    ///
    /// Preserves exact initial ordering in the returned results.
    /// Read-only concurrent tools are dispatched in parallel via tokio JoinSet.
    /// Mutating or interactive tools are serialized to guarantee zero race conditions.
    pub async fn dispatch_batch(
        &self,
        requests: Vec<ToolCallRequest>,
        handler: ToolExecutionHandler,
    ) -> Vec<ToolCallResult> {
        let total = requests.len();
        if total == 0 {
            return Vec::new();
        }

        let mut results: Vec<Option<ToolCallResult>> = (0..total).map(|_| None).collect();

        // Separate indices by concurrency safety
        let mut concurrent_group = Vec::new();
        let mut serial_group = Vec::new();

        for (idx, req) in requests.iter().enumerate() {
            let cat = ToolConcurrencyCategory::classify(&req.tool_name);
            if cat.is_concurrent_safe() {
                concurrent_group.push((idx, req.clone(), cat));
            } else {
                serial_group.push((idx, req.clone(), cat));
            }
        }

        // 1. Execute concurrent read-only tools via JoinSet
        if !concurrent_group.is_empty() {
            let mut join_set = JoinSet::new();

            for (idx, req, cat) in concurrent_group {
                let sem = Arc::clone(&self.concurrency_semaphore);
                let h = Arc::clone(&handler);
                let timeout_dur = self.config.default_timeout;

                join_set.spawn(async move {
                    let _permit = sem.acquire().await.ok();
                    let start = Instant::now();

                    let exec_fut = (h)(req.tool_name.clone(), req.arguments);
                    let res = tokio::time::timeout(timeout_dur, exec_fut).await;
                    let duration_us = start.elapsed().as_micros() as u64;

                    match res {
                        Ok(inner_res) => (
                            idx,
                            ToolCallResult {
                                call_id: req.call_id,
                                tool_name: req.tool_name,
                                output: inner_res,
                                duration_us,
                                category: cat,
                                timed_out: false,
                            },
                        ),
                        Err(_) => (
                            idx,
                            ToolCallResult {
                                call_id: req.call_id,
                                tool_name: req.tool_name,
                                output: Err(format!(
                                    "Tool execution timed out after {}ms",
                                    timeout_dur.as_millis()
                                )),
                                duration_us,
                                category: cat,
                                timed_out: true,
                            },
                        ),
                    }
                });
            }

            while let Some(join_res) = join_set.join_next().await {
                if let Ok((idx, result)) = join_res {
                    results[idx] = Some(result);
                }
            }
        }

        // 2. Execute mutating and interactive tools sequentially
        for (idx, req, cat) in serial_group {
            let start = Instant::now();
            let exec_fut = (handler)(req.tool_name.clone(), req.arguments);
            let timeout_dur = self.config.default_timeout;
            let res = tokio::time::timeout(timeout_dur, exec_fut).await;
            let duration_us = start.elapsed().as_micros() as u64;

            let call_res = match res {
                Ok(inner_res) => ToolCallResult {
                    call_id: req.call_id,
                    tool_name: req.tool_name,
                    output: inner_res,
                    duration_us,
                    category: cat,
                    timed_out: false,
                },
                Err(_) => ToolCallResult {
                    call_id: req.call_id,
                    tool_name: req.tool_name,
                    output: Err(format!(
                        "Tool execution timed out after {}ms",
                        timeout_dur.as_millis()
                    )),
                    duration_us,
                    category: cat,
                    timed_out: true,
                },
            };

            results[idx] = Some(call_res);

            if !self.config.continue_on_error
                && results[idx].as_ref().is_some_and(|r| !r.is_success())
            {
                break;
            }
        }

        // Unwrap results in exact original order, providing fallback for any unexecuted slots
        results
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                r.unwrap_or_else(|| ToolCallResult {
                    call_id: requests[i].call_id.clone(),
                    tool_name: requests[i].tool_name.clone(),
                    output: Err("Execution skipped due to earlier failure in batch.".to_string()),
                    duration_us: 0,
                    category: ToolConcurrencyCategory::classify(&requests[i].tool_name),
                    timed_out: false,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_batch() {
        let dispatcher = ParallelDispatcher::default();
        let handler: ToolExecutionHandler =
            Arc::new(|_name, _args| Box::pin(async move { Ok("ok".to_string()) }));

        let results = dispatcher.dispatch_batch(vec![], handler).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_parallel_read_tools_execution_and_order() {
        let dispatcher = ParallelDispatcher::new(ParallelDispatcherConfig {
            max_concurrency: 4,
            default_timeout: Duration::from_secs(5),
            continue_on_error: true,
        });

        let handler: ToolExecutionHandler = Arc::new(|name, args| {
            Box::pin(async move {
                let id = args.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                if id == 2 {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Ok(format!("{name}_result_{id}"))
            })
        });

        let requests = vec![
            ToolCallRequest::new("c1", "read_file", serde_json::json!({ "id": 1 })),
            ToolCallRequest::new("c2", "outline_file", serde_json::json!({ "id": 2 })),
            ToolCallRequest::new("c3", "grep_files", serde_json::json!({ "id": 3 })),
        ];

        let results = dispatcher.dispatch_batch(requests, handler).await;

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].call_id, "c1");
        assert_eq!(results[0].unwrap_output(), "read_file_result_1");
        assert_eq!(results[1].call_id, "c2");
        assert_eq!(results[1].unwrap_output(), "outline_file_result_2");
        assert_eq!(results[2].call_id, "c3");
        assert_eq!(results[2].unwrap_output(), "grep_files_result_3");
    }

    #[tokio::test]
    async fn test_mutating_tools_serialized() {
        let dispatcher = ParallelDispatcher::default();
        let tracker = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let trk = Arc::clone(&tracker);
        let handler: ToolExecutionHandler = Arc::new(move |name, args| {
            let t = Arc::clone(&trk);
            Box::pin(async move {
                let mut guard = t.lock().await;
                guard.push(name.clone());
                Ok(format!("{name}: {}", args))
            })
        });

        let requests = vec![
            ToolCallRequest::new("c1", "read_file", serde_json::json!({ "file": "a.rs" })),
            ToolCallRequest::new("c2", "write_file", serde_json::json!({ "file": "b.rs" })),
            ToolCallRequest::new("c3", "apply_patch", serde_json::json!({ "patch": "diff" })),
        ];

        let results = dispatcher.dispatch_batch(requests, handler).await;

        assert_eq!(results.len(), 3);
        assert!(results[0].is_success());
        assert!(results[1].is_success());
        assert!(results[2].is_success());
        assert_eq!(
            results[0].category,
            ToolConcurrencyCategory::ReadOnlyConcurrent
        );
        assert_eq!(
            results[1].category,
            ToolConcurrencyCategory::MutatingExclusive
        );
        assert_eq!(
            results[2].category,
            ToolConcurrencyCategory::MutatingExclusive
        );
    }

    #[tokio::test]
    async fn test_tool_timeout_handling() {
        let dispatcher = ParallelDispatcher::new(ParallelDispatcherConfig {
            max_concurrency: 4,
            default_timeout: Duration::from_millis(50),
            continue_on_error: true,
        });

        let handler: ToolExecutionHandler = Arc::new(|name, _args| {
            Box::pin(async move {
                if name == "web_search" {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                Ok("done".to_string())
            })
        });

        let requests = vec![
            ToolCallRequest::new("c1", "read_file", serde_json::json!({})),
            ToolCallRequest::new("c2", "web_search", serde_json::json!({})),
        ];

        let results = dispatcher.dispatch_batch(requests, handler).await;

        assert_eq!(results.len(), 2);
        assert!(results[0].is_success());
        assert!(!results[1].is_success());
        assert!(results[1].timed_out);
        assert!(results[1].unwrap_output().contains("timed out"));
    }
}
