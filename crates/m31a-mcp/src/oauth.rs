//! OAuth 2.0 PKCE authentication and user consent elicitation.

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::{RwLock, oneshot};
use uuid::Uuid;

/// OAuth 2.0 PKCE configuration for MCP servers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub redirect_uri: String,
    pub pkce: bool,
}

/// Network egress policy governing an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNetworkPolicy {
    pub allowed_domains: Vec<String>,
    pub blocked_domains: Vec<String>,
    pub require_egress_approval: bool,
    pub allowed_ports: Vec<u16>,
    pub blocked_ports: Vec<u16>,
}

impl Default for McpNetworkPolicy {
    fn default() -> Self {
        Self {
            allowed_domains: vec![],
            blocked_domains: vec![],
            require_egress_approval: true,
            allowed_ports: vec![],
            blocked_ports: vec![22, 23, 25, 135, 139, 445, 3389],
        }
    }
}

/// Elicitation request
pub struct ElicitationRequest {
    pub id: String,
    pub server_name: String,
    pub request_type: ElicitationType,
    pub prompt: String,
    pub options: Vec<String>,
    pub response_tx: oneshot::Sender<ElicitationResponse>,
}

/// Elicitation types
#[derive(Debug, Clone)]
pub enum ElicitationType {
    OAuthConsent {
        auth_url: String,
    },
    Permission {
        tool: String,
        action: String,
    },
    NetworkAccess {
        domain: String,
        port: u16,
    },
    FileAccess {
        path: String,
        access_type: FileAccessType,
    },
}

/// File access types
#[derive(Debug, Clone)]
pub enum FileAccessType {
    Read,
    Write,
    Execute,
}

/// Elicitation response
#[derive(Debug, Clone)]
pub enum ElicitationResponse {
    Approved { token: Option<String> },
    Denied,
    Custom(String),
}

/// Manages interactive authorization and consent requests for MCP servers.
pub struct ElicitationReviewer {
    pending: RwLock<HashMap<String, oneshot::Sender<ElicitationResponse>>>,
    auto_approve: bool,
}

impl Default for ElicitationReviewer {
    fn default() -> Self {
        Self::new(true)
    }
}

impl ElicitationReviewer {
    pub fn new(auto_approve: bool) -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
            auto_approve,
        }
    }

    /// Submits a user decision for a pending elicitation request.
    pub async fn submit_response(&self, id: &str, response: ElicitationResponse) -> bool {
        if let Some(tx) = self.pending.write().await.remove(id) {
            tx.send(response).is_ok()
        } else {
            false
        }
    }

    /// Request OAuth consent
    pub async fn elicit_oauth(&self, oauth: &McpOAuthConfig, server_name: &str) -> Result<String> {
        let state = Uuid::new_v4().to_string();
        let pkce_verifier = if oauth.pkce {
            Some(Self::generate_pkce_verifier())
        } else {
            None
        };

        let _auth_url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}{}",
            oauth.auth_url,
            urlencoding::encode(&oauth.client_id),
            urlencoding::encode(&oauth.redirect_uri),
            urlencoding::encode(&oauth.scopes.join(" ")),
            urlencoding::encode(&state),
            if let Some(ref verifier) = pkce_verifier {
                format!(
                    "&code_challenge={}&code_challenge_method=S256",
                    urlencoding::encode(&Self::pkce_challenge(verifier))
                )
            } else {
                String::new()
            }
        );

        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        if self.auto_approve {
            let _ = tx.send(ElicitationResponse::Approved { token: None });
        } else {
            self.pending.write().await.insert(id.clone(), tx);
        }

        let response = rx.await.context("Elicitation channel closed")?;

        match response {
            ElicitationResponse::Approved { token } => {
                if let Some(token) = token {
                    Ok(token)
                } else {
                    self.exchange_code_for_token(oauth, &state, pkce_verifier)
                        .await
                }
            }
            ElicitationResponse::Denied => {
                Err(anyhow::anyhow!("OAuth consent denied for {server_name}"))
            }
            ElicitationResponse::Custom(_) => {
                Err(anyhow::anyhow!("Unexpected custom OAuth response"))
            }
        }
    }

    /// Request permission for a tool action
    pub async fn elicit_permission(
        &self,
        _server: &str,
        _tool: &str,
        _action: &str,
    ) -> Result<bool> {
        if self.auto_approve {
            return Ok(true);
        }

        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.write().await.insert(id, tx);

        match rx.await.context("Elicitation channel closed")? {
            ElicitationResponse::Approved { .. } => Ok(true),
            _ => Ok(false),
        }
    }

    /// Request network access
    pub async fn elicit_network_access(
        &self,
        _server: &str,
        _domain: &str,
        _port: u16,
    ) -> Result<bool> {
        if self.auto_approve {
            return Ok(true);
        }

        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.write().await.insert(id, tx);

        match rx.await.context("Elicitation channel closed")? {
            ElicitationResponse::Approved { .. } => Ok(true),
            _ => Ok(false),
        }
    }

    fn generate_pkce_verifier() -> String {
        use rand::Rng;
        const CHARSET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        let mut rng = rand::thread_rng();
        (0..128)
            .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
            .collect()
    }

    fn pkce_challenge(verifier: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let result = hasher.finalize();
        base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(result)
    }

    async fn exchange_code_for_token(
        &self,
        oauth: &McpOAuthConfig,
        code: &str,
        pkce_verifier: Option<String>,
    ) -> Result<String> {
        let client = reqwest::Client::new();

        let mut params = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", code.to_string()),
            ("redirect_uri", oauth.redirect_uri.clone()),
            ("client_id", oauth.client_id.clone()),
            ("client_secret", oauth.client_secret.clone()),
        ];

        if let Some(verifier) = pkce_verifier {
            params.push(("code_verifier", verifier));
        }

        let response = client.post(&oauth.token_url).form(&params).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Token exchange failed with status: {}",
                response.status()
            ));
        }

        let token_response: serde_json::Value = response.json().await?;
        token_response["access_token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No access_token found in OAuth response payload"))
    }
}
