//! Concurrent approval coordinator managing async waiter multiplexing and cancellation cascades (POL-01, D-09, D-11).

use chrono::Utc;
use sqlx::{Row, SqlitePool};
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
pub const DEFAULT_APPROVAL_TIMEOUT: Duration =
    Duration::from_secs(crate::config::canonical::DEFAULT_APPROVAL_TIMEOUT_SECS);

/// Coordinates concurrent operator approval requests, multiplexing async waiters without blocking the DAG.
#[derive(Clone)]
pub struct ApprovalCoordinator {
    pool: Option<SqlitePool>,
    channel: Arc<RwLock<Option<Arc<dyn ApprovalChannel>>>>,
    waiters: Arc<Mutex<HashMap<ApprovalRequestId, oneshot::Sender<ApprovalAction>>>>,
    task_map: Arc<Mutex<HashMap<TaskId, Vec<ApprovalRequestId>>>>,
    in_memory_states: Arc<Mutex<HashMap<ApprovalRequestId, ApprovalRequestState>>>,
    event_bus: Option<Arc<dyn crate::events::EventBus>>,
}

impl ApprovalCoordinator {
    pub fn new(pool: Option<SqlitePool>, channel: Option<Arc<dyn ApprovalChannel>>) -> Self {
        Self {
            pool,
            channel: Arc::new(RwLock::new(channel)),
            waiters: Arc::new(Mutex::new(HashMap::new())),
            task_map: Arc::new(Mutex::new(HashMap::new())),
            in_memory_states: Arc::new(Mutex::new(HashMap::new())),
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

        // Calculate expiry timestamp
        let effective_expires_at = req.expires_at.unwrap_or_else(|| {
            Utc::now()
                + chrono::Duration::from_std(timeout)
                    .unwrap_or_else(|_| chrono::Duration::seconds(300))
        });
        let expires_at_str = effective_expires_at.to_rfc3339();

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
                    expires_at,
                    created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            .bind(&expires_at_str)
            .bind(created_at_str)
            .execute(pool)
            .await
            .map_err(|e| ApprovalError::Database(e.to_string()))?;
        }

        // Always register initial in-memory state
        {
            let mut states = self.in_memory_states.lock().await;
            states.insert(req.id, ApprovalRequestState::Pending);
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
                    tool_name: Some(req.tool_or_capability.clone()),
                    parameters_summary: Some(req.redacted_args.to_string()),
                    risk_tier: Some(format!("{:?}", req.risk_classification)),
                    agent_role: req.agent_id.as_ref().map(|a| a.to_string()),
                },
            );
            let _ = bus.publish(env).await;
        }

        // Register waiter channel
        let (tx, mut rx) = oneshot::channel();
        {
            let mut waiters = self.waiters.lock().await;
            waiters.insert(req.id, tx);

            if let Some(task_id) = req.task_id {
                let mut task_map = self.task_map.lock().await;
                task_map.entry(task_id).or_default().push(req.id);
            }
        }

        // Await resolution with timeout
        match tokio::time::timeout(timeout, &mut rx).await {
            Ok(Ok(action)) => Ok(action),
            Ok(Err(_)) => {
                // Sender dropped (e.g. cancelled)
                Err(ApprovalError::Cancelled(format!(
                    "Approval request '{}' was cancelled",
                    req.id
                )))
            }
            Err(_) => {
                // Timed out in tokio::time::timeout
                // Check if winner sent an action right before timeout
                if let Ok(action) = rx.try_recv() {
                    return Ok(action);
                }

                {
                    let mut waiters = self.waiters.lock().await;
                    waiters.remove(&req.id);
                }

                let expired = if let Some(pool) = &self.pool {
                    let res = sqlx::query(
                        "UPDATE approval_requests SET resolution_state = 'expired', resolved_at = ? WHERE id = ? AND resolution_state = 'pending'"
                    )
                    .bind(Utc::now().to_rfc3339())
                    .bind(req.id.as_bytes().as_slice())
                    .execute(pool)
                    .await;
                    res.map(|r| r.rows_affected() > 0).unwrap_or(false)
                } else {
                    let mut states = self.in_memory_states.lock().await;
                    if states.get(&req.id) == Some(&ApprovalRequestState::Pending) {
                        states.insert(req.id, ApprovalRequestState::Expired);
                        true
                    } else {
                        false
                    }
                };

                if expired {
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
                }

                Err(ApprovalError::Timeout(format!(
                    "Approval request '{}' timed out after {:?}",
                    req.id, timeout
                )))
            }
        }
    }

    /// Resolve a pending approval request with an operator action.
    ///
    /// Single-winner guarantee under concurrency:
    /// Exactly one resolver can transition durable and in-memory state.
    /// A losing resolver receives an explicit typed error and never notifies the waiter.
    pub async fn resolve_request(
        &self,
        id: ApprovalRequestId,
        action: ApprovalAction,
        resolved_by: &str,
    ) -> Result<(), ApprovalError> {
        let resolution_state = if action.is_allowed() {
            ApprovalRequestState::Approved
        } else {
            ApprovalRequestState::Denied
        };

        let scope_str = action.resolution_scope().map(|s| s.as_str());
        let now_str = Utc::now().to_rfc3339();

        if let Some(pool) = &self.pool {
            // 1. Verify existence and check expiration timestamp
            let row = sqlx::query(
                "SELECT resolution_state, expires_at FROM approval_requests WHERE id = ?",
            )
            .bind(id.as_bytes().as_slice())
            .fetch_optional(pool)
            .await
            .map_err(|e| ApprovalError::Database(e.to_string()))?;

            let (state, expires_at) = match row {
                Some(r) => {
                    let st: String = r.try_get("resolution_state").unwrap_or_default();
                    let exp: Option<String> = r.try_get("expires_at").ok();
                    (st, exp)
                }
                None => {
                    return Err(ApprovalError::NotFound(format!(
                        "Approval request '{}' not found",
                        id
                    )));
                }
            };

            if let Some(exp_str) = expires_at {
                if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&exp_str) {
                    if Utc::now() > exp {
                        let _ = sqlx::query(
                            "UPDATE approval_requests SET resolution_state = 'expired', resolved_at = ? WHERE id = ? AND resolution_state = 'pending'"
                        )
                        .bind(&now_str)
                        .bind(id.as_bytes().as_slice())
                        .execute(pool)
                        .await;

                        return Err(ApprovalError::Timeout(format!(
                            "Approval request '{}' already expired",
                            id
                        )));
                    }
                }
            }

            if state != "pending" {
                return match state.as_str() {
                    "approved" | "denied" => Err(ApprovalError::AlreadyResolved(format!(
                        "Approval request '{}' is already resolved ({})",
                        id, state
                    ))),
                    "expired" => Err(ApprovalError::Timeout(format!(
                        "Approval request '{}' already expired",
                        id
                    ))),
                    "cancelled" => Err(ApprovalError::Cancelled(format!(
                        "Approval request '{}' was cancelled",
                        id
                    ))),
                    other => Err(ApprovalError::InvalidState(format!(
                        "Approval request '{}' cannot be resolved in state: {}",
                        id, other
                    ))),
                };
            }

            // 2. Perform single-winner atomic conditional update
            let res = sqlx::query(
                r#"
                UPDATE approval_requests
                SET resolution_state = ?, resolution_scope = ?, resolved_by = ?, resolved_at = ?
                WHERE id = ? AND resolution_state = 'pending'
                "#,
            )
            .bind(resolution_state.as_str())
            .bind(scope_str)
            .bind(resolved_by)
            .bind(&now_str)
            .bind(id.as_bytes().as_slice())
            .execute(pool)
            .await
            .map_err(|e| ApprovalError::Database(e.to_string()))?;

            if res.rows_affected() == 0 {
                // Another caller resolved or cancelled concurrently and won the race!
                let row =
                    sqlx::query("SELECT resolution_state FROM approval_requests WHERE id = ?")
                        .bind(id.as_bytes().as_slice())
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| ApprovalError::Database(e.to_string()))?;

                let curr_state = row
                    .and_then(|r| r.try_get::<String, _>("resolution_state").ok())
                    .unwrap_or_else(|| "settled".to_string());

                return match curr_state.as_str() {
                    "approved" | "denied" => Err(ApprovalError::AlreadyResolved(format!(
                        "Approval request '{}' is already resolved ({})",
                        id, curr_state
                    ))),
                    "expired" => Err(ApprovalError::Timeout(format!(
                        "Approval request '{}' already expired",
                        id
                    ))),
                    "cancelled" => Err(ApprovalError::Cancelled(format!(
                        "Approval request '{}' was cancelled",
                        id
                    ))),
                    other => Err(ApprovalError::InvalidState(format!(
                        "Approval request '{}' cannot be resolved in state: {}",
                        id, other
                    ))),
                };
            }
        } else {
            // In-memory mode (persistence disabled)
            let mut states = self.in_memory_states.lock().await;
            match states.get(&id) {
                None => {
                    return Err(ApprovalError::NotFound(format!(
                        "No active waiter for approval request '{}'",
                        id
                    )));
                }
                Some(ApprovalRequestState::Pending) => {
                    states.insert(id, resolution_state);
                }
                Some(ApprovalRequestState::Approved) | Some(ApprovalRequestState::Denied) => {
                    let st = states.get(&id).unwrap();
                    return Err(ApprovalError::AlreadyResolved(format!(
                        "Approval request '{}' is already resolved ({})",
                        id,
                        st.as_str()
                    )));
                }
                Some(ApprovalRequestState::Expired) => {
                    return Err(ApprovalError::Timeout(format!(
                        "Approval request '{}' already expired",
                        id
                    )));
                }
                Some(ApprovalRequestState::Cancelled) => {
                    return Err(ApprovalError::Cancelled(format!(
                        "Approval request '{}' was cancelled",
                        id
                    )));
                }
                Some(other) => {
                    return Err(ApprovalError::InvalidState(format!(
                        "Approval request '{}' cannot be resolved in state: {}",
                        id,
                        other.as_str()
                    )));
                }
            }
        }

        // 3. Only the authoritative WINNER delivers action to waiting worker
        let sender = {
            let mut waiters = self.waiters.lock().await;
            waiters.remove(&id)
        };

        if let Some(tx) = sender {
            let _ = tx.send(action.clone());
        }

        // 4. Publish verified resolution event only AFTER confirmed success
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

        Ok(())
    }

    /// Race-safely cancel all pending approvals for a task when cancelled.
    pub async fn cancel_task_approvals(&self, task_id: TaskId) {
        let req_ids = {
            let mut task_map = self.task_map.lock().await;
            task_map.remove(&task_id).unwrap_or_default()
        };

        for id in req_ids {
            let cancelled = if let Some(pool) = &self.pool {
                let res = sqlx::query(
                    "UPDATE approval_requests SET resolution_state = 'cancelled', resolved_at = ? WHERE id = ? AND resolution_state = 'pending'"
                )
                .bind(Utc::now().to_rfc3339())
                .bind(id.as_bytes().as_slice())
                .execute(pool)
                .await;
                res.map(|r| r.rows_affected() > 0).unwrap_or(false)
            } else {
                let mut states = self.in_memory_states.lock().await;
                if states.get(&id) == Some(&ApprovalRequestState::Pending) {
                    states.insert(id, ApprovalRequestState::Cancelled);
                    true
                } else {
                    false
                }
            };

            if cancelled {
                let mut waiters = self.waiters.lock().await;
                if let Some(tx) = waiters.remove(&id) {
                    let _ = tx.send(ApprovalAction::DenyAndCancelTask {
                        reason: "Task was cancelled".to_string(),
                    });
                }
            }
        }
    }
}
