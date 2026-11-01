//! Sovereign unified tool execution harness and orchestration router.
//!
//! Unified tool execution harness validating schemas, enforcing permissions, and recording metrics.
//! and `tool_executor.rs`.
//! Coordinates the end-to-end tool execution lifecycle:
//! - Pre-execution validation & concurrency classification.
//! - Parallel execution via `ParallelDispatcher` with semaphore bounds.
//! - Output budget truncation & error formatting.
//! - High-resolution forensic telemetry recording in `ToolDispatchTracer`.

use crate::catalog_selector::{CatalogSelector, CatalogSelectorConfig};
use crate::dispatch_trace::{
    ToolDispatchRecord, ToolDispatchStatus, ToolDispatchTracer, ToolTurnSummary,
};
use crate::parallel_dispatcher::{
    ParallelDispatcher, ParallelDispatcherConfig, ToolCallRequest, ToolCallResult,
    ToolExecutionHandler,
};
use std::sync::Arc;
use std::time::Instant;

/// Configuration options for the tool execution harness.
#[derive(Debug, Clone)]
pub struct ToolHarnessConfig {
    pub max_output_bytes: usize,
    pub parallel_config: ParallelDispatcherConfig,
    pub catalog_config: CatalogSelectorConfig,
}

impl Default for ToolHarnessConfig {
    fn default() -> Self {
        Self {
            max_output_bytes: 50 * 1024, // 50 KiB output budget
            parallel_config: ParallelDispatcherConfig::default(),
            catalog_config: CatalogSelectorConfig::default(),
        }
    }
}

/// Sovereign unified tool execution harness.
pub struct ToolExecutionHarness {
    config: ToolHarnessConfig,
    dispatcher: ParallelDispatcher,
    tracer: Arc<ToolDispatchTracer>,
    catalog: CatalogSelector,
}

impl Default for ToolExecutionHarness {
    fn default() -> Self {
        Self::new(ToolHarnessConfig::default())
    }
}

impl ToolExecutionHarness {
    pub fn new(config: ToolHarnessConfig) -> Self {
        let dispatcher = ParallelDispatcher::new(config.parallel_config.clone());
        let tracer = Arc::new(ToolDispatchTracer::default());
        let catalog = CatalogSelector::new(config.catalog_config.clone());

        Self {
            config,
            dispatcher,
            tracer,
            catalog,
        }
    }

    pub fn tracer(&self) -> &ToolDispatchTracer {
        &self.tracer
    }

    pub fn catalog(&self) -> &CatalogSelector {
        &self.catalog
    }

    pub fn catalog_mut(&mut self) -> &mut CatalogSelector {
        &mut self.catalog
    }

    /// Execute a single tool call through the harness with telemetry recording.
    pub async fn execute_single(
        &self,
        session_id: &str,
        turn_index: usize,
        request: ToolCallRequest,
        handler: ToolExecutionHandler,
    ) -> ToolCallResult {
        let results = self
            .execute_batch(session_id, turn_index, vec![request], handler)
            .await;
        results.into_iter().next().unwrap()
    }

    /// Execute a batch of tool calls through the parallel dispatcher with telemetry recording.
    pub async fn execute_batch(
        &self,
        session_id: &str,
        turn_index: usize,
        requests: Vec<ToolCallRequest>,
        handler: ToolExecutionHandler,
    ) -> Vec<ToolCallResult> {
        let is_parallel_batch = requests.len() > 1;
        let start = Instant::now();

        // 1. Dispatch batch through ParallelDispatcher
        let mut results = self
            .dispatcher
            .dispatch_batch(requests.clone(), handler)
            .await;

        // 2. Post-process outputs (truncation) & record telemetry
        for (idx, res) in results.iter_mut().enumerate() {
            let req = &requests[idx];
            let args_bytes = serde_json::to_string(&req.arguments)
                .map(|s| s.len())
                .unwrap_or(0);

            // Apply output truncation if output exceeds maximum budget
            if let Ok(output_str) = &mut res.output
                && output_str.len() > self.config.max_output_bytes
            {
                let truncated = format!(
                    "{}\n\n[... Output truncated: {} bytes omitted to protect context budget ...]",
                    &output_str[..self.config.max_output_bytes],
                    output_str.len() - self.config.max_output_bytes
                );
                *output_str = truncated;
            }

            let output_bytes = match &res.output {
                Ok(s) => s.len(),
                Err(e) => e.len(),
            };

            let status = if res.timed_out {
                ToolDispatchStatus::TimedOut
            } else if res.is_success() {
                ToolDispatchStatus::Success
            } else {
                ToolDispatchStatus::Error
            };

            let err_msg = match &res.output {
                Err(e) => Some(e.clone()),
                Ok(_) => None,
            };

            let trace_record = ToolDispatchRecord::new(
                session_id,
                turn_index,
                &res.call_id,
                &res.tool_name,
                args_bytes,
                output_bytes,
                status,
                res.duration_us,
                is_parallel_batch,
                err_msg,
            );

            self.tracer.record(trace_record);
        }

        let _total_elapsed = start.elapsed();
        results
    }

    /// Retrieve aggregate metrics for a conversation turn.
    pub fn get_turn_summary(&self, session_id: &str, turn_index: usize) -> ToolTurnSummary {
        self.tracer.turn_summary(session_id, turn_index)
    }

    /// Export telemetry records as JSONL.
    pub fn export_telemetry_jsonl(&self) -> Result<String, serde_json::Error> {
        self.tracer.export_jsonl()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_harness_single_execution_and_telemetry() {
        let harness = ToolExecutionHarness::default();
        let handler: ToolExecutionHandler = Arc::new(|name, _args| {
            Box::pin(async move { Ok(format!("Executed {name} successfully")) })
        });

        let req = ToolCallRequest::new(
            "call_abc",
            "read_file",
            serde_json::json!({ "path": "main.rs" }),
        );

        let res = harness.execute_single("sess_test", 1, req, handler).await;
        assert!(res.is_success());
        assert_eq!(res.unwrap_output(), "Executed read_file successfully");

        let summary = harness.get_turn_summary("sess_test", 1);
        assert_eq!(summary.total_calls, 1);
        assert_eq!(summary.successful_calls, 1);
        assert!(summary.total_duration_us > 0);
    }

    #[tokio::test]
    async fn test_harness_output_truncation() {
        let config = ToolHarnessConfig {
            max_output_bytes: 100,
            ..Default::default()
        };
        let harness = ToolExecutionHarness::new(config);

        let large_output = "A".repeat(500);
        let handler: ToolExecutionHandler = Arc::new(move |_name, _args| {
            let out = large_output.clone();
            Box::pin(async move { Ok(out) })
        });

        let req = ToolCallRequest::new("call_big", "read_file", serde_json::json!({}));
        let res = harness.execute_single("sess_trunc", 1, req, handler).await;

        assert!(res.is_success());
        let text = res.unwrap_output();
        assert!(
            text.contains(
                "[... Output truncated: 400 bytes omitted to protect context budget ...]"
            )
        );
        assert!(text.starts_with(&"A".repeat(100)));
    }

    #[tokio::test]
    async fn test_harness_batch_execution_telemetry() {
        let harness = ToolExecutionHarness::default();
        let handler: ToolExecutionHandler = Arc::new(|name, args| {
            Box::pin(async move {
                if name == "failing_tool" {
                    Err("Intentional error".to_string())
                } else {
                    Ok(format!("{name}: {}", args))
                }
            })
        });

        let requests = vec![
            ToolCallRequest::new("c1", "read_file", serde_json::json!({ "k": 1 })),
            ToolCallRequest::new("c2", "failing_tool", serde_json::json!({ "k": 2 })),
        ];

        let results = harness
            .execute_batch("sess_batch", 2, requests, handler)
            .await;
        assert_eq!(results.len(), 2);
        assert!(results[0].is_success());
        assert!(!results[1].is_success());

        let summary = harness.get_turn_summary("sess_batch", 2);
        assert_eq!(summary.total_calls, 2);
        assert_eq!(summary.successful_calls, 1);
        assert_eq!(summary.failed_calls, 1);
    }
}
