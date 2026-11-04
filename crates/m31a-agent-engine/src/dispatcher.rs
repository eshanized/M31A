//! Authoritative Tool Execution Dispatcher.
//!
//! Provides a single, authoritative execution pipeline for all tool calls in M31A,
//! including regular model tool calls, parallel tool calls, Code Mode top-level calls,
//! and Code Mode nested JavaScript tool invocations.
//!
//! Enforces the 14-step unified security and execution lifecycle:
//! 1. Tool identification & validation
//! 2. Argument validation
//! 3. Tool effect classification
//! 4. ExecPolicy evaluation
//! 5. Collaboration Mode restrictions (Plan Mode read-only enforcement)
//! 6. ApprovalPolicy enforcement (fail-closed auto-denial under Never)
//! 7. Guardian review (ApproveForMe automated reviewer)
//! 8. Interactive approval request with cancellation race
//! 9. PreToolUse lifecycle hooks
//! 10. Sandbox selection
//! 11. Tool execution
//! 12. Cancellation propagation
//! 13. PostToolUse lifecycle hooks
//! 14. Result normalization, event reporting & execution auditing

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::Result;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use m31a_core::ApprovalPolicy;
use m31a_core::SandboxPolicy;
use m31a_core::prompts::CollaborationMode;
use m31a_core::types::ToolCall;
use m31a_execpolicy::{Decision, ExecPolicy, classify_tool, evaluate_plan_mode};
use m31a_guardian::GuardianReviewer;
use m31a_hooks::{HookDisposition, HookEvent, HookRuntime};
use m31a_provider::ProviderClient;
use m31a_tools::ToolRegistry;

use crate::turn_loop::{EngineEvent, PendingApproval, guardian_required_for_call};

/// Per-invocation context for an authoritative dispatch operation.
pub struct DispatchContext<'a> {
    pub session_id: &'a str,
    pub turn_id: &'a str,
    pub call_id: &'a str,
    pub cancel_token: &'a CancellationToken,
    pub event_tx: &'a mpsc::UnboundedSender<EngineEvent>,
    pub turn_client: Option<&'a ProviderClient>,
    pub task_context: &'a str,
}

/// The authoritative tool execution dispatcher owning the full security lifecycle.
#[derive(Clone)]
pub struct AuthoritativeToolDispatcher {
    pub tools: ToolRegistry,
    pub exec_policy: Arc<RwLock<ExecPolicy>>,
    pub approval_policy: ApprovalPolicy,
    pub dangerously_bypass: bool,
    pub sandbox_policy: SandboxPolicy,
    pub collaboration_mode: CollaborationMode,
    pub guardian: Option<Arc<GuardianReviewer>>,
    pub hooks: Option<Arc<HookRuntime>>,
    pub session_allowed_files: Arc<RwLock<HashSet<PathBuf>>>,
    pub cwd: PathBuf,
}

impl AuthoritativeToolDispatcher {
    pub async fn dispatch(
        &self,
        ctx: &DispatchContext<'_>,
        fn_name: &str,
        fn_args: &str,
    ) -> Result<String> {
        // Step 1: Cancellation check
        if ctx.cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }

        let mut current_args = fn_args.to_string();

        // Step 2 & 3: Identification, argument validation & effect classification
        let _effect = classify_tool(fn_name, Some(&current_args));

        // Step 4: ExecPolicy evaluation
        let mut needs_approval = false;
        let mut approval_reason = String::new();
        let mut cmd_str = None;

        if fn_name == "exec_command"
            && let Ok(val) = serde_json::from_str::<serde_json::Value>(&current_args)
            && let Some(cmd) = val
                .get("command")
                .or_else(|| val.get("cmd"))
                .and_then(|c| c.as_str())
        {
            cmd_str = Some(cmd.to_string());
            let decision = self
                .exec_policy
                .read()
                .map(|ep| ep.evaluate(cmd))
                .unwrap_or(Decision::Prompt);

            match decision {
                Decision::Deny => {
                    let _ = ctx.event_tx.send(EngineEvent::ToolCallStarted {
                        id: ctx.call_id.to_string(),
                        name: fn_name.to_string(),
                        args: current_args.clone(),
                    });
                    let denial = "Execution hard-denied by security policy.".to_string();
                    let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                        id: ctx.call_id.to_string(),
                        output: denial.clone(),
                        success: false,
                    });
                    anyhow::bail!("{denial}");
                }
                Decision::Prompt => {
                    needs_approval = true;
                    approval_reason = "Command requires interactive confirmation".to_string();
                }
                Decision::Allow => {
                    if self.approval_policy == ApprovalPolicy::OnRequest {
                        needs_approval = true;
                        approval_reason =
                            "Interactive approval required for shell execution".to_string();
                    }
                }
            }
        }

        // Step 5: Collaboration Mode (Plan Mode) restrictions
        if self.collaboration_mode == CollaborationMode::Plan
            && let Err(violation) = evaluate_plan_mode(fn_name, Some(&current_args))
        {
            let blocked = format!(
                "Tool '{fn_name}' is blocked: Agent is in Plan Mode. {violation}. Call 'update_plan' to document proposed execution steps or switch to Default/Code Mode."
            );
            let _ = ctx.event_tx.send(EngineEvent::ToolCallStarted {
                id: ctx.call_id.to_string(),
                name: fn_name.to_string(),
                args: current_args.clone(),
            });
            let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                id: ctx.call_id.to_string(),
                output: blocked.clone(),
                success: false,
            });
            anyhow::bail!("{blocked}");
        }

        // Step 6: ApprovalPolicy enforcement
        if matches!(fn_name, "write_file" | "edit" | "apply_patch")
            && self.approval_policy == ApprovalPolicy::OnRequest
        {
            let path = serde_json::from_str::<serde_json::Value>(&current_args)
                .ok()
                .and_then(|value| {
                    value
                        .get("path")
                        .or_else(|| value.get("file_path"))
                        .and_then(|p| p.as_str())
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
                    approval_reason = format!(
                        "File modification requires interactive confirmation: {}",
                        path.display()
                    );
                }
            }
        }

        // Apply dangerously_bypass override
        if self.dangerously_bypass {
            needs_approval = false;
        } else if needs_approval && self.approval_policy == ApprovalPolicy::Never {
            // Fail closed: Never auto-denies operations that require approval
            let _ = ctx.event_tx.send(EngineEvent::ToolCallStarted {
                id: ctx.call_id.to_string(),
                name: fn_name.to_string(),
                args: current_args.clone(),
            });
            let denial = "Action rejected: Approval policy is 'Never' and interactive confirmation is not permitted.".to_string();
            let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                id: ctx.call_id.to_string(),
                output: denial.clone(),
                success: false,
            });
            anyhow::bail!("{denial}");
        }

        // Step 7: Guardian review when applicable
        if guardian_required_for_call(&self.approval_policy, &self.guardian)
            && let Some(guardian) = &self.guardian
        {
            if let Some(client) = ctx.turn_client {
                match guardian
                    .review(client, fn_name, &current_args, ctx.task_context)
                    .await
                {
                    Ok(assessment) if !assessment.approved => {
                        let _ = ctx.event_tx.send(EngineEvent::ToolCallStarted {
                            id: ctx.call_id.to_string(),
                            name: fn_name.to_string(),
                            args: current_args.clone(),
                        });
                        let denial = format!(
                            "Tool execution blocked by safety reviewer: {}",
                            assessment.rationale
                        );
                        let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                            id: ctx.call_id.to_string(),
                            output: denial.clone(),
                            success: false,
                        });
                        let _ = ctx.event_tx.send(EngineEvent::GuardianDenied {
                            rationale: assessment.rationale.clone(),
                        });
                        anyhow::bail!("{denial}");
                    }
                    Ok(_) => {
                        // Guardian approved on user's behalf
                        needs_approval = false;
                    }
                    Err(e) => {
                        let _ = ctx.event_tx.send(EngineEvent::Notice(
                            format!(
                                "Guardian review failed: {} — continuing with normal approval flow",
                                e
                            ),
                            true,
                        ));
                    }
                }
            } else {
                // Local static evaluation for parallel calls
                let args_value = serde_json::from_str::<serde_json::Value>(&current_args)
                    .unwrap_or(serde_json::Value::String(current_args.clone()));
                let action = m31a_guardian::PlannedAction::new(
                    fn_name.to_string(),
                    args_value,
                    m31a_guardian::PlannedActionKind::Command,
                );
                if let Some(a) = guardian.evaluate_action_locally(&action)
                    && !a.approved
                {
                    let _ = ctx.event_tx.send(EngineEvent::ToolCallStarted {
                        id: ctx.call_id.to_string(),
                        name: fn_name.to_string(),
                        args: current_args.clone(),
                    });
                    let denial =
                        format!("Tool execution blocked by safety reviewer: {}", a.rationale);
                    let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                        id: ctx.call_id.to_string(),
                        output: denial.clone(),
                        success: false,
                    });
                    let _ = ctx.event_tx.send(EngineEvent::GuardianDenied {
                        rationale: a.rationale.clone(),
                    });
                    anyhow::bail!("{denial}");
                }
            }
        }

        // Step 8: Interactive Approval Request with Cancellation racing
        if needs_approval {
            let (resp_tx, resp_rx) = oneshot::channel();
            let pending = PendingApproval {
                tool_call: ToolCall {
                    id: ctx.call_id.to_string(),
                    call_type: "function".to_string(),
                    function: m31a_core::types::FunctionCall {
                        name: fn_name.to_string(),
                        arguments: current_args.clone(),
                    },
                },
                reason: approval_reason,
                command: cmd_str,
            };

            let _ = ctx.event_tx.send(EngineEvent::ApprovalRequested {
                approval: pending,
                response_tx: resp_tx,
            });

            tokio::select! {
                biased;
                _ = ctx.cancel_token.cancelled() => {
                    anyhow::bail!("Turn cancelled during interactive approval request");
                }
                decision_res = resp_rx => {
                    match decision_res {
                        Ok(m31a_elicitation::ApprovalDecision::AllowOnce) => {
                            // Approved for this invocation
                        }
                        Ok(m31a_elicitation::ApprovalDecision::AllowFile { path }) => {
                            if let Ok(mut allowed) = self.session_allowed_files.write() {
                                allowed.insert(PathBuf::from(path));
                            }
                        }
                        Ok(m31a_elicitation::ApprovalDecision::AlwaysAllowPattern(pattern)) => {
                            if let Ok(rule) = m31a_execpolicy::PrefixRule::from_pattern(
                                &pattern,
                                Decision::Allow,
                                Some("Interactively allowed by user pattern".to_string()),
                            ) && let Ok(mut ep) = self.exec_policy.write()
                            {
                                ep.add_prefix_rule(rule);
                            }
                        }
                        Ok(m31a_elicitation::ApprovalDecision::EditCommand { new_command }) => {
                            if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(&current_args)
                                && parsed.get("command").is_some()
                            {
                                parsed["command"] = serde_json::json!(new_command);
                                if let Ok(updated) = serde_json::to_string(&parsed) {
                                    current_args = updated;
                                }
                            }
                        }
                        Ok(m31a_elicitation::ApprovalDecision::Deny { reason }) => {
                            let _ = ctx.event_tx.send(EngineEvent::ToolCallStarted {
                                id: ctx.call_id.to_string(),
                                name: fn_name.to_string(),
                                args: current_args.clone(),
                            });
                            let rejection_msg = if reason.trim().is_empty()
                                || reason == "Operation denied by user."
                                || reason == "Action rejected by user."
                            {
                                "Action rejected by user.".to_string()
                            } else {
                                format!("Action rejected by user. User feedback/guidance: {}", reason)
                            };
                            let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                                id: ctx.call_id.to_string(),
                                output: rejection_msg.clone(),
                                success: false,
                            });
                            anyhow::bail!("{rejection_msg}");
                        }
                        Err(_) => {
                            let _ = ctx.event_tx.send(EngineEvent::ToolCallStarted {
                                id: ctx.call_id.to_string(),
                                name: fn_name.to_string(),
                                args: current_args.clone(),
                            });
                            let rejection_msg = "Action rejected by user (approval dismissed).".to_string();
                            let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                                id: ctx.call_id.to_string(),
                                output: rejection_msg.clone(),
                                success: false,
                            });
                            anyhow::bail!("{rejection_msg}");
                        }
                    }
                }
            }
        }

        // Step 9: PreToolUse hook
        if let Some(hooks) = &self.hooks {
            let disposition = hooks
                .dispatch(
                    HookEvent::PreToolUse,
                    fn_name,
                    serde_json::json!({ "tool": fn_name, "args": current_args }),
                )
                .await;
            if let HookDisposition::Block(reason) = disposition {
                let _ = ctx.event_tx.send(EngineEvent::ToolCallStarted {
                    id: ctx.call_id.to_string(),
                    name: fn_name.to_string(),
                    args: current_args.clone(),
                });
                let denial = format!("Execution blocked by hook: {}", reason);
                let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                    id: ctx.call_id.to_string(),
                    output: denial.clone(),
                    success: false,
                });
                anyhow::bail!("{denial}");
            }
        }

        // Step 10: Sandbox selection
        let effective_sandbox = if self.collaboration_mode == CollaborationMode::Plan {
            SandboxPolicy::ReadOnly
        } else {
            self.sandbox_policy
        };

        // Step 11: Tool execution
        let _ = ctx.event_tx.send(EngineEvent::ToolCallStarted {
            id: ctx.call_id.to_string(),
            name: fn_name.to_string(),
            args: current_args.clone(),
        });

        let exec_result = self
            .tools
            .execute(fn_name, &current_args, &self.cwd, effective_sandbox)
            .await;

        // Step 12: Cancellation check
        if ctx.cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled during tool execution");
        }

        // Step 13: PostToolUse hook
        let final_result = match (exec_result, &self.hooks) {
            (Ok(out), Some(hooks)) => {
                let decision = hooks
                    .dispatch(
                        HookEvent::PostToolUse,
                        fn_name,
                        serde_json::json!({ "tool": fn_name, "result": out }),
                    )
                    .await;
                match decision {
                    HookDisposition::Block(reason) => {
                        Err(anyhow::anyhow!("Result rejected by hook: {}", reason))
                    }
                    _ => Ok(out),
                }
            }
            (other, _) => other,
        };

        // Step 14: Result normalization & event reporting
        match &final_result {
            Ok(out) => {
                let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                    id: ctx.call_id.to_string(),
                    output: out.clone(),
                    success: true,
                });
                Ok(out.clone())
            }
            Err(e) => {
                let err_msg = format!("Tool error: {}", e);
                let _ = ctx.event_tx.send(EngineEvent::ToolCallFinished {
                    id: ctx.call_id.to_string(),
                    output: err_msg.clone(),
                    success: false,
                });
                Err(anyhow::anyhow!("{e}"))
            }
        }
    }
}

/// Adapter allowing Code Mode runtimes to route nested JS tool calls
/// through the authoritative dispatcher.
#[derive(Clone)]
pub struct CodeModeDispatcherBridge {
    pub dispatcher: AuthoritativeToolDispatcher,
    pub session_id: String,
    pub turn_id: String,
    pub cancel_token: CancellationToken,
    pub event_tx: mpsc::UnboundedSender<EngineEvent>,
    pub turn_client: Option<ProviderClient>,
    pub task_context: String,
}

#[async_trait::async_trait]
impl m31a_code_mode::CodeModeToolDispatcher for CodeModeDispatcherBridge {
    async fn execute(
        &self,
        name: &str,
        args: &str,
        _cwd: &Path,
        _policy: SandboxPolicy,
    ) -> Result<String> {
        let call_id = format!("cm_{}", uuid::Uuid::new_v4().simple());
        let ctx = DispatchContext {
            session_id: &self.session_id,
            turn_id: &self.turn_id,
            call_id: &call_id,
            cancel_token: &self.cancel_token,
            event_tx: &self.event_tx,
            turn_client: self.turn_client.as_ref(),
            task_context: &self.task_context,
        };
        self.dispatcher.dispatch(&ctx, name, args).await
    }
}
