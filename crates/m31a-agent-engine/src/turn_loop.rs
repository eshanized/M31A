//! Production reactive agent turn loop with in-flight dynamic steering, parallel dispatch, and auto-compaction.

use anyhow::Result;
use m31a_core::context_compactor::{CompactedHistory, digest_tool_output};
use m31a_core::prompts::CollaborationMode;
use m31a_core::types::{ChatMessage, Role, TokenUsage, ToolCall};
use m31a_core::{
    ApprovalPolicy, CompactTokenBudget, ContextCompactor, SandboxPolicy, approx_token_count,
};
use m31a_execpolicy::{Decision, ExecPolicy};
use m31a_guardian::GuardianReviewer;
use m31a_hooks::{HookDisposition, HookEvent};
use m31a_provider::{HybridModelRouter, ProviderClient, StreamCallback, classify_task_tier};
use m31a_storage::db::Storage;
use m31a_tools::registry::ToolRegistry;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct PendingApproval {
    pub tool_call: ToolCall,
    pub reason: String,
    pub command: Option<String>,
}

#[derive(Debug)]
pub enum EngineEvent {
    ContentChunk(String),
    ReasoningChunk(String),
    ToolCallStarted {
        id: String,
        name: String,
        args: String,
    },
    ToolCallFinished {
        id: String,
        output: String,
        success: bool,
    },
    /// Guardian blocked a tool call (rationale shown to the user).
    GuardianDenied {
        rationale: String,
    },
    Notice(String, bool), // (message, is_warning)
    ApprovalRequested {
        approval: PendingApproval,
        response_tx: oneshot::Sender<m31a_elicitation::ApprovalDecision>,
    },
    QuestionRequested {
        question: m31a_elicitation::QuestionItem,
        response_tx: oneshot::Sender<Vec<String>>,
    },
    Compacted {
        /// UUIDv7 id of the freshly started context window (report §5.1).
        window_id: String,
        original_tokens: usize,
        compacted_tokens: usize,
    },
    TurnComplete {
        usage: TokenUsage,
        messages: Vec<ChatMessage>,
    },
    TurnFailed(String),
}

/// Tools safe to dispatch concurrently. Names must match the registry
/// (`m31a_tools::registry`) exactly — stale names silently degrade batching.
fn is_parallel_safe_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "file_search"
            | "grep_search"
            | "list_dir"
            | "outline_file"
            | "find_definition"
            | "find_references"
            | "extract_symbol_body"
            | "web_search"
            | "list_background_processes"
            | "get_background_process_output"
            | "find_related_files"
            | "list_locks"
    )
}

#[cfg(test)]
fn is_plan_mode_blocked_tool(name: &str) -> bool {
    m31a_execpolicy::evaluate_plan_mode(name, None).is_err()
}

#[cfg(test)]
fn is_mutating_shell_command(cmd_args: &str) -> bool {
    m31a_execpolicy::evaluate_plan_mode("exec_command", Some(cmd_args)).is_err()
}

/// Guardian reviews every sequential dispatch only when the user opted into
/// automated supervision (`ApproveForMe`) and a reviewer is configured.
fn guardian_required_for_call(
    approval_policy: &ApprovalPolicy,
    guardian: &Option<Arc<GuardianReviewer>>,
) -> bool {
    *approval_policy == ApprovalPolicy::ApproveForMe && guardian.is_some()
}

/// Self-healing guidance injection stays within the per-turn budget.
fn heal_iterations_remaining(used: usize, max: usize) -> bool {
    used < max
}

/// Extracts the most recent user message text — used as Guardian task context
/// and as router classification input.
fn last_user_text(history: &[ChatMessage]) -> String {
    history
        .iter()
        .rev()
        .find(|m| m.role == m31a_core::types::Role::User)
        .and_then(|m| m.content.clone())
        .unwrap_or_default()
}

/// Default per-tool-result truncation budget in tokens (report §4.2).
const DEFAULT_TOOL_OUTPUT_TOKEN_LIMIT: usize = 16_384;

/// Wrap-up reminder fires at this fraction of the compaction trigger threshold,
/// giving the model a grace band to converge before the context rollover (§5.1).
const WRAP_UP_REMINDER_THRESHOLD: f64 = 0.85;

/// Rough token estimate of an entire history (content + reasoning only),
/// matching the compactor's counting heuristic (report §5.1).
pub fn estimate_history_tokens(history: &[ChatMessage]) -> usize {
    history
        .iter()
        .map(|m| {
            let mut n = 0;
            if let Some(ref text) = m.content {
                n += approx_token_count(text);
            }
            if let Some(ref reasoning) = m.reasoning_content {
                n += approx_token_count(reasoning);
            }
            n
        })
        .sum()
}

/// A compaction computed in the background (report §5.2), ready for
/// consumption at the foreground trigger.
struct PreparedCompaction {
    /// History length the transform ran on. History is append-only within a
    /// run, so the snapshot is a true prefix of any later history.
    snapshot_len: usize,
    result: CompactedHistory,
}

/// §4.2 layered truncation: token-budgeted head+tail truncation with an
/// explicit warning prefix, applied to every tool result before it enters
/// model context, engine events, and persistence. `limit == 0` disables it.
fn truncate_tool_result_for_context(limit: usize, raw: &str) -> String {
    if limit == 0 {
        return raw.to_string();
    }
    let (truncated, was_truncated) =
        m31a_core::truncate_text_with_policy(raw, m31a_core::TruncationPolicy::Tokens(limit));
    if !was_truncated {
        return raw.to_string();
    }
    format!(
        "Warning: output truncated (original tokens: ~{}, total lines: {})\n\n{}",
        approx_token_count(raw),
        raw.lines().count(),
        truncated
    )
}

/// Final tool surface presented to the model: registry definitions plus the
/// code-mode `exec`/`wait` pair when code-mode is enabled (report §2.4).
fn compose_tool_surface(
    base: &[m31a_core::types::ToolDefinition],
    code_mode: Option<&m31a_code_mode::CodeModeOrchestrator>,
) -> Vec<m31a_core::types::ToolDefinition> {
    let mut defs = base.to_vec();
    if let Some(cm) = code_mode {
        defs.extend(cm.tool_definitions(base));
    }
    defs
}

#[derive(Clone)]
pub struct AgentEngine {
    pub client: ProviderClient,
    pub tools: ToolRegistry,
    pub storage: Storage,
    pub approval_policy: ApprovalPolicy,
    pub exec_policy: std::sync::Arc<std::sync::RwLock<ExecPolicy>>,
    pub sandbox_policy: SandboxPolicy,
    pub cwd: PathBuf,
    pub compactor: Arc<ContextCompactor>,
    /// Optional automated safety supervisor. Reviewed before dispatch when
    /// `approval_policy == ApproveForMe` (report §2.2).
    pub guardian: Option<Arc<GuardianReviewer>>,
    /// Persist turn/tool/assistant records to storage (CLI `--ephemeral` sets this false).
    pub persist_items: bool,
    /// Inject self-healing repair guidance when tool output contains compiler/test diagnostics.
    pub self_heal: bool,
    /// Maximum self-healing guidance injections per `run_turn` call.
    pub max_heal_iterations: usize,
    /// Opt-in dynamic model routing per task tier (report §2.5). Disabled by default.
    pub routing_enabled: bool,
    /// Per-tool-result truncation budget in tokens (report §4.2). 0 disables truncation.
    pub tool_output_token_limit: usize,
    /// Code-mode runtime (report §2.4). `Some` appends the `exec`/`wait` tool
    /// pair to the model surface and routes those calls through the JS/DSL
    /// orchestrator instead of the registry.
    pub code_mode: Option<m31a_code_mode::CodeModeOrchestrator>,
    /// Hook dispatcher (report §4.4). Lifecycle events dispatch to configured
    /// command handlers; an empty config fast-paths to Allow.
    pub hooks: Option<Arc<m31a_hooks::HookRuntime>>,
    /// Paths the user explicitly allowed for the remainder of this session.
    pub session_allowed_files: Arc<std::sync::RwLock<std::collections::HashSet<PathBuf>>>,
    /// Active collaboration mode (Plan vs Default). In Plan mode, file mutations are blocked.
    pub collaboration_mode: CollaborationMode,
    /// Pre-turn git checkpoint snapshot for zero-loss rollback/revert.
    pub auto_checkpoint: bool,
    /// Optional telemetry pipeline for session and turn observability
    pub telemetry: Option<Arc<m31a_telemetry::TelemetryPipeline>>,
    /// Authoritative runtime-owned system instructions (P0-A invariant).
    /// Maintained separately from compactable dialogue to guarantee preservation across compaction.
    pub authoritative_system_prompt: Option<String>,
}

impl AgentEngine {
    pub fn new(
        client: impl Into<ProviderClient>,
        tools: ToolRegistry,
        storage: Storage,
        approval_policy: ApprovalPolicy,
        exec_policy: ExecPolicy,
        sandbox_policy: SandboxPolicy,
        cwd: PathBuf,
    ) -> Self {
        let client = client.into();
        let budget = CompactTokenBudget::default();
        let compactor = Arc::new(ContextCompactor::new(budget));

        Self {
            client,
            tools,
            storage,
            approval_policy,
            exec_policy: std::sync::Arc::new(std::sync::RwLock::new(exec_policy)),
            sandbox_policy,
            cwd,
            compactor,
            guardian: None,
            persist_items: true,
            self_heal: false,
            max_heal_iterations: 3,
            routing_enabled: false,
            tool_output_token_limit: DEFAULT_TOOL_OUTPUT_TOKEN_LIMIT,
            code_mode: None,
            hooks: None,
            session_allowed_files: Arc::new(std::sync::RwLock::new(
                std::collections::HashSet::new(),
            )),
            collaboration_mode: CollaborationMode::Default,
            auto_checkpoint: true,
            telemetry: None,
            authoritative_system_prompt: None,
        }
    }

    /// Sets the authoritative system prompt on this engine.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.authoritative_system_prompt = Some(prompt.into());
        self
    }

    pub fn with_telemetry(mut self, telemetry: Arc<m31a_telemetry::TelemetryPipeline>) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    /// Shared tool-result ingestion (§4.2): truncates for events/persistence/
    /// model context, then parses the RAW output for self-heal diagnostics.
    #[allow(clippy::too_many_arguments)]
    fn ingest_tool_result(
        &self,
        history: &mut Vec<ChatMessage>,
        session_id: &str,
        turn_id: &str,
        call_id: &str,
        tool_name: &str,
        fn_args: &str,
        mut result: Result<String>,
        event_tx: &mpsc::UnboundedSender<EngineEvent>,
        heal_used: &mut usize,
        timeline: &mut m31a_verify::VerificationTimeline,
        turn: usize,
        oscillation_detector: &mut crate::oscillation::OscillationDetector,
    ) {
        // Adaptive problem-solving (§4.1): evaluate oscillation / stuck loops
        let call = ToolCall {
            id: call_id.to_string(),
            call_type: "function".to_string(),
            function: m31a_core::types::FunctionCall {
                name: tool_name.to_string(),
                arguments: fn_args.to_string(),
            },
        };
        let is_err = result.is_err();
        let snippet = match &result {
            Ok(s) => s.as_str(),
            Err(_) => "",
        };
        if let Some(alert) = oscillation_detector.record_and_evaluate(&call, is_err, Some(snippet))
        {
            let _ = event_tx.send(EngineEvent::Notice(alert.description.clone(), true));
            history.push(ChatMessage::user(alert.adaptive_steering_prompt));
        }

        match tool_name {
            "write_file" | "edit" | "apply_patch" => {
                timeline.record_edit(turn);
            }
            "verify_workspace" => {
                let parsed_args = serde_json::from_str::<serde_json::Value>(fn_args).ok();
                let explicit_criterion_id = parsed_args
                    .as_ref()
                    .and_then(|v| {
                        v.get("criterion_id")
                            .or_else(|| v.get("requirement_id"))
                            .and_then(|s| s.as_str())
                            .map(|s| s.to_string())
                    })
                    .or_else(|| {
                        timeline.plan.as_ref().and_then(|p| {
                            if p.criteria.len() == 1 {
                                p.criteria.first().map(|c| c.id.clone())
                            } else {
                                None
                            }
                        })
                    });
                let mode = parsed_args
                    .as_ref()
                    .and_then(|v| v.get("mode").and_then(|s| s.as_str()))
                    .unwrap_or("check");

                let (success, status, out_str) = match &result {
                    Ok(out) => {
                        if out.starts_with("Verification PASSED") {
                            timeline.record_check_result(turn, true, Some(call_id));
                            (true, m31a_verify::VerificationStatus::Success, out.clone())
                        } else {
                            timeline.record_check_result(turn, false, None);
                            timeline.record_failure(turn);
                            (
                                false,
                                m31a_verify::VerificationStatus::Failure {
                                    exit_code: Some(1),
                                    diagnostics: out.clone(),
                                },
                                out.clone(),
                            )
                        }
                    }
                    Err(e) => {
                        timeline.record_failure(turn);
                        (
                            false,
                            m31a_verify::VerificationStatus::InfrastructureFailure {
                                error: e.to_string(),
                            },
                            e.to_string(),
                        )
                    }
                };

                if let Some(criterion_id) = explicit_criterion_id {
                    let record = m31a_verify::EvidenceRecord {
                        requirement_id: criterion_id,
                        evidence_id: call_id.to_string(),
                        evidence_type: if mode == "test" {
                            m31a_verify::EvidenceType::UnitTest
                        } else {
                            m31a_verify::EvidenceType::CompilerCheck
                        },
                        executed_operation: format!("verify_workspace (mode: {})", mode),
                        execution_timestamp: chrono::Utc::now().timestamp(),
                        actual_result: out_str,
                        success,
                        relevant_artifacts: Vec::new(),
                        tool_execution_reference: Some(call_id.to_string()),
                        test_and_diagnostic_references: Vec::new(),
                        environment_info: std::env::consts::OS.to_string(),
                        verification_limitations: None,
                        execution_turn: Some(turn),
                        exit_code: if success { Some(0) } else { Some(1) },
                        tests_passed: None,
                        tests_failed: None,
                        tests_total: None,
                        duration_ms: None,
                        status: Some(status),
                    };
                    timeline.record_criterion_evidence(record);
                }
            }
            "exec_command" => {
                let parsed_args = serde_json::from_str::<serde_json::Value>(fn_args).ok();
                let cmd_str = parsed_args
                    .as_ref()
                    .and_then(|v| {
                        v.get("command")
                            .or_else(|| v.get("cmd"))
                            .and_then(|s| s.as_str())
                    })
                    .unwrap_or("");
                let explicit_criterion_id = parsed_args
                    .as_ref()
                    .and_then(|v| {
                        v.get("criterion_id")
                            .or_else(|| v.get("requirement_id"))
                            .and_then(|s| s.as_str())
                            .map(|s| s.to_string())
                    })
                    .or_else(|| {
                        timeline.plan.as_ref().and_then(|p| {
                            if p.criteria.len() == 1 {
                                p.criteria.first().map(|c| c.id.clone())
                            } else {
                                None
                            }
                        })
                    });

                let parsed_exit_code = match &result {
                    Ok(out) => {
                        if let Some(stripped) = out.strip_prefix("Exit code: ") {
                            stripped
                                .split_whitespace()
                                .next()
                                .and_then(|c| c.parse::<i32>().ok())
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                };

                let (success, status, out_str) = match &result {
                    Ok(out) => {
                        let lower = out.to_lowercase();
                        let has_zero_tests = lower.contains("0 passed; 0 failed")
                            || lower.contains("running 0 tests")
                            || lower.contains("no tests were found")
                            || lower.contains("collected 0 items");

                        let timed_out = out.contains("[COMMAND TIMED OUT]");

                        if timed_out {
                            timeline.record_failure(turn);
                            (
                                false,
                                m31a_verify::VerificationStatus::Failure {
                                    exit_code: Some(124),
                                    diagnostics: out.clone(),
                                },
                                out.clone(),
                            )
                        } else if has_zero_tests {
                            timeline.record_failure(turn);
                            (
                                false,
                                m31a_verify::VerificationStatus::InsufficientEvidence {
                                    reason: "0 tests executed during test run".to_string(),
                                },
                                out.clone(),
                            )
                        } else if parsed_exit_code == Some(0) {
                            timeline.record_test_result(turn, true, Some(call_id));
                            (true, m31a_verify::VerificationStatus::Success, out.clone())
                        } else if let Some(code) = parsed_exit_code {
                            timeline.record_failure(turn);
                            (
                                false,
                                m31a_verify::VerificationStatus::Failure {
                                    exit_code: Some(code),
                                    diagnostics: out.clone(),
                                },
                                out.clone(),
                            )
                        } else {
                            // Non-command tool or uninstrumented output: rely on truthful Ok outcome
                            (true, m31a_verify::VerificationStatus::Success, out.clone())
                        }
                    }
                    Err(e) => {
                        timeline.record_failure(turn);
                        (
                            false,
                            m31a_verify::VerificationStatus::InfrastructureFailure {
                                error: e.to_string(),
                            },
                            e.to_string(),
                        )
                    }
                };

                if let Some(criterion_id) = explicit_criterion_id {
                    let record = m31a_verify::EvidenceRecord {
                        requirement_id: criterion_id,
                        evidence_id: call_id.to_string(),
                        evidence_type: m31a_verify::EvidenceType::UnitTest,
                        executed_operation: format!("exec_command: {}", cmd_str),
                        execution_timestamp: chrono::Utc::now().timestamp(),
                        actual_result: out_str,
                        success,
                        relevant_artifacts: Vec::new(),
                        tool_execution_reference: Some(call_id.to_string()),
                        test_and_diagnostic_references: Vec::new(),
                        environment_info: std::env::consts::OS.to_string(),
                        verification_limitations: None,
                        execution_turn: Some(turn),
                        exit_code: parsed_exit_code.or(if success { Some(0) } else { Some(1) }),
                        tests_passed: None,
                        tests_failed: None,
                        tests_total: None,
                        duration_ms: None,
                        status: Some(status),
                    };
                    timeline.record_criterion_evidence(record);
                }
            }
            "agent_plan" => {
                if let Ok(plan) = serde_json::from_str::<m31a_verify::AgentPlan>(fn_args) {
                    timeline.record_plan(plan);
                }
            }
            "agent_complete" => {
                if let Ok(report) =
                    serde_json::from_str::<m31a_verify::AgentCompletionReport>(fn_args)
                    && let Err(issues) = timeline.evaluate_completion(&report)
                {
                    result = Err(anyhow::anyhow!(
                        "Completion rejected by verification state machine:\n- {}",
                        issues.join("\n- ")
                    ));
                }
            }
            _ => {}
        }

        match result {
            Ok(out) => {
                let ingested = truncate_tool_result_for_context(self.tool_output_token_limit, &out);
                let _ = event_tx.send(EngineEvent::ToolCallFinished {
                    id: call_id.to_string(),
                    output: ingested.clone(),
                    success: true,
                });
                if self.persist_items {
                    let _ = self.storage.save_item(
                        &uuid::Uuid::new_v4().to_string(),
                        session_id,
                        Some(turn_id),
                        "tool",
                        &ingested,
                    );
                }
                history.push(ChatMessage::tool_response(call_id.to_string(), ingested));
                self.consider_self_heal(history, tool_name, &out, heal_used, event_tx);
            }
            Err(e) => {
                let err_msg = format!("Tool error: {}", e);
                let _ = event_tx.send(EngineEvent::ToolCallFinished {
                    id: call_id.to_string(),
                    output: err_msg.clone(),
                    success: false,
                });
                if self.persist_items {
                    let _ = self.storage.save_item(
                        &uuid::Uuid::new_v4().to_string(),
                        session_id,
                        Some(turn_id),
                        "tool_error",
                        &err_msg,
                    );
                }
                history.push(ChatMessage::tool_response(call_id.to_string(), err_msg));
            }
        }
    }

    /// Self-healing hook (report §2.1/§2.6): when enabled and within budget, parses
    /// compiler/test diagnostics from tool output and injects structured repair
    /// guidance as a user message so the next model iteration can repair.
    fn consider_self_heal(
        &self,
        history: &mut Vec<ChatMessage>,
        tool_name: &str,
        output: &str,
        heal_used: &mut usize,
        event_tx: &mpsc::UnboundedSender<EngineEvent>,
    ) -> bool {
        if !self.self_heal || !heal_iterations_remaining(*heal_used, self.max_heal_iterations) {
            return false;
        }

        let diags = m31a_verify::DiagnosticParser::parse(output);
        if diags.is_empty() {
            return false;
        }

        let raw = m31a_verify::RawRunResult {
            project_kind: m31a_verify::ProjectRunner::detect(&self.cwd),
            command: tool_name.to_string(),
            success: false,
            stdout: output.to_string(),
            stderr: String::new(),
            exit_code: 1,
            timed_out: false,
        };
        let report = m31a_verify::SelfHealingGenerator::evaluate(raw, diags);
        if let Some(guidance) = report.self_healing_prompt {
            *heal_used += 1;
            let _ = event_tx.send(EngineEvent::Notice(
                format!(
                    "Self-Healing: detected compiler/test diagnostics; repair guidance injected ({} of max {}).",
                    *heal_used, self.max_heal_iterations
                ),
                false,
            ));
            history.push(ChatMessage::user(guidance));
            return true;
        }
        false
    }

    /// Consumes a §5.2 background preparation: merges the prepared transform
    /// with the live history under the append-only invariant (the prepare
    /// snapshot is a true prefix).
    ///
    /// Coverage rules — no message is ever lost or duplicated:
    /// - prepared has a summary (pass 2 ran): the summary covers
    ///   `S[..n-4]`, so the live tail `H[n-4..]` is kept;
    /// - digest-only preparation: prepared messages cover ALL of `S`, so
    ///   only the appended region `H[n..]` is added.
    ///
    /// Oversized tool outputs in the live region are then digested
    /// (already-digested content re-digests as a no-op).
    fn merge_prepared_compaction(
        &self,
        history: &[ChatMessage],
        p: PreparedCompaction,
    ) -> CompactedHistory {
        let n = p.snapshot_len.min(history.len());
        let original_tokens = estimate_history_tokens(history);
        let digest_budget = self.compactor.budget().max_single_tool_output_tokens;

        let mut merged = if p.result.turns_summarized > 0 {
            // P0-A: Preserve authoritative system instructions across merge
            let auth_system: Vec<ChatMessage> = history
                .iter()
                .filter(|m| m31a_core::context_compactor::is_authoritative_system_message(m))
                .cloned()
                .collect();
            let auth_system = if auth_system.is_empty() {
                p.result
                    .compacted_messages
                    .iter()
                    .filter(|m| m31a_core::context_compactor::is_authoritative_system_message(m))
                    .cloned()
                    .collect()
            } else {
                auth_system
            };

            let mut v = Vec::with_capacity(history.len() + auth_system.len() + 1);
            v.extend(auth_system);

            if let Some(summary) = p
                .result
                .compacted_messages
                .iter()
                .find(|m| m31a_core::context_compactor::is_compaction_summary(m))
            {
                v.push(summary.clone());
            }

            let dialogue_start = history
                .iter()
                .take_while(|m| m31a_core::context_compactor::is_authoritative_system_message(m))
                .count();

            let tail_start = (n.saturating_sub(4)).max(dialogue_start);
            v.extend(history[tail_start..].iter().cloned());
            v
        } else {
            let mut v = p.result.compacted_messages.clone();
            v.extend(history[n..].iter().cloned());
            v
        };

        let mut tools_digested = p.result.tools_digested;
        for m in &mut merged {
            if m.role == Role::Tool
                && let Some(ref text) = m.content
            {
                let (digested, was) = digest_tool_output(text, None, digest_budget);
                if was {
                    tools_digested += 1;
                    m.content = Some(digested);
                }
            }
        }

        CompactedHistory {
            compacted_tokens: estimate_history_tokens(&merged),
            compacted_messages: merged,
            original_tokens,
            turns_summarized: p.result.turns_summarized,
            tools_digested,
        }
    }

    /// Executes an autonomous turn loop with support for dynamic steering, parallel dispatch, and inline compaction.
    pub async fn run_turn(
        &mut self,
        session_id: &str,
        mut history: Vec<ChatMessage>,
        event_tx: mpsc::UnboundedSender<EngineEvent>,
        mut steer_rx: mpsc::UnboundedReceiver<String>,
        cancel_token: CancellationToken,
        max_turns: usize,
    ) -> Result<()> {
        self.tools.set_session_id(session_id);

        // P0-A: Synchronize authoritative system prompt between engine state and history
        if let Some(text) = history
            .first()
            .filter(|m| m31a_core::context_compactor::is_authoritative_system_message(m))
            .and_then(|m| m.content.as_deref())
        {
            self.authoritative_system_prompt = Some(text.to_string());
        }
        let has_authoritative_head = history
            .first()
            .map(m31a_core::context_compactor::is_authoritative_system_message)
            .unwrap_or(false);
        if let (false, Some(prompt)) = (
            has_authoritative_head,
            self.authoritative_system_prompt.as_ref(),
        ) {
            history.insert(0, ChatMessage::system(prompt.clone()));
        }

        let tools_def = compose_tool_surface(&self.tools.definitions(), self.code_mode.as_ref());
        let mut total_usage = TokenUsage::default();
        let router = HybridModelRouter::new();
        let mut heal_iterations_used: usize = 0;
        let mut wrap_up_reminded = false;
        let mut stop_hook_blocks: usize = 0;
        const MAX_STOP_HOOK_BLOCKS: usize = 3;
        let mut oscillation_detector = crate::oscillation::OscillationDetector::default();

        // §5.2 background compaction state: one prepared compaction slot per
        // context window. Preparation starts at the prepare ratio (65%);
        // consume happens at the foreground trigger; re-armed after rollover.
        let prepared: Arc<std::sync::Mutex<Option<PreparedCompaction>>> =
            Arc::new(std::sync::Mutex::new(None));
        let mut prepare_armed = true;

        // §4.4 session lifecycle hooks: SessionStart fires once per run, then
        // UserPromptSubmit may block the prompt outright (upstream semantics).
        if let Some(hooks) = &self.hooks {
            let _ = hooks
                .dispatch(
                    HookEvent::SessionStart,
                    "",
                    serde_json::json!({ "source": "startup" }),
                )
                .await;

            let prompt = last_user_text(&history);
            if !prompt.is_empty()
                && let HookDisposition::Block(reason) = hooks
                    .dispatch(
                        HookEvent::UserPromptSubmit,
                        "",
                        serde_json::json!({ "prompt": prompt }),
                    )
                    .await
            {
                let _ = event_tx.send(EngineEvent::Notice(
                    format!("Prompt blocked by hook: {reason}"),
                    true,
                ));
                let _ = event_tx.send(EngineEvent::TurnComplete {
                    usage: total_usage,
                    messages: history,
                });
                return Ok(());
            }
        }

        // Execution claims & crash recovery: recover uncommitted or crashed turns from prior run
        if let Ok(recovered) = self.storage.recover_crashed_claims() {
            for rec in recovered {
                if rec.thread_id == session_id {
                    let _ = event_tx.send(EngineEvent::Notice(
                        format!(
                            "Recovered uncommitted turn {} after abnormal termination",
                            rec.turn_index
                        ),
                        true,
                    ));
                }
            }
        }

        // §4.9 Cross-session consistency: reconcile working tree against prior baseline
        if let Ok(reconciliation) =
            m31a_core::WorkingTreeBaseline::reconcile_session_resume(&self.cwd, None)
            && reconciliation.has_diverged
        {
            let _ = event_tx.send(EngineEvent::Notice(
                reconciliation.reconciliation_notice.clone(),
                true,
            ));
            history.push(ChatMessage::user(reconciliation.reconciliation_notice));
        }

        let my_pid = std::process::id();
        let prompt_preview = last_user_text(&history);
        let active_claim = self
            .storage
            .create_execution_claim(
                session_id,
                1,
                my_pid,
                if prompt_preview.is_empty() {
                    None
                } else {
                    Some(&prompt_preview)
                },
            )
            .ok();

        let session_start_time = std::time::Instant::now();
        if let Some(telem) = &self.telemetry {
            telem.session_start(
                session_id.to_string(),
                self.client.model().to_string(),
                "nvidia-nim".to_string(),
                "default".to_string(),
            );
        }

        let (prompt_tx, mut prompt_rx) = mpsc::channel::<String>(32);
        let prompt_tx_clone = prompt_tx.clone();
        m31a_tools::schedule::register_prompt_delivery_callback(Arc::new(
            move |task_id, prompt| {
                let _ = prompt_tx_clone
                    .try_send(format!("[Scheduled Notification #{task_id}]: {prompt}"));
            },
        ))
        .await;

        let mut timeline = m31a_verify::VerificationTimeline::new();
        let mut stream_attempt = 0;

        for turn in 1..=max_turns {
            if cancel_token.is_cancelled() {
                let _ = event_tx.send(EngineEvent::Notice("Turn cancelled by user.".into(), true));
                break;
            }

            // Update tool registry with current compactor and history for context-aware tools
            self.tools.compactor = Some(self.compactor.clone());
            self.tools.message_history = Some(Arc::new(history.clone()));

            // 1. Check for mid-turn steering input from user or scheduled prompt delivery
            while let Ok(steer_msg) = steer_rx.try_recv() {
                let _ = event_tx.send(EngineEvent::Notice(
                    format!("Steering guidance injected: \"{}\"", steer_msg),
                    false,
                ));
                history.push(ChatMessage::user(steer_msg));
            }
            while let Ok(sched_msg) = prompt_rx.try_recv() {
                let _ = event_tx.send(EngineEvent::Notice(
                    format!("Scheduled notification: \"{}\"", sched_msg),
                    false,
                ));
                history.push(ChatMessage::user(sched_msg));
            }

            if let Some(telem) = &self.telemetry {
                let _ = telem.send(m31a_telemetry::TelemetryEvent::TurnStart {
                    session_id: session_id.to_string(),
                    turn_id: format!("{}-t{}", session_id, turn),
                    tools_available: tools_def.len(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                });
            }

            // 2. Inline Context Compaction check — gated on the foreground
            // trigger; consumes a §5.2 background preparation when one is
            // available (otherwise the prepared result is kept for a later
            // iteration), else falls back to the synchronous transform.
            let compacted = if self
                .compactor
                .budget()
                .should_compact(estimate_history_tokens(&history))
            {
                match prepared.lock().unwrap().take() {
                    Some(p) if p.snapshot_len <= history.len() => {
                        self.merge_prepared_compaction(&history, p)
                    }
                    _ => self.compactor.compact_if_needed(&history),
                }
            } else {
                self.compactor.compact_if_needed(&history)
            };
            if compacted.turns_summarized > 0 || compacted.tools_digested > 0 {
                // §4.4: PreCompact/PostCompact hooks are observability seams —
                // outcomes are ignored (blocking compaction under token
                // pressure would overflow the window).
                if let Some(hooks) = &self.hooks {
                    let _ = hooks
                        .dispatch(
                            HookEvent::PreCompact,
                            "",
                            serde_json::json!({
                                "original_tokens": compacted.original_tokens,
                                "compacted_tokens": compacted.compacted_tokens,
                            }),
                        )
                        .await;
                }
                let window_id = uuid::Uuid::now_v7().to_string();
                let _ = event_tx.send(EngineEvent::Compacted {
                    window_id: window_id.clone(),
                    original_tokens: compacted.original_tokens,
                    compacted_tokens: compacted.compacted_tokens,
                });
                // §5.5: persist the compaction checkpoint as a host-owned
                // fact — the rollover point and its model identity survive
                // compaction. Non-fatal on failure.
                let model_hash = format!(
                    "sha1:{}",
                    m31a_core::world_state::digest_hex(self.client.model())
                );
                let _ = self.storage.save_retained_fact(
                    session_id,
                    "compaction_checkpoint",
                    "engine",
                    &serde_json::json!({
                        "window_id": window_id,
                        "original_tokens": compacted.original_tokens,
                        "compacted_tokens": compacted.compacted_tokens,
                        "turns_summarized": compacted.turns_summarized,
                        "tools_digested": compacted.tools_digested,
                    })
                    .to_string(),
                    Some(&model_hash),
                );
                history = compacted.compacted_messages;
                // Re-arm the wrap-up reminder and background preparation for
                // the new context window (§5.1, §5.2).
                wrap_up_reminded = false;
                prepare_armed = true;
                if let Some(hooks) = &self.hooks {
                    let _ = hooks
                        .dispatch(
                            HookEvent::PostCompact,
                            "",
                            serde_json::json!({
                                "compacted_tokens": compacted.compacted_tokens,
                            }),
                        )
                        .await;
                }
            }

            // §5.2: background compaction preparation — at the prepare ratio
            // (default 65% of the window), spawn the compaction transform off
            // the turn loop so a prepared artifact is ready before the
            // foreground trigger. One preparation per context window.
            if prepare_armed {
                let prepare_threshold = self.compactor.budget().prepare_threshold_tokens();
                if prepare_threshold > 0 && estimate_history_tokens(&history) >= prepare_threshold {
                    prepare_armed = false;
                    let snapshot = history.clone();
                    let slot = Arc::clone(&prepared);
                    let compactor = self.compactor.clone();
                    tokio::spawn(async move {
                        let result = compactor.compact_unconditional(&snapshot);
                        let mut guard = slot.lock().unwrap();
                        if guard.is_none() {
                            *guard = Some(PreparedCompaction {
                                snapshot_len: snapshot.len(),
                                result,
                            });
                        }
                    });
                    let _ = event_tx.send(EngineEvent::Notice(
                        format!(
                            "Background compaction preparation started (~{} tokens).",
                            estimate_history_tokens(&history)
                        ),
                        false,
                    ));
                }
            }

            // Dynamic model routing (opt-in via `routing_enabled`): classify the task and
            // swap the model per tier. Router tier mapping only yields nvidia_nim models.
            let mut turn_client = if self.routing_enabled {
                let tier = classify_task_tier(&last_user_text(&history), None);
                let routed = router.route_for_tier(tier);
                let _ = event_tx.send(EngineEvent::Notice(
                    format!("Routing: tier={} model={}", tier.as_str(), routed.model_id),
                    false,
                ));
                self.client.with_model(routed.model_id)
            } else {
                self.client.clone()
            };

            // §5.1 wrap-up reminder: when context pressure is within ~15% of the
            // compaction trigger, tell the model to converge before the rollover.
            // Pushed after routing so router classification still sees the task
            // text. Fires once per context window; re-arms after compaction.
            if !wrap_up_reminded {
                let trigger_tokens = self.compactor.budget().trigger_threshold_tokens();
                let estimated = estimate_history_tokens(&history);
                if trigger_tokens > 0
                    && estimated >= (trigger_tokens as f64 * WRAP_UP_REMINDER_THRESHOLD) as usize
                {
                    wrap_up_reminded = true;
                    let remaining = trigger_tokens.saturating_sub(estimated);
                    let reminder = format!(
                        "<context_window>\n\
                         The conversation is approaching the automatic compaction threshold: ~{estimated} of {trigger_tokens} tokens used (~{remaining} remaining before the context window rolls over).\n\
                         Wrap up the current task now: finish in-flight work, summarize results, and avoid starting large new subtasks.\n\
                         </context_window>"
                    );
                    let _ = event_tx.send(EngineEvent::Notice(
                        "Context window nearly full — wrap-up reminder injected (compaction imminent).".to_string(),
                        true,
                    ));
                    history.push(ChatMessage::user(reminder));
                }
            }

            let turn_id = uuid::Uuid::new_v4().to_string();
            if self.persist_items {
                let _ = self.storage.save_turn(&turn_id, session_id, turn, "active");
            }

            // Normalize history to fix any orphan tool calls, empty outputs, or displaced system messages
            crate::history_normalizer::normalize_tool_history(&mut history);

            // P0-A invariant: Authoritative system instructions must be at head of history before dispatch
            let has_authoritative_head = history
                .first()
                .map(m31a_core::context_compactor::is_authoritative_system_message)
                .unwrap_or(false);
            if let (false, Some(prompt)) = (
                has_authoritative_head,
                self.authoritative_system_prompt.as_ref(),
            ) {
                history.insert(0, ChatMessage::system(prompt.clone()));
            }

            // 3. Setup streaming callbacks with DeltaBatcher (50ms interval)
            let streamed_content = Arc::new(std::sync::Mutex::new(String::new()));
            let streamed_tokens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let delta_batcher = Arc::new(std::sync::Mutex::new(
                crate::delta_batcher::DeltaBatcher::new(50),
            ));

            let content_acc = streamed_content.clone();
            let token_acc = streamed_tokens.clone();
            let tx_content = event_tx.clone();
            let batcher_content = delta_batcher.clone();
            let on_content: StreamCallback = Arc::new(move |chunk: &str| {
                if let Ok(mut lock) = content_acc.lock() {
                    lock.push_str(chunk);
                }
                token_acc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if let Ok(mut b) = batcher_content.lock() {
                    b.push_content(chunk);
                    if b.is_ready()
                        && let Some(flushed) = b.take_content()
                    {
                        let _ = tx_content.send(EngineEvent::ContentChunk(flushed));
                    }
                }
            });

            let tx_reasoning = event_tx.clone();
            let batcher_reasoning = delta_batcher.clone();
            let on_reasoning: StreamCallback = Arc::new(move |chunk: &str| {
                if let Ok(mut b) = batcher_reasoning.lock() {
                    b.push_reasoning(chunk);
                    if b.is_ready()
                        && let Some(flushed) = b.take_reasoning()
                    {
                        let _ = tx_reasoning.send(EngineEvent::ReasoningChunk(flushed));
                    }
                }
            });

            // 4. Stream chat completion from model
            let chat_future = turn_client.stream_chat(
                &history,
                Some(&tools_def),
                Some(on_content),
                Some(on_reasoning),
            );

            let chat_result = tokio::select! {
                _ = cancel_token.cancelled() => {
                    if let Ok(mut b) = delta_batcher.lock() {
                        let (c, r) = b.flush_all();
                        if let Some(content) = c {
                            let _ = event_tx.send(EngineEvent::ContentChunk(content));
                        }
                        if let Some(reasoning) = r {
                            let _ = event_tx.send(EngineEvent::ReasoningChunk(reasoning));
                        }
                    }
                    let _ = event_tx.send(EngineEvent::Notice("Turn cancelled by user.".into(), true));
                    break;
                }
                res = chat_future => res,
            };

            // Flush all remaining buffered deltas immediately at stream boundary before tool handling
            if let Ok(mut b) = delta_batcher.lock() {
                let (c, r) = b.flush_all();
                if let Some(content) = c {
                    let _ = event_tx.send(EngineEvent::ContentChunk(content));
                }
                if let Some(reasoning) = r {
                    let _ = event_tx.send(EngineEvent::ReasoningChunk(reasoning));
                }
            }

            match chat_result {
                Ok((response_msg, usage)) => {
                    total_usage.prompt_tokens += usage.prompt_tokens;
                    total_usage.completion_tokens += usage.completion_tokens;
                    total_usage.reasoning_tokens += usage.reasoning_tokens;
                    total_usage.total_tokens += usage.total_tokens;
                    stream_attempt = 0;

                    let has_tools = response_msg.tool_calls.is_some();
                    history.push(response_msg.clone());

                    if self.persist_items
                        && let Some(ref text) = response_msg.content
                        && !text.is_empty()
                    {
                        let _ = self.storage.save_item(
                            &uuid::Uuid::new_v4().to_string(),
                            session_id,
                            Some(&turn_id),
                            "assistant",
                            text,
                        );
                    }

                    if let Some(tool_calls) = response_msg.tool_calls {
                        // Pre-turn git checkpoint (GhostCommit parity)
                        if self.auto_checkpoint && self.persist_items && !tool_calls.is_empty() {
                            let cp_label = format!("Turn {} Pre-Execution Snapshot", turn);
                            let git_commit = std::process::Command::new("git")
                                .args(["rev-parse", "HEAD"])
                                .current_dir(&self.cwd)
                                .output()
                                .ok()
                                .and_then(|o| String::from_utf8(o.stdout).ok())
                                .map(|s| s.trim().to_string());
                            let _ = self.storage.create_checkpoint(
                                session_id,
                                &cp_label,
                                git_commit.as_deref(),
                                turn,
                                &[],
                                None,
                            );
                        }

                        let all_readonly = tool_calls.len() > 1
                            && tool_calls
                                .iter()
                                .all(|c| is_parallel_safe_tool(&c.function.name));

                        if all_readonly && !cancel_token.is_cancelled() {
                            // Concurrent parallel execution of independent read-only tools
                            let mut join_handles = Vec::new();
                            for call in tool_calls {
                                let call_id = call.id.clone();
                                let fn_name = call.function.name.clone();
                                let fn_args = call.function.arguments.clone();

                                // Guardian static review (ApproveForMe): zero-latency local
                                // heuristics only — parallel read-only calls skip the LLM
                                // reviewer to preserve concurrency benefits.
                                if guardian_required_for_call(&self.approval_policy, &self.guardian)
                                {
                                    let guardian = self.guardian.as_ref().unwrap();
                                    let args_value =
                                        serde_json::from_str::<serde_json::Value>(&fn_args)
                                            .unwrap_or(serde_json::Value::String(fn_args.clone()));
                                    let action = m31a_guardian::PlannedAction::new(
                                        fn_name.clone(),
                                        args_value,
                                        m31a_guardian::PlannedActionKind::Command,
                                    );
                                    if let Some(a) = guardian.evaluate_action_locally(&action)
                                        && !a.approved
                                    {
                                        let denial = format!(
                                            "Tool execution blocked by safety reviewer: {}",
                                            a.rationale
                                        );
                                        let _ = event_tx.send(EngineEvent::ToolCallStarted {
                                            id: call_id.clone(),
                                            name: fn_name.clone(),
                                            args: fn_args.clone(),
                                        });
                                        let _ = event_tx.send(EngineEvent::ToolCallFinished {
                                            id: call_id.clone(),
                                            output: denial.clone(),
                                            success: false,
                                        });
                                        let _ = event_tx.send(EngineEvent::GuardianDenied {
                                            rationale: a.rationale.clone(),
                                        });
                                        if self.persist_items {
                                            let _ = self.storage.save_item(
                                                &uuid::Uuid::new_v4().to_string(),
                                                session_id,
                                                Some(&turn_id),
                                                "guardian_denial",
                                                &denial,
                                            );
                                        }
                                        history.push(ChatMessage::tool_response(call_id, denial));
                                        continue;
                                    }
                                }

                                let tools = self.tools.clone();
                                let cwd = self.cwd.clone();
                                let sandbox = self.sandbox_policy;
                                let tx = event_tx.clone();

                                let _ = tx.send(EngineEvent::ToolCallStarted {
                                    id: call_id.clone(),
                                    name: fn_name.clone(),
                                    args: fn_args.clone(),
                                });

                                join_handles.push(tokio::spawn(async move {
                                    let res =
                                        tools.execute(&fn_name, &fn_args, &cwd, sandbox).await;
                                    (call_id, fn_name, fn_args, res)
                                }));
                            }

                            for h in join_handles {
                                if let Ok((c_id, f_name, f_args, res)) = h.await {
                                    self.ingest_tool_result(
                                        &mut history,
                                        session_id,
                                        &turn_id,
                                        &c_id,
                                        &f_name,
                                        &f_args,
                                        res,
                                        &event_tx,
                                        &mut heal_iterations_used,
                                        &mut timeline,
                                        turn,
                                        &mut oscillation_detector,
                                    );
                                }
                            }
                        } else {
                            // Sequential execution with approval and elicitation checks
                            for call in tool_calls {
                                if cancel_token.is_cancelled() {
                                    break;
                                }

                                let call_id = call.id.clone();
                                let fn_name = call.function.name.clone();
                                let mut fn_args = call.function.arguments.clone();

                                // Interactive question choice elicitation
                                if fn_name == "request_user_input"
                                    || fn_name == "question"
                                    || fn_name == "ask_question"
                                {
                                    let mut items: Vec<(String, Vec<String>, bool)> = Vec::new();

                                    if let Ok(v) =
                                        serde_json::from_str::<serde_json::Value>(&fn_args)
                                    {
                                        if let Some(questions) =
                                            v.get("questions").and_then(|q| q.as_array())
                                        {
                                            for q_val in questions {
                                                let q_text = q_val
                                                    .get("question")
                                                    .or_else(|| q_val.get("prompt"))
                                                    .and_then(|p| p.as_str())
                                                    .unwrap_or("Question:")
                                                    .to_string();
                                                let mut options = Vec::new();
                                                if let Some(opts) =
                                                    q_val.get("options").and_then(|o| o.as_array())
                                                {
                                                    for o in opts {
                                                        if let Some(s) = o.as_str() {
                                                            options.push(s.to_string());
                                                        }
                                                    }
                                                }
                                                let is_multi = q_val
                                                    .get("is_multi_select")
                                                    .and_then(|b| b.as_bool())
                                                    .unwrap_or(false);
                                                items.push((q_text, options, is_multi));
                                            }
                                        } else {
                                            let q_text = v
                                                .get("question")
                                                .or_else(|| v.get("prompt"))
                                                .and_then(|p| p.as_str())
                                                .unwrap_or("Input requested by agent:")
                                                .to_string();
                                            let mut options = Vec::new();
                                            if let Some(opts) =
                                                v.get("options").and_then(|o| o.as_array())
                                            {
                                                for o in opts {
                                                    if let Some(s) = o.as_str() {
                                                        options.push(s.to_string());
                                                    }
                                                }
                                            }
                                            let is_multi = v
                                                .get("is_multi_select")
                                                .and_then(|b| b.as_bool())
                                                .unwrap_or(false);
                                            items.push((q_text, options, is_multi));
                                        }
                                    }

                                    if items.is_empty() {
                                        items.push((
                                            "Please provide input:".to_string(),
                                            Vec::new(),
                                            false,
                                        ));
                                    }

                                    let mut answers_summary = Vec::new();

                                    for (question_text, options, is_multi) in items {
                                        let q_item = m31a_elicitation::QuestionItem::new(
                                            question_text.clone(),
                                            options,
                                            is_multi,
                                        );
                                        let (q_tx, q_rx) = oneshot::channel();
                                        let _ = event_tx.send(EngineEvent::QuestionRequested {
                                            question: q_item,
                                            response_tx: q_tx,
                                        });

                                        let (user_replied, answer) = match q_rx.await {
                                            Ok(ans) if !ans.is_empty() => (true, ans.join(", ")),
                                            _ => (
                                                false,
                                                "[no user reply — incomplete evidence rather than a partial grant] Proceed with default recommendation.".to_string(),
                                            ),
                                        };

                                        // §5.5: verified Q/A persists as a host-owned
                                        // fact (survives compaction outside the
                                        // summarization contract). Non-fatal on failure.
                                        let fact_content = if user_replied {
                                            format!(
                                                "[Verified answer] Q: {question_text} → A: {answer}"
                                            )
                                        } else {
                                            format!(
                                                "[no user reply — incomplete evidence rather than a partial grant] Q: {question_text}"
                                            )
                                        };
                                        let _ = self.storage.save_retained_fact(
                                            session_id,
                                            "verified_qa",
                                            "engine",
                                            &fact_content,
                                            None,
                                        );

                                        answers_summary
                                            .push(format!("Q: {}\nA: {}", question_text, answer));
                                    }

                                    let combined = answers_summary.join("\n\n");
                                    history.push(ChatMessage::tool_response(
                                        call_id.clone(),
                                        combined.clone(),
                                    ));
                                    let _ = event_tx.send(EngineEvent::ToolCallFinished {
                                        id: call_id.clone(),
                                        output: combined,
                                        success: true,
                                    });
                                    continue;
                                }

                                // §4.4 PreToolUse hook: may rewrite the tool
                                // input or block the call before any policy
                                // evaluation.
                                if let Some(hooks) = &self.hooks {
                                    let input_value =
                                        serde_json::from_str::<serde_json::Value>(&fn_args)
                                            .unwrap_or(serde_json::Value::Null);
                                    match hooks
                                        .dispatch(
                                            HookEvent::PreToolUse,
                                            &fn_name,
                                            serde_json::json!({ "tool": fn_name, "input": input_value }),
                                        )
                                        .await
                                    {
                                        HookDisposition::Rewrite { input } => {
                                            fn_args = input;
                                        }
                                        HookDisposition::Block(reason) => {
                                            let _ = event_tx.send(EngineEvent::ToolCallStarted {
                                                id: call_id.clone(),
                                                name: fn_name.clone(),
                                                args: fn_args.clone(),
                                            });
                                            let _ = event_tx.send(EngineEvent::ToolCallFinished {
                                                id: call_id.clone(),
                                                output: reason.clone(),
                                                success: false,
                                            });
                                            if self.persist_items {
                                                let _ = self.storage.save_item(
                                                    &uuid::Uuid::new_v4().to_string(),
                                                    session_id,
                                                    Some(&turn_id),
                                                    "tool_error",
                                                    &reason,
                                                );
                                            }
                                            history.push(ChatMessage::tool_response(call_id.clone(), reason));
                                            continue;
                                        }
                                        HookDisposition::Allow => {}
                                    }
                                }

                                // §2.4: code-mode `exec`/`wait` route through the
                                // orchestrator instead of the registry — these are
                                // engine-level tools with cell state, not registry
                                // tools, so they are intercepted before execpolicy
                                // (their payload is JS source, not a shell command).
                                if let Some(code_mode) = self.code_mode.as_ref().filter(|_| {
                                    fn_name == m31a_code_mode::EXEC_TOOL_NAME
                                        || fn_name == m31a_code_mode::WAIT_TOOL_NAME
                                }) {
                                    let _ = event_tx.send(EngineEvent::ToolCallStarted {
                                        id: call_id.clone(),
                                        name: fn_name.clone(),
                                        args: fn_args.clone(),
                                    });

                                    let args_value =
                                        serde_json::from_str::<serde_json::Value>(&fn_args)
                                            .unwrap_or(serde_json::Value::Null);
                                    let res = if fn_name == m31a_code_mode::EXEC_TOOL_NAME {
                                        match args_value.get("code").and_then(|c| c.as_str()) {
                                            Some(code) => {
                                                code_mode
                                                    .execute_batch(
                                                        code,
                                                        &self.tools,
                                                        &self.cwd,
                                                        self.sandbox_policy,
                                                    )
                                                    .await
                                            }
                                            None => Err(anyhow::anyhow!("Missing 'code' argument")),
                                        }
                                    } else {
                                        let cell_id = args_value
                                            .get("cell_id")
                                            .and_then(|c| c.as_str())
                                            .unwrap_or_default()
                                            .to_string();
                                        let yield_ms = args_value
                                            .get("yield_time_ms")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(m31a_code_mode::DEFAULT_EXEC_YIELD_TIME_MS);
                                        let max_tokens = args_value
                                            .get("max_tokens")
                                            .and_then(|v| v.as_u64())
                                            .map(|n| n as usize);
                                        let terminate = args_value
                                            .get("terminate")
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false);
                                        code_mode
                                            .wait_cell(&cell_id, yield_ms, max_tokens, terminate)
                                    };

                                    self.ingest_tool_result(
                                        &mut history,
                                        session_id,
                                        &turn_id,
                                        &call_id,
                                        &fn_name,
                                        &fn_args,
                                        res,
                                        &event_tx,
                                        &mut heal_iterations_used,
                                        &mut timeline,
                                        turn,
                                        &mut oscillation_detector,
                                    );
                                    continue;
                                }

                                // Policy and Approval check
                                let mut needs_approval = false;
                                let mut reason = String::new();
                                let mut cmd_str = None;

                                if fn_name == "exec_command"
                                    && let Ok(val) =
                                        serde_json::from_str::<serde_json::Value>(&fn_args)
                                    && let Some(cmd) = val.get("command").and_then(|c| c.as_str())
                                {
                                    cmd_str = Some(cmd.to_string());
                                    let decision = self
                                        .exec_policy
                                        .read()
                                        .map(|ep| ep.evaluate(cmd))
                                        .unwrap_or(Decision::Prompt);
                                    match decision {
                                        Decision::Deny => {
                                            let _ = event_tx.send(EngineEvent::ToolCallStarted {
                                                id: call_id.clone(),
                                                name: fn_name.clone(),
                                                args: fn_args.clone(),
                                            });
                                            let denial =
                                                "Execution hard-denied by security policy."
                                                    .to_string();
                                            let _ = event_tx.send(EngineEvent::ToolCallFinished {
                                                id: call_id.clone(),
                                                output: denial.clone(),
                                                success: false,
                                            });
                                            history.push(ChatMessage::tool_response(
                                                call_id.clone(),
                                                denial,
                                            ));
                                            continue;
                                        }
                                        Decision::Prompt => {
                                            needs_approval = true;
                                            reason = "Command requires interactive confirmation"
                                                .to_string();
                                        }
                                        Decision::Allow => {
                                            if self.approval_policy == ApprovalPolicy::OnRequest {
                                                needs_approval = true;
                                                reason = "Interactive approval required for shell execution".to_string();
                                            }
                                        }
                                    }
                                }

                                // File mutations can be approved per file for the rest of the
                                // session. Without an explicit session exception every mutating
                                // call is confirmed individually under OnRequest.
                                if matches!(fn_name.as_str(), "write_file" | "edit" | "apply_patch")
                                    && self.approval_policy == ApprovalPolicy::OnRequest
                                {
                                    let path = serde_json::from_str::<serde_json::Value>(&fn_args)
                                        .ok()
                                        .and_then(|value| {
                                            value
                                                .get("path")
                                                .or_else(|| value.get("file_path"))
                                                .and_then(|path| path.as_str())
                                                .map(PathBuf::from)
                                        });
                                    if let Some(path) = path {
                                        let allowed = self
                                            .session_allowed_files
                                            .read()
                                            .map(|allowed| allowed.contains(&path))
                                            .unwrap_or(false);
                                        if !allowed {
                                            needs_approval = true;
                                            reason = format!(
                                                "File modification requires interactive confirmation: {}",
                                                path.display()
                                            );
                                        }
                                    }
                                }

                                // Guardian automated review (report §2.2): enforced between
                                // execpolicy evaluation and the interactive approval oneshot.
                                // Denials short-circuit execution; reviewer errors degrade to a
                                // warning Notice and normal approval flow (CLI parity).
                                if guardian_required_for_call(&self.approval_policy, &self.guardian)
                                {
                                    let guardian = self.guardian.as_ref().unwrap();
                                    let task_context = last_user_text(&history);
                                    match guardian
                                        .review(&turn_client, &fn_name, &fn_args, &task_context)
                                        .await
                                    {
                                        Ok(assessment) if !assessment.approved => {
                                            let _ = event_tx.send(EngineEvent::ToolCallStarted {
                                                id: call_id.clone(),
                                                name: fn_name.clone(),
                                                args: fn_args.clone(),
                                            });
                                            let denial = format!(
                                                "Tool execution blocked by safety reviewer: {}",
                                                assessment.rationale
                                            );
                                            let _ = event_tx.send(EngineEvent::ToolCallFinished {
                                                id: call_id.clone(),
                                                output: denial.clone(),
                                                success: false,
                                            });
                                            let _ = event_tx.send(EngineEvent::GuardianDenied {
                                                rationale: assessment.rationale.clone(),
                                            });
                                            if self.persist_items {
                                                let _ = self.storage.save_item(
                                                    &uuid::Uuid::new_v4().to_string(),
                                                    session_id,
                                                    Some(&turn_id),
                                                    "guardian_denial",
                                                    &denial,
                                                );
                                            }
                                            history.push(ChatMessage::tool_response(
                                                call_id.clone(),
                                                denial,
                                            ));
                                            continue;
                                        }
                                        Ok(_) => {
                                            // Guardian approved on the user's behalf — skip the
                                            // interactive approval oneshot (ApproveForMe semantics).
                                            needs_approval = false;
                                        }
                                        Err(e) => {
                                            let _ = event_tx.send(EngineEvent::Notice(
                                                format!("Guardian review failed: {} — continuing with normal approval flow", e),
                                                true,
                                            ));
                                        }
                                    }
                                }

                                // §CollaborationModes: Plan Mode enforcement
                                let plan_violation = if self.collaboration_mode
                                    == CollaborationMode::Plan
                                {
                                    m31a_execpolicy::evaluate_plan_mode(&fn_name, Some(&fn_args))
                                        .err()
                                } else {
                                    None
                                };
                                if let Some(violation) = plan_violation {
                                    let blocked_reason = format!(
                                        "Tool '{}' is blocked: Agent is in Plan Mode. {violation}. Call 'update_plan' to document proposed execution steps or switch to Default/Code Mode.",
                                        fn_name
                                    );
                                    let _ = event_tx.send(EngineEvent::ToolCallStarted {
                                        id: call_id.clone(),
                                        name: fn_name.clone(),
                                        args: fn_args.clone(),
                                    });
                                    let _ = event_tx.send(EngineEvent::ToolCallFinished {
                                        id: call_id.clone(),
                                        output: blocked_reason.clone(),
                                        success: false,
                                    });
                                    history.push(ChatMessage::tool_response(
                                        call_id.clone(),
                                        blocked_reason,
                                    ));
                                    continue;
                                }

                                if needs_approval {
                                    let (resp_tx, resp_rx) = oneshot::channel();
                                    let pending = PendingApproval {
                                        tool_call: call.clone(),
                                        reason,
                                        command: cmd_str,
                                    };

                                    let _ = event_tx.send(EngineEvent::ApprovalRequested {
                                        approval: pending,
                                        response_tx: resp_tx,
                                    });

                                    match resp_rx.await {
                                        Ok(m31a_elicitation::ApprovalDecision::AllowOnce) => {
                                            // Approved for this single invocation
                                        }
                                        Ok(m31a_elicitation::ApprovalDecision::AllowFile {
                                            path,
                                        }) => {
                                            // Session-wide exception for this exact file.
                                            if let Ok(mut allowed) =
                                                self.session_allowed_files.write()
                                            {
                                                allowed.insert(PathBuf::from(path));
                                            }
                                        }
                                        Ok(
                                            m31a_elicitation::ApprovalDecision::AlwaysAllowPattern(
                                                pattern,
                                            ),
                                        ) => {
                                            let prog = pattern
                                                .split_whitespace()
                                                .next()
                                                .unwrap_or(&pattern)
                                                .to_string();
                                            if let Ok(mut ep) = self.exec_policy.write() {
                                                ep.add_prefix_rule(m31a_execpolicy::PrefixRule {
                                                    program: prog,
                                                    rest: Vec::new(),
                                                    decision: m31a_execpolicy::Decision::Allow,
                                                    justification: Some(
                                                        "Interactively allowed by user pattern"
                                                            .to_string(),
                                                    ),
                                                });
                                            }
                                        }
                                        Ok(m31a_elicitation::ApprovalDecision::EditCommand {
                                            new_command,
                                        }) => {
                                            // Interactive command modification: allows user edits before executing shell actions
                                            if let Ok(mut parsed) =
                                                serde_json::from_str::<serde_json::Value>(&fn_args)
                                                && parsed.get("command").is_some()
                                            {
                                                parsed["command"] = serde_json::json!(new_command);
                                                if let Ok(updated) = serde_json::to_string(&parsed)
                                                {
                                                    fn_args = updated;
                                                }
                                            }
                                        }
                                        Ok(m31a_elicitation::ApprovalDecision::Deny { reason }) => {
                                            let _ = event_tx.send(EngineEvent::ToolCallStarted {
                                                id: call_id.clone(),
                                                name: fn_name.clone(),
                                                args: fn_args.clone(),
                                            });
                                            // Interactive feedback on denial: guides model recovery with user clarification
                                            let rejection_msg = if reason.trim().is_empty()
                                                || reason == "Operation denied by user."
                                                || reason == "Action rejected by user."
                                            {
                                                "Action rejected by user.".to_string()
                                            } else {
                                                format!(
                                                    "Action rejected by user. User feedback/guidance: {}",
                                                    reason
                                                )
                                            };
                                            let _ = event_tx.send(EngineEvent::ToolCallFinished {
                                                id: call_id.clone(),
                                                output: rejection_msg.clone(),
                                                success: false,
                                            });
                                            history.push(ChatMessage::tool_response(
                                                call_id.clone(),
                                                rejection_msg,
                                            ));
                                            continue;
                                        }
                                        Err(_) => {
                                            let _ = event_tx.send(EngineEvent::ToolCallStarted {
                                                id: call_id.clone(),
                                                name: fn_name.clone(),
                                                args: fn_args.clone(),
                                            });
                                            let rejection_msg =
                                                "Action rejected by user (approval dismissed)."
                                                    .to_string();
                                            let _ = event_tx.send(EngineEvent::ToolCallFinished {
                                                id: call_id.clone(),
                                                output: rejection_msg.clone(),
                                                success: false,
                                            });
                                            history.push(ChatMessage::tool_response(
                                                call_id.clone(),
                                                rejection_msg,
                                            ));
                                            continue;
                                        }
                                    }
                                }

                                // Execute tool
                                let _ = event_tx.send(EngineEvent::ToolCallStarted {
                                    id: call_id.clone(),
                                    name: fn_name.clone(),
                                    args: fn_args.clone(),
                                });

                                let effective_sandbox =
                                    if self.collaboration_mode == CollaborationMode::Plan {
                                        SandboxPolicy::ReadOnly
                                    } else {
                                        self.sandbox_policy
                                    };
                                let res = self
                                    .tools
                                    .execute(&fn_name, &fn_args, &self.cwd, effective_sandbox)
                                    .await;
                                // §4.4 PostToolUse hook gates result acceptance
                                // (sequential path; the parallel allowlist skips
                                // hooks by design).
                                let res = match (res, &self.hooks) {
                                    (Ok(out), Some(hooks)) => {
                                        let decision = hooks
                                            .dispatch(
                                                HookEvent::PostToolUse,
                                                &fn_name,
                                                serde_json::json!({ "tool": fn_name, "result": out }),
                                            )
                                            .await;
                                        match decision {
                                            HookDisposition::Block(reason) => Err(anyhow::anyhow!(
                                                "Result rejected by hook: {}",
                                                reason
                                            )),
                                            _ => Ok(out),
                                        }
                                    }
                                    (other, _) => other,
                                };
                                self.ingest_tool_result(
                                    &mut history,
                                    session_id,
                                    &turn_id,
                                    &call_id,
                                    &fn_name,
                                    &fn_args,
                                    res,
                                    &event_tx,
                                    &mut heal_iterations_used,
                                    &mut timeline,
                                    turn,
                                    &mut oscillation_detector,
                                );
                            }
                        }
                    }

                    if self.persist_items {
                        let _ = self
                            .storage
                            .save_turn(&turn_id, session_id, turn, "completed");
                    }

                    if !has_tools {
                        // Bolt.diy state machine parity: reject premature stop if code was modified
                        // but no tests or compiler verification passed since the modification.
                        if let Some(premature_reason) = timeline.is_premature_stop()
                            && turn < max_turns
                        {
                            let _ = event_tx.send(EngineEvent::Notice(
                                format!(
                                    "Verification gate rejected early stop: {}",
                                    premature_reason
                                ),
                                true,
                            ));
                            history.push(ChatMessage::user(format!(
                                    "Premature completion rejected by verification gate: {}\nPlease run testing or compiler verification tools before completing.",
                                    premature_reason
                                )));
                            continue;
                        }

                        // §4.4 Stop hook: may block continuation — the agent
                        // must produce more output before stopping. Bounded to
                        // MAX_STOP_HOOK_BLOCKS forced continuations.
                        if let Some(hooks) = &self.hooks
                            && stop_hook_blocks < MAX_STOP_HOOK_BLOCKS
                        {
                            let last = history
                                .iter()
                                .rev()
                                .find(|m| m.role == m31a_core::types::Role::Assistant)
                                .and_then(|m| m.content.clone())
                                .unwrap_or_default();
                            if let HookDisposition::Block(reason) = hooks
                                .dispatch(
                                    HookEvent::Stop,
                                    "",
                                    serde_json::json!({ "last_assistant": last }),
                                )
                                .await
                            {
                                stop_hook_blocks += 1;
                                let _ = event_tx.send(EngineEvent::Notice(
                                    format!(
                                        "Stop hook blocked continuation ({} of {}): {}",
                                        stop_hook_blocks, MAX_STOP_HOOK_BLOCKS, reason
                                    ),
                                    true,
                                ));
                                history.push(ChatMessage::user(format!(
                                    "You must continue before stopping: {reason}"
                                )));
                                continue;
                            }
                        }
                        let _ = event_tx.send(EngineEvent::TurnComplete {
                            usage: total_usage.clone(),
                            messages: history,
                        });
                        break;
                    }

                    if turn == max_turns && has_tools {
                        let _ = event_tx.send(EngineEvent::Notice(
                            format!("Maximum turn depth ({}) reached.", max_turns),
                            true,
                        ));
                        let _ = event_tx.send(EngineEvent::TurnComplete {
                            usage: total_usage.clone(),
                            messages: history,
                        });
                        break;
                    }
                }
                Err(err) => {
                    let acc = streamed_content
                        .lock()
                        .map(|g| g.clone())
                        .unwrap_or_default();
                    let tok_count = streamed_tokens.load(std::sync::atomic::Ordering::Relaxed);
                    match crate::stream_recovery::classify_stream_failure(
                        &err,
                        tok_count,
                        &acc,
                        stream_attempt,
                    ) {
                        crate::stream_recovery::StreamOutcome::TransparentRetry { delay_ms } => {
                            let _ = event_tx.send(EngineEvent::Notice(
                                format!(
                                    "Model request transiently failed (retrying in {}ms): {}",
                                    delay_ms, err
                                ),
                                true,
                            ));
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                            stream_attempt += 1;
                            continue;
                        }
                        crate::stream_recovery::StreamOutcome::ContinueInterrupted {
                            partial_content,
                        } => {
                            let _ = event_tx.send(EngineEvent::Notice(
                                "Streaming connection dropped mid-response. Continuing automatically from partial output...".to_string(),
                                true,
                            ));
                            history.push(ChatMessage::assistant(Some(partial_content), None));
                            history.push(ChatMessage::user(
                                crate::stream_recovery::CONTINUE_INTERRUPTED_PROMPT,
                            ));
                            continue;
                        }
                        crate::stream_recovery::StreamOutcome::ContextOverflow => {
                            let _ = event_tx.send(EngineEvent::Notice(
                                "Provider context window overflow detected: triggering emergency recovery...".to_string(),
                                true,
                            ));
                            if turn < max_turns {
                                history.push(ChatMessage::user(
                                    "CRITICAL: Context window exceeded. Please summarize your previous progress concisely and immediately proceed with the next step.".to_string(),
                                ));
                                continue;
                            }
                            let _ = event_tx.send(EngineEvent::TurnFailed(err.to_string()));
                            break;
                        }
                        _ => {
                            let err_str = err.to_string();
                            let is_retryable = err_str.contains("429")
                                || err_str.contains("rate limit")
                                || err_str.contains("overloaded")
                                || err_str.contains("timeout")
                                || err_str.contains("timed out")
                                || err_str.contains("503")
                                || err_str.contains("502")
                                || err_str.contains("504")
                                || err_str.contains("connection closed")
                                || err_str.contains("connection reset");

                            if is_retryable && stream_attempt < 3 {
                                let tier = classify_task_tier(&last_user_text(&history), None);
                                if let Some(fallback_cfg) = router
                                    .fallback_for_failure_tier(turn_client.model(), Some(tier))
                                {
                                    let backoff_ms = (stream_attempt + 1) * 1000;
                                    let _ = event_tx.send(EngineEvent::Notice(
                                        format!(
                                            "Provider error on model '{}' ({}); executing fallback failover to '{}' in {}ms",
                                            turn_client.model(),
                                            err,
                                            fallback_cfg.model_id,
                                            backoff_ms
                                        ),
                                        true,
                                    ));
                                    turn_client = self.client.with_model(&fallback_cfg.model_id);
                                    tokio::time::sleep(tokio::time::Duration::from_millis(
                                        backoff_ms as u64,
                                    ))
                                    .await;
                                    stream_attempt += 1;
                                    continue;
                                }
                            }

                            let _ = event_tx.send(EngineEvent::TurnFailed(err.to_string()));
                            break;
                        }
                    }
                }
            }
        }

        // §4.4 SessionEnd hook: fires once at run exit (complete/cancel/fail).
        if let Some(hooks) = &self.hooks {
            let _ = hooks
                .dispatch(
                    HookEvent::SessionEnd,
                    "",
                    serde_json::json!({ "reason": "ended" }),
                )
                .await;
        }

        if let Some(telem) = &self.telemetry {
            let _ = telem.send(m31a_telemetry::TelemetryEvent::SessionEnd {
                session_id: session_id.to_string(),
                duration_ms: session_start_time.elapsed().as_millis() as u64,
                total_turns: max_turns,
                total_tokens: total_usage.total_tokens,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
        }

        if let Some(claim) = &active_claim {
            let status = if cancel_token.is_cancelled() {
                "aborted"
            } else {
                "completed"
            };
            let _ = self
                .storage
                .update_execution_claim_status(&claim.claim_id, status);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_allowlist_matches_real_registry_names() {
        // Names verified against m31a-tools/src/registry.rs definitions.
        for name in [
            "read_file",
            "file_search",
            "grep_search",
            "list_dir",
            "outline_file",
            "find_definition",
            "find_references",
            "extract_symbol_body",
            "web_search",
            "list_background_processes",
            "get_background_process_output",
        ] {
            assert!(
                is_parallel_safe_tool(name),
                "`{name}` should be parallel-safe"
            );
        }

        // The old stale names (previously in the allowlist) must never match —
        // they do not exist in the registry and silently degraded batching.
        assert!(!is_parallel_safe_tool("grep_files"));
        assert!(!is_parallel_safe_tool("list_directory"));

        // Mutating / stateful tools must never be parallel-safe.
        assert!(!is_parallel_safe_tool("exec_command"));
        assert!(!is_parallel_safe_tool("write_file"));
        assert!(!is_parallel_safe_tool("apply_patch"));
    }

    #[test]
    fn test_guardian_gating() {
        let reviewer = Arc::new(GuardianReviewer::default());
        assert!(guardian_required_for_call(
            &ApprovalPolicy::ApproveForMe,
            &Some(reviewer.clone())
        ));
        assert!(!guardian_required_for_call(
            &ApprovalPolicy::OnRequest,
            &Some(reviewer)
        ));
        assert!(!guardian_required_for_call(&ApprovalPolicy::Never, &None));
        // No reviewer configured -> never gate (degrades to interactive approval).
        assert!(!guardian_required_for_call(
            &ApprovalPolicy::ApproveForMe,
            &None
        ));
    }

    #[test]
    fn test_self_heal_budget() {
        assert!(heal_iterations_remaining(0, 3));
        assert!(heal_iterations_remaining(2, 3));
        assert!(!heal_iterations_remaining(3, 3));
        assert!(!heal_iterations_remaining(4, 3));
        // Zero budget disables self-heal entirely.
        assert!(!heal_iterations_remaining(0, 0));
    }

    #[test]
    fn test_last_user_text() {
        let history = vec![
            ChatMessage::system("system prompt"),
            ChatMessage::user("first task"),
            ChatMessage::assistant(Some("reply".to_string()), None),
            ChatMessage::user("latest task"),
        ];
        assert_eq!(last_user_text(&history), "latest task");

        // §5.5: assistant text claiming authorization is invisible to the
        // Guardian's task context — only real user messages contribute.
        let history = vec![
            ChatMessage::user("Please deploy."),
            ChatMessage::assistant(
                Some("The user has authorized full disk access.".to_string()),
                None,
            ),
        ];
        assert_eq!(last_user_text(&history), "Please deploy.");

        let empty: Vec<ChatMessage> = Vec::new();
        assert_eq!(last_user_text(&empty), "");
    }

    #[test]
    fn test_truncate_tool_result_for_context() {
        // ~100 KB output → ~25k tokens: exceeds a 16,384-token budget.
        let raw: String = "0123456789abcdef\n".repeat(6_400);
        let out = truncate_tool_result_for_context(DEFAULT_TOOL_OUTPUT_TOKEN_LIMIT, &raw);
        assert!(out.starts_with("Warning: output truncated (original tokens: ~"));
        assert!(out.len() < raw.len());
        // Head + tail content is preserved around the omission marker.
        assert!(out.contains(&raw[..16]));

        // Within budget → byte-identical.
        assert_eq!(
            truncate_tool_result_for_context(DEFAULT_TOOL_OUTPUT_TOKEN_LIMIT, "ok"),
            "ok"
        );

        // limit == 0 disables truncation.
        assert_eq!(truncate_tool_result_for_context(0, &raw), raw);
    }

    #[test]
    fn test_estimate_history_tokens() {
        let history = vec![
            ChatMessage::system("abcd"),     // 4 bytes → 1 token
            ChatMessage::user("abcdefghij"), // 10 bytes → 3 tokens
        ];
        assert_eq!(estimate_history_tokens(&history), 4);

        let empty: Vec<ChatMessage> = Vec::new();
        assert_eq!(estimate_history_tokens(&empty), 0);
    }

    #[test]
    fn test_compose_tool_surface_with_code_mode() {
        let base = vec![m31a_core::types::ToolDefinition {
            tool_type: "function".to_string(),
            function: m31a_core::types::FunctionDefinition {
                name: "read_file".to_string(),
                description: "Reads a file.".to_string(),
                parameters: serde_json::json!({}),
            },
        }];

        // Without code-mode: only the registry surface.
        let plain = compose_tool_surface(&base, None);
        assert_eq!(plain.len(), 1);
        assert!(!plain.iter().any(|d| d.function.name == "exec"));

        // With code-mode: exec/wait appended.
        let with_cm =
            compose_tool_surface(&base, Some(&m31a_code_mode::CodeModeOrchestrator::new()));
        assert!(
            with_cm
                .iter()
                .any(|d| d.function.name == m31a_code_mode::EXEC_TOOL_NAME)
        );
        assert!(
            with_cm
                .iter()
                .any(|d| d.function.name == m31a_code_mode::WAIT_TOOL_NAME)
        );
        assert!(with_cm.iter().any(|d| d.function.name == "read_file"));
    }

    /// §5.2 merge coverage rules: nothing lost, nothing duplicated.
    #[test]
    fn test_merge_prepared_compaction_coverage() {
        let client = ProviderClient::from_provider(m31a_provider::MockProvider::new("test-model"));
        let storage = Storage::open_in_memory().expect("storage");
        let engine = AgentEngine::new(
            client,
            ToolRegistry::new(),
            storage,
            ApprovalPolicy::Never,
            ExecPolicy::standard(),
            SandboxPolicy::DangerFullAccess,
            std::env::temp_dir(),
        );

        // Snapshot S = [a, b, c, d, e, f]; prepared summarized S[..2] = [a, b].
        let s: Vec<ChatMessage> = (0..6)
            .map(|i| ChatMessage::user(format!("msg-{}", (i as u8 + b'a') as char)))
            .collect();
        let summary = ChatMessage::system("### Earlier Dialogue Summary (Compacted Context)");
        let prepared = PreparedCompaction {
            snapshot_len: 6,
            result: CompactedHistory {
                compacted_messages: vec![summary.clone()],
                original_tokens: 100,
                compacted_tokens: 20,
                turns_summarized: 2,
                tools_digested: 0,
            },
        };

        // Live history H = S + [g, h] (append-only growth).
        let mut history = s.clone();
        history.push(ChatMessage::user("msg-g"));
        history.push(ChatMessage::user("msg-h"));

        let merged = engine.merge_prepared_compaction(&history, prepared);
        // [summary] + H[2..] = summary + c,d,e,f,g,h
        assert_eq!(merged.compacted_messages.len(), 7);
        assert_eq!(
            merged.compacted_messages[0].content.as_deref(),
            Some("### Earlier Dialogue Summary (Compacted Context)")
        );
        for i in 2..8 {
            let expect = format!("msg-{}", (i as u8 + b'a') as char);
            assert!(
                merged
                    .compacted_messages
                    .iter()
                    .any(|m| m.content.as_deref() == Some(expect.as_str())),
                "message {expect} lost in merge"
            );
        }
        assert_eq!(merged.turns_summarized, 2);
    }

    #[test]
    fn test_merge_prepared_compaction_preserves_system_prompt_with_new_messages() {
        let client = ProviderClient::from_provider(m31a_provider::MockProvider::new("test-model"));
        let storage = Storage::open_in_memory().expect("storage");
        let engine = AgentEngine::new(
            client,
            ToolRegistry::new(),
            storage,
            ApprovalPolicy::Never,
            ExecPolicy::standard(),
            SandboxPolicy::DangerFullAccess,
            std::env::temp_dir(),
        );

        let unique_prompt = "AUTHORITATIVE SYSTEM INSTRUCTION #882211 - MERGE IMMUTABLE";
        let sys = ChatMessage::system(unique_prompt);
        let summary = ChatMessage::system("### Earlier Dialogue Summary (Compacted Context)");

        // Snapshot S = [sys, a, b, c, d, e, f]; prepared summarized dialogue [a, b].
        let mut s = vec![sys.clone()];
        for i in 0..6 {
            s.push(ChatMessage::user(format!(
                "msg-{}",
                (i as u8 + b'a') as char
            )));
        }

        let prepared = PreparedCompaction {
            snapshot_len: 7,
            result: CompactedHistory {
                compacted_messages: vec![sys.clone(), summary.clone()],
                original_tokens: 200,
                compacted_tokens: 50,
                turns_summarized: 2,
                tools_digested: 0,
            },
        };

        // Live history H = S + [msg-g, msg-h] (messages arrived while preparing)
        let mut history = s.clone();
        history.push(ChatMessage::user("msg-g"));
        history.push(ChatMessage::user("msg-h"));

        let merged = engine.merge_prepared_compaction(&history, prepared);

        // System prompt must be at index 0 byte-for-byte!
        assert_eq!(
            merged.compacted_messages[0].content.as_deref(),
            Some(unique_prompt),
            "System prompt must be at index 0 after prepared compaction merge"
        );
        // Summary message at index 1
        assert_eq!(
            merged.compacted_messages[1].content.as_deref(),
            Some("### Earlier Dialogue Summary (Compacted Context)")
        );
        // Live tail and new messages preserved
        assert!(
            merged
                .compacted_messages
                .iter()
                .any(|m| m.content.as_deref() == Some("msg-g"))
        );
        assert!(
            merged
                .compacted_messages
                .iter()
                .any(|m| m.content.as_deref() == Some("msg-h"))
        );
    }

    /// §5.2 end-to-end: transport-level background-prepare pipeline verification
    /// (prepare notice → consume at trigger) over local wire server.
    #[tokio::test]
    async fn test_background_compaction_end_to_end() {
        let server = m31a_provider::wire_server::LocalWireServer::start()
            .await
            .expect("local wire server must start");
        server.enqueue_text("OK");

        let dir = tempfile::tempdir().expect("tempdir");
        let client = m31a_provider::NvidiaNimClient::new(
            server.api_base(),
            "local-wire-test-key",
            "meta/llama-3.3-70b-instruct",
        );
        let storage = Storage::open_in_memory().expect("storage");
        let mut engine = AgentEngine::new(
            client,
            ToolRegistry::new(),
            storage,
            ApprovalPolicy::Never,
            ExecPolicy::standard(),
            SandboxPolicy::DangerFullAccess,
            dir.path().to_path_buf(),
        );
        engine.persist_items = false;
        engine.compactor = Arc::new(ContextCompactor::new(CompactTokenBudget {
            max_context_tokens: 2_000,
            trigger_ratio: 0.75, // trigger at 1,500
            prepare_ratio: 0.65, // prepare at 1,300
            target_ratio: 0.40,  // target 800
            max_single_tool_output_tokens: 50,
            ..CompactTokenBudget::default()
        }));

        // Seed ~1,450 tokens: one oversized tool output (digestible) + chatter.
        let big_user = "u".repeat(4_800); // ~1,200 tokens
        let tool_out = "t".repeat(400); // ~100 tokens (> 50 → digestible)
        let history = vec![
            ChatMessage::user(big_user),
            ChatMessage {
                role: Role::Tool,
                content: Some(tool_out),
                tool_calls: None,
                tool_call_id: Some("call-1".into()),
                reasoning_content: None,
            },
            ChatMessage::user("x".repeat(200)),
            ChatMessage::user("y".repeat(200)),
            ChatMessage::user("Please answer with OK"),
        ];

        let (tx, mut rx) = mpsc::unbounded_channel();
        let cancel = tokio_util::sync::CancellationToken::new();
        let (_, steer_rx) = mpsc::unbounded_channel();
        engine
            .run_turn("sess-5.2", history, tx, steer_rx, cancel, 1)
            .await
            .expect("turn runs");

        let mut events_seen: Vec<String> = Vec::new();
        let mut prepared_notice = false;
        let mut completed = false;
        while let Ok(ev) = rx.try_recv() {
            match ev {
                EngineEvent::Notice(n, _) => {
                    events_seen.push(format!("Notice({n})"));
                    if n.contains("Background compaction preparation started") {
                        prepared_notice = true;
                    }
                }
                EngineEvent::Compacted {
                    window_id: _,
                    original_tokens,
                    compacted_tokens,
                } => {
                    events_seen.push(format!("Compacted({original_tokens} → {compacted_tokens})"));
                }
                EngineEvent::TurnComplete { .. } => {
                    events_seen.push("TurnComplete".into());
                    completed = true;
                }
                other => events_seen.push(format!("{other:?}")),
            }
        }
        assert!(
            completed,
            "turn must complete without failure; events: {events_seen:?}"
        );
        assert!(
            prepared_notice,
            "background preparation must start; events: {events_seen:?}"
        );
    }

    #[test]
    fn test_approval_decision_helpers() {
        use m31a_elicitation::ApprovalDecision;

        let allow = ApprovalDecision::from(true);
        assert_eq!(allow, ApprovalDecision::AllowOnce);
        assert!(allow.is_allowed());
        assert_eq!(allow.denial_feedback(), None);

        let deny = ApprovalDecision::from(false);
        assert_eq!(
            deny,
            ApprovalDecision::Deny {
                reason: "Operation denied by user.".to_string()
            }
        );
        assert!(!deny.is_allowed());
        assert_eq!(deny.denial_feedback(), Some("Operation denied by user."));

        let custom_deny = ApprovalDecision::Deny {
            reason: "Do not rm -rf, use git clean".to_string(),
        };
        assert!(!custom_deny.is_allowed());
        assert_eq!(
            custom_deny.denial_feedback(),
            Some("Do not rm -rf, use git clean")
        );

        let edit = ApprovalDecision::EditCommand {
            new_command: "cargo check".to_string(),
        };
        assert!(edit.is_allowed());

        let pattern = ApprovalDecision::AlwaysAllowPattern("cargo *".to_string());
        assert!(pattern.is_allowed());
    }

    #[test]
    fn test_plan_mode_tool_gating() {
        assert!(is_plan_mode_blocked_tool("write_file"));
        assert!(is_plan_mode_blocked_tool("apply_patch"));
        assert!(is_plan_mode_blocked_tool("create_worktree"));
        assert!(!is_plan_mode_blocked_tool("read_file"));
        assert!(!is_plan_mode_blocked_tool("update_plan"));
        assert!(!is_plan_mode_blocked_tool("file_search"));

        assert!(is_mutating_shell_command("rm -rf src/old.rs"));
        assert!(is_mutating_shell_command("git commit -m 'wip'"));
        assert!(!is_mutating_shell_command("ls -la"));
        assert!(!is_mutating_shell_command("git status"));

        // Redirection bypasses, subshells, interpreter invocations blocked in Plan Mode
        assert!(is_mutating_shell_command("echo 'foo' > /tmp/evil"));
        assert!(is_mutating_shell_command("cat < /dev/null >> test.txt"));
        assert!(is_mutating_shell_command("$(whoami)"));
        assert!(is_mutating_shell_command(
            "python3 -c 'import os; os.remove(\"foo\")'"
        ));
    }

    #[test]
    fn test_verify_workspace_failure_recorded_as_failure_in_timeline() {
        let mut timeline = m31a_verify::VerificationTimeline::new();
        timeline.record_edit(1);

        // Simulate failing verify_workspace result
        let result: anyhow::Result<String> = Ok(
            "Verification FAILED (2 errors, 0 warnings):\nerror: cannot find value `foo`"
                .to_string(),
        );
        let turn = 2;
        let call_id = "call-verify-1";

        match &result {
            Ok(out) => {
                if out.starts_with("Verification PASSED") {
                    timeline.record_check_result(turn, true, Some(call_id));
                } else {
                    timeline.record_check_result(turn, false, None);
                    timeline.record_failure(turn);
                }
            }
            Err(_) => {
                timeline.record_failure(turn);
            }
        }

        assert_eq!(timeline.last_failure_turn, Some(2));
        assert_eq!(timeline.last_check_turn, None);
        assert!(!timeline.tool_evidences.contains(&call_id.to_string()));

        // Simulate passing verify_workspace result
        let pass_result: anyhow::Result<String> = Ok("Verification PASSED for command `cargo check` (exit code 0).\nNo compiler or test errors found.".to_string());
        let turn = 3;
        let call_id = "call-verify-pass";

        match &pass_result {
            Ok(out) => {
                if out.starts_with("Verification PASSED") {
                    timeline.record_check_result(turn, true, Some(call_id));
                } else {
                    timeline.record_check_result(turn, false, None);
                    timeline.record_failure(turn);
                }
            }
            Err(_) => {
                timeline.record_failure(turn);
            }
        }

        assert_eq!(timeline.last_check_turn, Some(3));
        assert!(timeline.tool_evidences.contains(&call_id.to_string()));
    }

    #[test]
    fn test_turn_loop_with_generic_model_provider() {
        let mock_provider = m31a_provider::MockProvider::new("custom-mock-engine");
        let client = ProviderClient::from_provider(mock_provider);
        let storage = Storage::open_in_memory().unwrap();
        let engine = AgentEngine::new(
            client,
            ToolRegistry::new(),
            storage,
            ApprovalPolicy::Never,
            ExecPolicy::standard(),
            SandboxPolicy::DangerFullAccess,
            std::env::temp_dir(),
        );

        assert_eq!(engine.client.model(), "custom-mock-engine");
        let derived = engine.client.with_model("derived-model-70b");
        assert_eq!(derived.model(), "derived-model-70b");
    }
}
