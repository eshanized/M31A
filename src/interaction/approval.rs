//! Interactive CLI Approval Channel and Coordinator Bridge (POL-01, D-09, D-11, CLI-01).
//!
//! "When policy produces ASK, the interactive CLI MUST remain alive."
//! "The answer must reach the authoritative runtime. Support: approve, deny, cancel."

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};

use crate::ids::ApprovalRequestId;
use crate::policy::approval::channel::{ApprovalChannel, ApprovalError};
use crate::policy::approval::{ApprovalAction, ApprovalExplanationPacket};

/// Holds pending approval packet details for presentation to the operator.
#[derive(Debug, Clone)]
pub struct PendingApprovalPrompt {
    pub request_id: ApprovalRequestId,
    pub tool_or_capability: String,
    pub affected_resources: Vec<String>,
    pub reason: String,
    pub justification: String,
    pub risk_tier: String,
}

/// Interactive approval channel connecting runtime policy escalations to the interactive CLI/TUI prompt.
#[derive(Clone)]
pub struct InteractiveApprovalChannel {
    pending_prompts: Arc<Mutex<HashMap<ApprovalRequestId, PendingApprovalPrompt>>>,
    decision_senders: Arc<Mutex<HashMap<ApprovalRequestId, oneshot::Sender<ApprovalAction>>>>,
    active_prompt_listener: Option<Arc<dyn Fn(PendingApprovalPrompt) + Send + Sync>>,
}

impl Default for InteractiveApprovalChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractiveApprovalChannel {
    pub fn new() -> Self {
        Self {
            pending_prompts: Arc::new(Mutex::new(HashMap::new())),
            decision_senders: Arc::new(Mutex::new(HashMap::new())),
            active_prompt_listener: None,
        }
    }

    /// Set an optional listener callback for immediate notification when approval is requested.
    pub fn with_listener(
        mut self,
        listener: Arc<dyn Fn(PendingApprovalPrompt) + Send + Sync>,
    ) -> Self {
        self.active_prompt_listener = Some(listener);
        self
    }

    /// Retrieve currently pending approval prompt if any.
    pub async fn current_pending_prompt(&self) -> Option<PendingApprovalPrompt> {
        let map = self.pending_prompts.lock().await;
        map.values().next().cloned()
    }

    /// Submit an operator decision resolving an outstanding approval request.
    pub async fn submit_decision(
        &self,
        id: ApprovalRequestId,
        action: ApprovalAction,
    ) -> Result<(), ApprovalError> {
        let mut prompts = self.pending_prompts.lock().await;
        prompts.remove(&id);

        let mut senders = self.decision_senders.lock().await;
        if let Some(tx) = senders.remove(&id) {
            let _ = tx.send(action);
            Ok(())
        } else {
            Err(ApprovalError::NotFound(format!(
                "No pending waiter for approval request '{id}'"
            )))
        }
    }
}

#[async_trait]
impl ApprovalChannel for InteractiveApprovalChannel {
    async fn notify_request(
        &self,
        packet: &ApprovalExplanationPacket,
    ) -> Result<(), ApprovalError> {
        let prompt = PendingApprovalPrompt {
            request_id: packet.request_id,
            tool_or_capability: packet.tool_or_capability.clone(),
            affected_resources: packet.affected_resources.clone(),
            reason: packet.reason.clone(),
            justification: packet.summary.clone(),
            risk_tier: format!("{:?}", packet.risk_classification),
        };

        {
            let mut map = self.pending_prompts.lock().await;
            map.insert(packet.request_id, prompt.clone());
        }

        if let Some(ref listener) = self.active_prompt_listener {
            listener(prompt);
        }

        Ok(())
    }

    async fn poll_response(
        &self,
        id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError> {
        // Create a oneshot channel for awaiting the operator response asynchronously
        let (tx, rx) = oneshot::channel();
        {
            let mut senders = self.decision_senders.lock().await;
            senders.insert(id, tx);
        }

        match rx.await {
            Ok(action) => Ok(Some(action)),
            Err(_) => Err(ApprovalError::Cancelled(format!(
                "Approval request '{id}' cancelled before response"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_interactive_approval_channel_notify_and_resolve() {
        let channel = InteractiveApprovalChannel::new();
        let req_id = ApprovalRequestId::new();

        let packet = ApprovalExplanationPacket {
            request_id: req_id,
            tool_or_capability: "write_file".to_string(),
            affected_resources: vec!["src/parser.rs".to_string()],
            risk_classification: crate::tools::risk::RiskClass::HighRiskMutation,
            matched_rule_id: Some("write-rule".to_string()),
            policy_hash: "test-hash".to_string(),
            reason: "Modifying repository code".to_string(),
            summary: "Write new expression parser".to_string(),
            redacted_args: serde_json::json!({"path": "src/parser.rs"}),
        };

        channel.notify_request(&packet).await.unwrap();

        let prompt = channel.current_pending_prompt().await.unwrap();
        assert_eq!(prompt.request_id, req_id);
        assert_eq!(prompt.tool_or_capability, "write_file");

        // Spawn poll task in background
        let chan_clone = channel.clone();
        let handle = tokio::spawn(async move { chan_clone.poll_response(req_id).await });

        // Submit approval
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        channel
            .submit_decision(req_id, ApprovalAction::AllowOnce)
            .await
            .unwrap();

        let result = handle.await.unwrap().unwrap().unwrap();
        assert!(result.is_allowed());
    }
}
