//! Authentication, Authorization, and Session Ownership for M31A App Server.
//!
//! Enforces request identity, token revocation, scope checking, thread ownership boundaries,
//! and audit logging for sensitive HTTP, SSE, and RPC endpoints.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthScope {
    Read,
    Write,
    Admin,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientIdentity {
    pub client_id: String,
    pub label: String,
    pub scopes: HashSet<AuthScope>,
}

impl ClientIdentity {
    pub fn has_scope(&self, scope: AuthScope) -> bool {
        self.scopes.contains(&scope) || self.scopes.contains(&AuthScope::Admin)
    }

    pub fn is_admin(&self) -> bool {
        self.scopes.contains(&AuthScope::Admin)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub token: String,
    pub identity: ClientIdentity,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthAuditEvent {
    pub timestamp: DateTime<Utc>,
    pub client_id: Option<String>,
    pub action: String,
    pub success: bool,
    pub details: String,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error(
        "Missing authentication credentials: HTTP Authorization header or token query parameter required"
    )]
    MissingCredentials,
    #[error("Invalid token provided")]
    InvalidToken,
    #[error("Authentication token has been revoked")]
    TokenRevoked,
    #[error("Authentication token has expired")]
    TokenExpired,
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Access denied: thread '{thread_id}' is owned by another client")]
    ThreadAccessDenied {
        thread_id: String,
        client_id: String,
    },
}

#[derive(Debug)]
pub struct AuthManager {
    tokens: Mutex<HashMap<String, TokenInfo>>,
    thread_owners: Mutex<HashMap<String, String>>, // thread_id -> client_id
    audit_log: Mutex<Vec<AuthAuditEvent>>,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            thread_owners: Mutex::new(HashMap::new()),
            audit_log: Mutex::new(Vec::new()),
        }
    }

    pub fn with_default_token(
        token: &str,
        client_id: &str,
        label: &str,
        scopes: Vec<AuthScope>,
    ) -> Self {
        let manager = Self::new();
        manager.register_token(token, client_id, label, scopes, None);
        manager
    }

    pub fn register_token(
        &self,
        token: &str,
        client_id: &str,
        label: &str,
        scopes: Vec<AuthScope>,
        ttl_secs: Option<u64>,
    ) -> String {
        let expires_at = ttl_secs.map(|s| Utc::now() + Duration::seconds(s as i64));
        let info = TokenInfo {
            token: token.to_string(),
            identity: ClientIdentity {
                client_id: client_id.to_string(),
                label: label.to_string(),
                scopes: scopes.into_iter().collect(),
            },
            created_at: Utc::now(),
            expires_at,
            revoked: false,
        };

        self.tokens.lock().unwrap().insert(token.to_string(), info);

        self.record_audit(
            Some(client_id),
            "token_created",
            true,
            &format!(
                "Created token for client '{}' with label '{}'",
                client_id, label
            ),
        );

        token.to_string()
    }

    pub fn generate_token(
        &self,
        client_id: &str,
        label: &str,
        scopes: Vec<AuthScope>,
        ttl_secs: Option<u64>,
    ) -> String {
        let token = format!("m31a_tok_{}", uuid::Uuid::new_v4().simple());
        self.register_token(&token, client_id, label, scopes, ttl_secs);
        token
    }

    pub fn validate_token(&self, token: &str) -> Result<ClientIdentity, AuthError> {
        let mut tokens = self.tokens.lock().unwrap();
        let info = match tokens.get_mut(token) {
            Some(i) => i,
            None => {
                self.record_audit(None, "token_validation", false, "Token not found");
                return Err(AuthError::InvalidToken);
            }
        };

        if info.revoked {
            self.record_audit(
                Some(&info.identity.client_id),
                "token_validation",
                false,
                "Token is revoked",
            );
            return Err(AuthError::TokenRevoked);
        }

        if let Some(exp) = info.expires_at
            && Utc::now() > exp
        {
            self.record_audit(
                Some(&info.identity.client_id),
                "token_validation",
                false,
                "Token expired",
            );
            return Err(AuthError::TokenExpired);
        }

        Ok(info.identity.clone())
    }

    pub fn revoke_token(&self, token: &str) -> bool {
        let mut tokens = self.tokens.lock().unwrap();
        if let Some(info) = tokens.get_mut(token) {
            info.revoked = true;
            let client_id = info.identity.client_id.clone();
            drop(tokens);
            self.record_audit(
                Some(&client_id),
                "token_revoked",
                true,
                "Token revoked successfully",
            );
            true
        } else {
            false
        }
    }

    pub fn rotate_token(
        &self,
        old_token: &str,
        ttl_secs: Option<u64>,
    ) -> Result<String, AuthError> {
        let identity = self.validate_token(old_token)?;
        self.revoke_token(old_token);
        let new_token = format!("m31a_tok_{}", uuid::Uuid::new_v4().simple());
        self.register_token(
            &new_token,
            &identity.client_id,
            &identity.label,
            identity.scopes.into_iter().collect(),
            ttl_secs,
        );
        self.record_audit(
            Some(&identity.client_id),
            "token_rotated",
            true,
            "Rotated authentication token",
        );
        Ok(new_token)
    }

    pub fn register_thread_owner(&self, thread_id: &str, client_id: &str) {
        self.thread_owners
            .lock()
            .unwrap()
            .insert(thread_id.to_string(), client_id.to_string());
        self.record_audit(
            Some(client_id),
            "thread_ownership_registered",
            true,
            &format!("Client '{}' claimed thread '{}'", client_id, thread_id),
        );
    }

    pub fn get_thread_owner(&self, thread_id: &str) -> Option<String> {
        self.thread_owners.lock().unwrap().get(thread_id).cloned()
    }

    pub fn is_thread_owner(&self, thread_id: &str, client_id: &str) -> bool {
        let owners = self.thread_owners.lock().unwrap();
        match owners.get(thread_id) {
            Some(owner) => owner == client_id,
            None => true, // Legacy or unowned threads can be accessed if no ownership assigned
        }
    }

    pub fn require_thread_access(
        &self,
        thread_id: &str,
        identity: &ClientIdentity,
    ) -> Result<(), AuthError> {
        if identity.is_admin() {
            return Ok(());
        }

        let owners = self.thread_owners.lock().unwrap();
        if let Some(owner) = owners.get(thread_id)
            && owner != &identity.client_id
        {
            self.record_audit(
                Some(&identity.client_id),
                "thread_access_denied",
                false,
                &format!(
                    "Client '{}' denied access to thread '{}' owned by '{}'",
                    identity.client_id, thread_id, owner
                ),
            );
            return Err(AuthError::ThreadAccessDenied {
                thread_id: thread_id.to_string(),
                client_id: identity.client_id.clone(),
            });
        }
        Ok(())
    }

    pub fn record_audit(
        &self,
        client_id: Option<&str>,
        action: &str,
        success: bool,
        details: &str,
    ) {
        let mut log = self.audit_log.lock().unwrap();
        log.push(AuthAuditEvent {
            timestamp: Utc::now(),
            client_id: client_id.map(|s| s.to_string()),
            action: action.to_string(),
            success,
            details: details.to_string(),
        });
        if log.len() > 1000 {
            let excess = log.len() - 1000;
            log.drain(0..excess);
        }
    }

    pub fn audit_events(&self) -> Vec<AuthAuditEvent> {
        self.audit_log.lock().unwrap().clone()
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}
