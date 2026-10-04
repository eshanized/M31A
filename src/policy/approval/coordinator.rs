//! Concurrent approval coordinator managing async waiter multiplexing and cancellation cascades (POL-01, D-09, D-11).

use chrono::Utc;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};

use crate::ids::{ApprovalRequestId, TaskId};
use crate::policy::approval::channel::{ApprovalChannel, ApprovalError};
use crate::policy::approval::explanation::format_explanation_packet;
use crate::policy::approval::{ApprovalAction, ApprovalRequest, ApprovalRequestState};
use crate::state::intake::AutonomyMode;

/// Default timeout waiting for operator approval before failing closed.
pub const DEFAULT_APPROVAL_TIMEOUT: Duration = Duration::from_secs(60);

/// Coordinates concurrent operator approval requests, multiplexing async waiters without blocking the DAG.
#[derive(Clone)]
pub struct ApprovalCoordinator {
    pool: Option<SqlitePool>,
    channel: Arc<RwLock<Option<Arc<dyn ApprovalChannel>>>>,
    waiters: Arc<Mutex<HashMap<ApprovalRequestId, oneshot::Sender<ApprovalAction>>>>,
    task_map: Arc<Mutex<HashMap<TaskId, Vec<ApprovalRequestId>>>>,
    event_bus: Option<Arc<dyn crate::events::EventBus>>,
}

impl ApprovalCoordinator {
    pub fn new(pool: Option<SqlitePool>, channel: Option<Arc<dyn ApprovalChannel>>) -> Self {
        Self {
            pool,
            channel: Arc::new(RwLock::new(channel)),
            waiters: Arc::new(Mutex::new(HashMap::new())),
            task_map: Arc::new(Mutex::new(HashMap::new())),
            event_bus: None,
        }
    }

    /// Attach an EventBus for publishing approval lifecycle events.
    pub fn with_event_bus(mut self, bus: Arc<dyn crate::events::EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Attach an operator approval channel at build time.
    pub fn with_channel(self, channel: Arc<dyn ApprovalChannel>) -> Self {
        if let Ok(mut ch) = self.channel.write() {
            *ch = Some(channel);
        }
        self
    }

    /// Dynamically register or replace the operator approval channel.
    pub fn set_channel(&self, channel: Arc<dyn ApprovalChannel>) {
        if let Ok(mut ch) = self.channel.write() {
            *ch = Some(channel);
        }
    }

    /// Check if a request ID currently has an active awaiting receiver.
    pub async fn has_active_waiter(&self, id: &ApprovalRequestId) -> bool {
        let waiters = self.waiters.lock().await;
        waiters.contains_key(id)
    }

    /// Number of currently pending approval requests.
    pub async fn active_waiters_count(&self) -> usize {
        let waiters = self.waiters.lock().await;
        waiters.len()
    }

    /// IDs of currently pending approval requests owned by this coordinator.
    ///
    /// Observability authority for Invariant 4: every user-visible approval ID
    /// MUST be a member of this set (or a durably persisted request row).
    /// Used by architecture tests and operator tooling to prove approval IDs
    /// are real coordinator requests, never fabricated identifiers.
    pub async fn pending_request_ids(&self) -> Vec<crate::ids::ApprovalRequestId> {
        let waiters = self.waiters.lock().await;
        waiters.keys().copied().collect()
    }

    /// Request operator approval for an action.
    ///
    /// Non-negotiable invariant (D-02, D-09, AUT-03):
    /// In Unattended or Plan mode, unresolved ASK strictly converts to DENY without waiting.
    pub async fn request_approval(
        &self,
        req: ApprovalRequest,
        mode: AutonomyMode,
        timeout: Duration,
    ) -> Result<ApprovalAction, ApprovalError> {
        // Invariant 1: Unattended or Plan mode strictly converts ASK to DENY immediately
        if mode == AutonomyMode::Unattended || mode == AutonomyMode::Plan {
            return Ok(ApprovalAction::Deny {
                reason: "Unattended mode strictly converts unresolved ASK to DENY".to_string(),
            });
        }

        // Persist pending approval request in SQLite if database is available
        if let Some(pool) = &self.pool {
            let req_id_bytes = req.id.as_bytes().as_slice();
            let mission_id_bytes = req.mission_id.as_bytes().as_slice();
            let task_id_bytes = req.task_id.as_ref().map(|t| t.as_bytes().to_vec());
            let agent_id_bytes = req.agent_id.as_ref().map(|a| a.as_bytes().to_vec());
            let tool_call_id_str = req.target.target_identifier();
            let norm_args_str = req.normalized_args.to_string();
            let redacted_args_str = req.redacted_args.to_string();
            let resources_str = req.affected_resources.join(",");
            let risk_str = format!("{:?}", req.risk_classification);
            let created_at_str = req.created_at.to_rfc3339();

            sqlx::query(
                r#"
                INSERT INTO approval_requests (
                    id,
                    mission_id,
                    task_id,
                    agent_id,
                    tool_call_id,
                    tool_or_capability,
                    normalized_args_json,
                    redacted_args_json,
                    affected_resources,
                    risk_classification,
                    matched_rule_id,
                    policy_hash,
                    reason,
                    resolution_state,
                    created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(req_id_bytes)
            .bind(mission_id_bytes)
            .bind(task_id_bytes)
            .bind(agent_id_bytes)
            .bind(tool_call_id_str)
            .bind(&req.tool_or_capability)
            .bind(norm_args_str)
            .bind(redacted_args_str)
            .bind(resources_str)
            .bind(risk_str)
            .bind(req.matched_rule_id.as_deref())
            .bind(&req.policy_hash)
            .bind(&req.reason)
            .bind("pending")
            .bind(created_at_str)
            .execute(pool)
            .await
            .map_err(|e| ApprovalError::Database(e.to_string()))?;
        }

        // Notify operator channel
        let packet = format_explanation_packet(&req);
        {
            let ch = self.channel.read().ok().and_then(|guard| guard.clone());
            if let Some(chan) = ch {
                chan.notify_request(&packet).await?;
            }
        }

        // Publish escalation event to event bus if present
        if let Some(bus) = &self.event_bus {
            let env = crate::events::EventEnvelope::new(
                0,
                Some(req.mission_id),
                None,
                "approval_coordinator".to_string(),
                crate::events::EventType::OperatorEscalationRequested {
                    request_id: req.id.to_string(),
                    mission_id: req.mission_id,
                    reason: req.reason.clone(),
                    timeout_seconds: Some(timeout.as_secs()),
                },
            );
            let _ = bus.publish(env).await;
        }

        // Register waiter channel
        let (tx, rx) = oneshot::channel();
        {
            let mut waiters = self.waiters.lock().await;
            waiters.insert(req.id, tx);

            if let Some(task_id) = req.task_id {
                let mut task_map = self.task_map.lock().await;
                task_map.entry(task_id).or_default().push(req.id);
            }
        }

        // Await resolution with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(action)) => Ok(action),
            Ok(Err(_)) => {
                // Sender dropped (e.g. cancelled)
                Err(ApprovalError::Cancelled(format!(
                    "Approval request '{}' was cancelled",
                    req.id
                )))
            }
            Err(_) => {
                // Timed out
                {
                    let mut waiters = self.waiters.lock().await;
                    waiters.remove(&req.id);
                }

                if let Some(pool) = &self.pool {
                    let _ = sqlx::query(
                        "UPDATE approval_requests SET resolution_state = 'expired', resolved_at = ? WHERE id = ?"
                    )
                    .bind(Utc::now().to_rfc3339())
                    .bind(req.id.as_bytes().as_slice())
                    .execute(pool)
                    .await;
                }

                if let Some(bus) = &self.event_bus {
                    let env = crate::events::EventEnvelope::new(
                        0,
                        Some(req.mission_id),
                        None,
                        "approval_coordinator".to_string(),
                        crate::events::EventType::EscalationTimedOut {
                            request_id: req.id.to_string(),
                            mission_id: req.mission_id,
                        },
                    );
                    let _ = bus.publish(env).await;
                }

                Err(ApprovalError::Timeout(format!(
                    "Approval request '{}' timed out after {:?}",
                    req.id, timeout
                )))
            }
        }
    }

    /// Resolve a pending approval request with an operator action.
    pub async fn resolve_request(
        &self,
        id: ApprovalRequestId,
        action: ApprovalAction,
        resolved_by: &str,
    ) -> Result<(), ApprovalError> {
        let sender = {
            let mut waiters = self.waiters.lock().await;
            waiters.remove(&id)
        };

        let resolution_state = if action.is_allowed() {
            ApprovalRequestState::Approved
        } else {
            ApprovalRequestState::Denied
        };

        let scope_str = action.resolution_scope().map(|s| s.as_str());

        let db_updated = if let Some(pool) = &self.pool {
            let res = sqlx::query(
                r#"
                UPDATE approval_requests
                SET resolution_state = ?, resolution_scope = ?, resolved_by = ?, resolved_at = ?
                WHERE id = ?
                "#,
            )
            .bind(resolution_state.as_str())
            .bind(scope_str)
            .bind(resolved_by)
            .bind(Utc::now().to_rfc3339())
            .bind(id.as_bytes().as_slice())
            .execute(pool)
            .await
            .map_err(|e| ApprovalError::Database(e.to_string()))?;
            res.rows_affected() > 0
        } else {
            false
        };

        if let Some(bus) = &self.event_bus {
            let env = crate::events::EventEnvelope::new(
                0,
                None,
                None,
                "approval_coordinator".to_string(),
                crate::events::EventType::ApprovalResolved {
                    request_id: id.to_string(),
                    decision: if action.is_allowed() {
                        "ALLOW".to_string()
                    } else {
                        "DENY".to_string()
                    },
                    resolved_by: resolved_by.to_string(),
                },
            );
            let _ = bus.publish(env).await;
        }

        if let Some(tx) = sender {
            let _ = tx.send(action);
            Ok(())
        } else if db_updated {
            Ok(())
        } else {
            Err(ApprovalError::NotFound(format!(
                "No active waiter for approval request '{}'",
                id
            )))
        }
    }

    /// Race-safely cancel all pending approvals for a task when cancelled.
    pub async fn cancel_task_approvals(&self, task_id: TaskId) {
        let req_ids = {
            let mut task_map = self.task_map.lock().await;
            task_map.remove(&task_id).unwrap_or_default()
        };

        let mut waiters = self.waiters.lock().await;
        for id in req_ids {
            if let Some(tx) = waiters.remove(&id) {
                let _ = tx.send(ApprovalAction::DenyAndCancelTask {
                    reason: "Task was cancelled".to_string(),
                });
            }

            if let Some(pool) = &self.pool {
                let _ = sqlx::query(
                    "UPDATE approval_requests SET resolution_state = 'cancelled', resolved_at = ? WHERE id = ?"
                )
                .bind(Utc::now().to_rfc3339())
                .bind(id.as_bytes().as_slice())
                .execute(pool)
                .await;
            }
        }
    }
}
