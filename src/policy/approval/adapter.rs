//! Production EscalationChannel implementation integrating ApprovalCoordinator and SQLite persistence (AUT-01, POL-01, BLK-02).

use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::kernel::seams::escalation::{
    EscalationChannel, EscalationError, EscalationRequest, EscalationRequestId, EscalationResponse,
    PendingEscalation,
};
use crate::policy::approval::coordinator::ApprovalCoordinator;

/// Production implementation of `EscalationChannel` seam trait.
#[derive(Clone)]
pub struct ProductionEscalationChannel {
    pool: Option<SqlitePool>,
    coordinator: Option<Arc<ApprovalCoordinator>>,
    pending: Arc<Mutex<HashMap<EscalationRequestId, PendingEscalation>>>,
    responses: Arc<Mutex<HashMap<EscalationRequestId, EscalationResponse>>>,
}

impl Default for ProductionEscalationChannel {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ProductionEscalationChannel {
    pub fn new(pool: Option<SqlitePool>) -> Self {
        Self {
            pool,
            coordinator: None,
            pending: Arc::new(Mutex::new(HashMap::new())),
            responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_coordinator(mut self, coordinator: Arc<ApprovalCoordinator>) -> Self {
        self.coordinator = Some(coordinator);
        self
    }

    pub async fn resolve(
        &self,
        id: EscalationRequestId,
        response: EscalationResponse,
    ) -> Result<(), EscalationError> {
        let resolution_state = match &response {
            EscalationResponse::Approved => "approved",
            EscalationResponse::Denied { .. } => "denied",
        };
        let reason = match &response {
            EscalationResponse::Approved => None,
            EscalationResponse::Denied { reason } => Some(reason.as_str()),
        };

        if let Some(ref pool) = self.pool {
            let _ = sqlx::query(
                r#"
                UPDATE approval_requests
                SET resolution_state = ?, reason = COALESCE(?, reason), resolved_by = 'operator', resolved_at = ?
                WHERE id = ?
                "#,
            )
            .bind(resolution_state)
            .bind(reason)
            .bind(Utc::now().to_rfc3339())
            .bind(id.as_bytes().as_slice())
            .execute(pool)
            .await;
        }

        self.responses.lock().await.insert(id, response);
        self.pending.lock().await.remove(&id);
        Ok(())
    }
}

#[async_trait]
impl EscalationChannel for ProductionEscalationChannel {
    async fn request_escalation(
        &self,
        req: EscalationRequest,
    ) -> Result<EscalationRequestId, EscalationError> {
        let req_id = req.id;
        let pending = PendingEscalation::new(req.clone(), Utc::now());

        if let Some(ref pool) = self.pool {
            let req_id_bytes = req_id.as_bytes().as_slice();
            let mission_id_bytes = req.mission_id.as_bytes().as_slice();
            let task_id_bytes = req.task_id.as_ref().map(|t| t.as_bytes().to_vec());
            let created_at_str = Utc::now().to_rfc3339();
            let expires_at_str = pending.expires_at.map(|e| e.to_rfc3339());

            let _ = sqlx::query(
                r#"
                INSERT INTO approval_requests (
                    id, mission_id, task_id, agent_id, tool_call_id,
                    tool_or_capability, normalized_args_json, redacted_args_json,
                    affected_resources, risk_classification, matched_rule_id,
                    policy_hash, reason, resolution_state, expires_at, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(req_id_bytes)
            .bind(mission_id_bytes)
            .bind(task_id_bytes)
            .bind(None::<Vec<u8>>)
            .bind(format!("escalation:{req_id}"))
            .bind("operator_escalation")
            .bind("{}")
            .bind("{}")
            .bind("")
            .bind("High")
            .bind(None::<String>)
            .bind("escalation")
            .bind(&req.reason)
            .bind("pending")
            .bind(expires_at_str)
            .bind(created_at_str)
            .execute(pool)
            .await;
        }

        self.pending.lock().await.insert(req_id, pending);
        Ok(req_id)
    }

    async fn check_response(
        &self,
        id: EscalationRequestId,
    ) -> Result<Option<EscalationResponse>, EscalationError> {
        // 1. Check in-memory responses first
        if let Some(resp) = self.responses.lock().await.get(&id).cloned() {
            return Ok(Some(resp));
        }

        // 2. Check SQLite approval_requests table
        if let Some(ref pool) = self.pool {
            let row = sqlx::query_as::<_, (String, Option<String>)>(
                "SELECT resolution_state, reason FROM approval_requests WHERE id = ?",
            )
            .bind(id.as_bytes().as_slice())
            .fetch_optional(pool)
            .await
            .map_err(|e| EscalationError::Failed(e.to_string()))?;

            if let Some((state, reason)) = row {
                match state.as_str() {
                    "approved" => {
                        let resp = EscalationResponse::Approved;
                        self.responses.lock().await.insert(id, resp.clone());
                        return Ok(Some(resp));
                    }
                    "denied" => {
                        let resp = EscalationResponse::Denied {
                            reason: reason.unwrap_or_else(|| "Denied by operator".to_string()),
                        };
                        self.responses.lock().await.insert(id, resp.clone());
                        return Ok(Some(resp));
                    }
                    "expired" => {
                        let resp = EscalationResponse::Denied {
                            reason: "Escalation request expired".to_string(),
                        };
                        self.responses.lock().await.insert(id, resp.clone());
                        return Ok(Some(resp));
                    }
                    "cancelled" => {
                        let resp = EscalationResponse::Denied {
                            reason: "Escalation request was cancelled".to_string(),
                        };
                        self.responses.lock().await.insert(id, resp.clone());
                        return Ok(Some(resp));
                    }
                    _ => {} // still pending
                }
            }
        }

        // 3. Check for timeout
        let is_timed_out = {
            let pending_map = self.pending.lock().await;
            if let Some(pending) = pending_map.get(&id) {
                pending.evaluate_timeout(Utc::now())
            } else {
                false
            }
        };

        if is_timed_out {
            let resp = EscalationResponse::Denied {
                reason: "Escalation request timed out".to_string(),
            };
            self.responses.lock().await.insert(id, resp.clone());
            self.pending.lock().await.remove(&id);

            if let Some(ref pool) = self.pool {
                let _ = sqlx::query(
                    "UPDATE approval_requests SET resolution_state = 'expired', resolved_at = ? WHERE id = ?"
                )
                .bind(Utc::now().to_rfc3339())
                .bind(id.as_bytes().as_slice())
                .execute(pool)
                .await;
            }

            return Ok(Some(resp));
        }

        Ok(None)
    }
}
