//! Authentication, Authorization, and Session Ownership for M31A App Server.
//!
//! Enforces request identity, token revocation, scope checking, thread ownership boundaries,
//! and audit logging for sensitive HTTP, SSE, and RPC endpoints.

use chrono::{DateTime, Duration, Utc};
use m31a_storage::Storage;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use thiserror::Error;

/// Scopes of authority in the App Server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthScope {
    Read,
    Write,
    Admin,
    Execute,
}

/// Validated identity of a connecting client.
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

    /// Explicit trusted-local execution context for internal IPC/stdio/UDS transports.
    pub fn trusted_local() -> Self {
        Self {
            client_id: "trusted_local".to_string(),
            label: "Trusted Local Transport".to_string(),
            scopes: [
                AuthScope::Read,
                AuthScope::Write,
                AuthScope::Admin,
                AuthScope::Execute,
            ]
            .into_iter()
            .collect(),
        }
    }
}

/// Declarative authorization policy for an RPC method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodPolicy {
    pub required_scope: AuthScope,
    pub requires_thread_ownership: bool,
    pub mutates_workspace: bool,
    pub external_side_effects: bool,
    pub is_public: bool,
}

/// Registry mapping registered RPC method names to their required authorization policies.
pub fn get_method_policy(method: &str) -> Option<MethodPolicy> {
    match method {
        // Public initialization
        "initialize" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: true,
        }),

        // Diagnostics & read-only catalog introspection
        "server/diagnostics" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "models/list" | "model/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "skills/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "instructions/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "tools/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "config/read" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Thread lifecycle operations
        "thread/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/read" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/create" | "thread/start" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: false,
            mutates_workspace: true,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/archive" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/unarchive" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/delete" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/fork" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/resume" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/section/create" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/section/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/section/assign" | "thread/section/move" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Turn & Execution operations
        "turn/start" => Some(MethodPolicy {
            required_scope: AuthScope::Execute,
            requires_thread_ownership: true,
            mutates_workspace: true,
            external_side_effects: true,
            is_public: false,
        }),
        "turn/steer" => Some(MethodPolicy {
            required_scope: AuthScope::Execute,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "turn/interrupt" => Some(MethodPolicy {
            required_scope: AuthScope::Execute,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "turn/approve" | "turn/decision" | "approval/respond" => Some(MethodPolicy {
            required_scope: AuthScope::Execute,
            requires_thread_ownership: true,
            mutates_workspace: true,
            external_side_effects: true,
            is_public: false,
        }),
        "turn/deny" => Some(MethodPolicy {
            required_scope: AuthScope::Execute,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "turn/answer" | "question/respond" => Some(MethodPolicy {
            required_scope: AuthScope::Execute,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Checkpoints & Session Management
        "checkpoint/create" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "checkpoint/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "checkpoint/restore" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: true,
            external_side_effects: false,
            is_public: false,
        }),
        "session/compact" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Path Locks & Board
        "locks/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "locks/acquire" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: false,
            mutates_workspace: true,
            external_side_effects: false,
            is_public: false,
        }),
        "locks/release" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: false,
            mutates_workspace: true,
            external_side_effects: false,
            is_public: false,
        }),
        "board/post" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: true,
            is_public: false,
        }),
        "board/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Goals, Inbox, and Claims
        "goal/set" | "session/goal/set" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "goal/get" | "session/goal/get" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "goal/clear" | "session/goal/clear" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "goal/update" | "session/goal/update" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "inbox/enqueue" | "session/inbox/enqueue" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "inbox/dequeue" | "session/inbox/dequeue" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "inbox/list" | "session/inbox/list" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "claims/active" | "session/claims/active" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "claims/recover" | "session/claims/recover" => Some(MethodPolicy {
            required_scope: AuthScope::Admin,
            requires_thread_ownership: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Administrative & Security Operations
        "workspace/trust_request" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "workspace/trust_grant" => Some(MethodPolicy {
            required_scope: AuthScope::Admin,
            requires_thread_ownership: false,
            mutates_workspace: true,
            external_side_effects: false,
            is_public: false,
        }),
        "auth/stream_ticket" => Some(MethodPolicy {
            required_scope: AuthScope::Read,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "auth/rotate_token" => Some(MethodPolicy {
            required_scope: AuthScope::Write,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "auth/revoke_token" => Some(MethodPolicy {
            required_scope: AuthScope::Admin,
            requires_thread_ownership: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        _ => None,
    }
}

/// Constant-time byte-slice comparison to prevent timing side channels.
pub fn constant_time_compare(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub token_hash: String,
    pub identity: ClientIdentity,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamTicket {
    pub ticket: String,
    pub identity: ClientIdentity,
    pub thread_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
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
    #[error("Missing authentication credentials: HTTP Authorization header required")]
    MissingCredentials,
    #[error("Invalid token provided")]
    InvalidToken,
    #[error("Authentication token has been revoked")]
    TokenRevoked,
    #[error("Authentication token has expired")]
    TokenExpired,
    #[error("Permission denied: required scope '{required:?}', caller has {actual:?}")]
    ScopeDenied {
        required: AuthScope,
        actual: Vec<AuthScope>,
    },
    #[error("Access denied: thread '{thread_id}' is owned by another client")]
    ThreadAccessDenied {
        thread_id: String,
        client_id: String,
    },
    #[error("Access denied: thread '{thread_id}' has no recorded owner (admin required)")]
    LegacyThreadRestricted { thread_id: String },
    #[error("Access denied: unknown or unclassified RPC method '{0}'")]
    UnknownMethod(String),
    #[error("Access denied: stream ticket invalid, expired, or already used")]
    InvalidStreamTicket,
    #[error("Storage error during authorization check: {0}")]
    StorageError(String),
}

impl AuthError {
    pub fn jsonrpc_code(&self) -> i64 {
        -32003
    }
}

pub struct AuthManager {
    tokens: Mutex<HashMap<String, TokenInfo>>, // token_hash -> TokenInfo
    tickets: Mutex<HashMap<String, StreamTicket>>, // ticket -> StreamTicket
    storage: Mutex<Option<Storage>>,
    in_memory_owners: Mutex<HashMap<String, String>>, // fallback if storage not yet attached
    audit_log: Mutex<Vec<AuthAuditEvent>>,
}

impl std::fmt::Debug for AuthManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthManager")
            .field("tokens_count", &self.tokens.lock().unwrap().len())
            .field("tickets_count", &self.tickets.lock().unwrap().len())
            .field("audit_log_count", &self.audit_log.lock().unwrap().len())
            .finish()
    }
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            tickets: Mutex::new(HashMap::new()),
            storage: Mutex::new(None),
            in_memory_owners: Mutex::new(HashMap::new()),
            audit_log: Mutex::new(Vec::new()),
        }
    }

    pub fn with_storage(storage: Storage) -> Self {
        let mgr = Self::new();
        *mgr.storage.lock().unwrap() = Some(storage);
        mgr
    }

    pub fn set_storage(&self, storage: Storage) {
        *self.storage.lock().unwrap() = Some(storage);
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

    /// Registers a token with SHA-256 hashed storage.
    pub fn register_token(
        &self,
        token: &str,
        client_id: &str,
        label: &str,
        scopes: Vec<AuthScope>,
        ttl_secs: Option<u64>,
    ) -> String {
        let token_hash = hash_token(token);
        let expires_at = ttl_secs.map(|s| Utc::now() + Duration::seconds(s as i64));
        let info = TokenInfo {
            token_hash: token_hash.clone(),
            identity: ClientIdentity {
                client_id: client_id.to_string(),
                label: label.to_string(),
                scopes: scopes.into_iter().collect(),
            },
            created_at: Utc::now(),
            expires_at,
            revoked: false,
        };

        self.tokens.lock().unwrap().insert(token_hash, info);

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

    /// Generates a high-entropy cryptographically secure token.
    pub fn generate_token(
        &self,
        client_id: &str,
        label: &str,
        scopes: Vec<AuthScope>,
        ttl_secs: Option<u64>,
    ) -> String {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        let mut token = String::with_capacity(73);
        token.push_str("m31a_tok_");
        for b in &bytes {
            use std::fmt::Write;
            let _ = write!(token, "{:02x}", b);
        }
        self.register_token(&token, client_id, label, scopes, ttl_secs);
        token
    }

    /// Validates a bearer token in constant time using SHA-256 hash comparison.
    pub fn validate_token(&self, token: &str) -> Result<ClientIdentity, AuthError> {
        let provided_hash = hash_token(token);
        let tokens = self.tokens.lock().unwrap();

        let mut matched_info: Option<TokenInfo> = None;
        for (stored_hash, info) in tokens.iter() {
            if constant_time_compare(stored_hash, &provided_hash) {
                matched_info = Some(info.clone());
                break;
            }
        }

        let info = match matched_info {
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

        Ok(info.identity)
    }

    pub fn revoke_token(&self, token: &str) -> bool {
        let token_hash = hash_token(token);
        let mut tokens = self.tokens.lock().unwrap();
        if let Some(info) = tokens.get_mut(&token_hash) {
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
        let new_token = self.generate_token(
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

    /// Issues a single-use scoped stream ticket for browser-based SSE connections.
    pub fn issue_stream_ticket(
        &self,
        identity: &ClientIdentity,
        thread_id: Option<&str>,
        ttl_secs: u64,
    ) -> String {
        let mut bytes = [0u8; 24];
        rand::rng().fill_bytes(&mut bytes);
        let mut ticket = String::with_capacity(57);
        ticket.push_str("m31a_st_");
        for b in &bytes {
            use std::fmt::Write;
            let _ = write!(ticket, "{:02x}", b);
        }

        let now = Utc::now();
        let expires_at = now + Duration::seconds(ttl_secs as i64);

        let stream_ticket = StreamTicket {
            ticket: ticket.clone(),
            identity: identity.clone(),
            thread_id: thread_id.map(|s| s.to_string()),
            created_at: now,
            expires_at,
            used: false,
        };

        self.tickets
            .lock()
            .unwrap()
            .insert(ticket.clone(), stream_ticket);
        ticket
    }

    /// Consumes a single-use stream ticket. Fails if invalid, expired, or previously consumed.
    pub fn consume_stream_ticket(&self, ticket: &str) -> Result<ClientIdentity, AuthError> {
        let mut tickets = self.tickets.lock().unwrap();
        let item = tickets
            .get_mut(ticket)
            .ok_or(AuthError::InvalidStreamTicket)?;

        if item.used {
            return Err(AuthError::InvalidStreamTicket);
        }

        if Utc::now() > item.expires_at {
            return Err(AuthError::InvalidStreamTicket);
        }

        item.used = true;
        Ok(item.identity.clone())
    }

    /// Authoritative, persistent registration of thread ownership in SQLite.
    pub fn register_thread_owner(&self, thread_id: &str, client_id: &str) {
        self.in_memory_owners
            .lock()
            .unwrap()
            .insert(thread_id.to_string(), client_id.to_string());

        let persist_err = self
            .storage
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|storage| storage.set_thread_owner(thread_id, client_id).err());
        if let Some(e) = persist_err {
            tracing::error!("Failed persisting thread ownership in SQLite: {}", e);
        }

        self.record_audit(
            Some(client_id),
            "thread_ownership_registered",
            true,
            &format!(
                "Client '{}' registered ownership of thread '{}'",
                client_id, thread_id
            ),
        );
    }

    pub fn get_thread_owner(&self, thread_id: &str) -> Option<String> {
        let stored_owner = self
            .storage
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|storage| storage.get_thread_owner(thread_id).ok().flatten());
        if let Some(owner) = stored_owner {
            return Some(owner);
        }
        self.in_memory_owners
            .lock()
            .unwrap()
            .get(thread_id)
            .cloned()
    }

    pub fn is_thread_owner(&self, thread_id: &str, client_id: &str) -> bool {
        match self.get_thread_owner(thread_id) {
            Some(owner) => owner == client_id,
            None => false, // Unowned/legacy threads fail closed: not granted by default
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

        match self.get_thread_owner(thread_id) {
            Some(owner) => {
                if owner == identity.client_id {
                    Ok(())
                } else {
                    self.record_audit(
                        Some(&identity.client_id),
                        "thread_access_denied",
                        false,
                        &format!(
                            "Client '{}' denied access to thread '{}' owned by '{}'",
                            identity.client_id, thread_id, owner
                        ),
                    );
                    Err(AuthError::ThreadAccessDenied {
                        thread_id: thread_id.to_string(),
                        client_id: identity.client_id.clone(),
                    })
                }
            }
            None => {
                self.record_audit(
                    Some(&identity.client_id),
                    "legacy_thread_restricted",
                    false,
                    &format!(
                        "Client '{}' denied access to legacy/unowned thread '{}' (admin required)",
                        identity.client_id, thread_id
                    ),
                );
                Err(AuthError::LegacyThreadRestricted {
                    thread_id: thread_id.to_string(),
                })
            }
        }
    }

    /// Centralized RPC authorization enforcement.
    ///
    /// Evaluates requested RPC method against the declared `MethodPolicy`.
    /// Enforces scope requirements, thread ownership, and fails closed on unknown or unclassified methods.
    pub fn authorize_rpc(
        &self,
        method: &str,
        params: &serde_json::Value,
        identity: Option<&ClientIdentity>,
    ) -> Result<(), AuthError> {
        let policy = match get_method_policy(method) {
            Some(p) => p,
            None => {
                self.record_audit(
                    identity.map(|i| i.client_id.as_str()),
                    "rpc_authorization_failed",
                    false,
                    &format!("Unknown RPC method '{}'", method),
                );
                return Err(AuthError::UnknownMethod(method.to_string()));
            }
        };

        if policy.is_public {
            return Ok(());
        }

        let identity = match identity {
            Some(i) => i,
            None => {
                self.record_audit(
                    None,
                    "rpc_authorization_failed",
                    false,
                    &format!("Missing credentials for method '{}'", method),
                );
                return Err(AuthError::MissingCredentials);
            }
        };

        if !identity.has_scope(policy.required_scope) {
            self.record_audit(
                Some(&identity.client_id),
                "rpc_scope_denied",
                false,
                &format!(
                    "Required scope '{:?}' not satisfied by client '{}' (has {:?}) for method '{}'",
                    policy.required_scope, identity.client_id, identity.scopes, method
                ),
            );
            return Err(AuthError::ScopeDenied {
                required: policy.required_scope,
                actual: identity.scopes.iter().copied().collect(),
            });
        }

        if policy.requires_thread_ownership {
            let thread_id = params
                .get("thread_id")
                .or_else(|| params.get("id"))
                .or_else(|| params.get("parent_id"))
                .and_then(|v| v.as_str());
            if let Some(tid) = thread_id {
                self.require_thread_access(tid, identity)?;
            }
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
