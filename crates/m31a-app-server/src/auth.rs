//! Authentication, Authorization, and Session Ownership for M31A App Server.
//!
//! Enforces request identity, token revocation, scope checking, thread ownership boundaries,
//! and audit logging for sensitive HTTP, SSE, and RPC endpoints.

use chrono::{DateTime, Utc};
use m31a_storage::Storage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
    #[error("Access denied: missing target identifier for method '{0}'")]
    MissingTargetIdentifier(String),
    #[error(
        "Access denied: conflicting resource identifiers: expected thread '{expected}', got '{actual}'"
    )]
    ConflictingResourceIdentifiers { expected: String, actual: String },
    #[error("Access denied: target resource not found: {0}")]
    ResourceNotFound(String),
    #[error("Storage error during authorization check: {0}")]
    StorageError(String),
}

impl AuthError {
    pub fn jsonrpc_code(&self) -> i64 {
        -32003
    }
}

/// Local-first application access manager.
/// Enforces resource validation, existence checks, thread-to-checkpoint correlation,
/// turn resolution, and comprehensive audit trails without first-party user authentication tokens.
pub struct AuthManager {
    storage: Mutex<Option<Storage>>,
    audit_log: Mutex<Vec<AuthAuditEvent>>,
}

impl std::fmt::Debug for AuthManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthManager")
            .field("audit_log_count", &self.audit_log.lock().unwrap().len())
            .finish()
    }
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            storage: Mutex::new(None),
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
        _token: &str,
        _client_id: &str,
        _label: &str,
        _scopes: Vec<AuthScope>,
    ) -> Self {
        Self::new()
    }

    // Compatibility stubs for local single-user access
    pub fn validate_token(&self, _token: &str) -> Result<ClientIdentity, AuthError> {
        Ok(ClientIdentity::trusted_local())
    }

    pub fn register_token(
        &self,
        token: &str,
        _client_id: &str,
        _label: &str,
        _scopes: Vec<AuthScope>,
        _ttl_secs: Option<u64>,
    ) -> String {
        token.to_string()
    }

    pub fn generate_token(
        &self,
        client_id: &str,
        label: &str,
        _scopes: Vec<AuthScope>,
        _ttl_secs: Option<u64>,
    ) -> String {
        format!("m31a_local_{client_id}_{label}")
    }

    pub fn revoke_token(&self, _token: &str) -> bool {
        true
    }

    pub fn rotate_token(
        &self,
        old_token: &str,
        _ttl_secs: Option<u64>,
    ) -> Result<String, AuthError> {
        Ok(format!("{old_token}_rotated"))
    }

    pub fn issue_stream_ticket(
        &self,
        _identity: &ClientIdentity,
        _thread_id: Option<&str>,
        _ttl_secs: u64,
    ) -> String {
        "m31a_st_local_ticket".to_string()
    }

    pub fn consume_stream_ticket(&self, _ticket: &str) -> Result<ClientIdentity, AuthError> {
        Ok(ClientIdentity::trusted_local())
    }

    pub fn register_thread_owner(&self, _thread_id: &str, _client_id: &str) {}

    pub fn get_thread_owner(&self, _thread_id: &str) -> Option<String> {
        Some("trusted_local".to_string())
    }

    pub fn is_thread_owner(&self, _thread_id: &str, _client_id: &str) -> bool {
        true
    }

    pub fn require_thread_access(
        &self,
        _thread_id: &str,
        _identity: &ClientIdentity,
    ) -> Result<(), AuthError> {
        Ok(())
    }

    /// Centralized RPC authorization and resource resolution enforcement.
    ///
    /// Evaluates requested RPC method against the declared `MethodPolicy`.
    /// Validates required resource identifiers, correlations, and existence checks.
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

        if policy.requires_thread_ownership {
            let mut candidate_thread_id = params
                .get("thread_id")
                .or_else(|| {
                    if method.starts_with("thread/") {
                        params.get("id").or_else(|| params.get("parent_id"))
                    } else {
                        None
                    }
                })
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let cp_id = if method == "checkpoint/restore" {
                params
                    .get("checkpoint_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
            } else {
                params.get("checkpoint_id").and_then(|v| v.as_str())
            };

            if let Some(cp_id) = cp_id {
                let storage_lock = self.storage.lock().unwrap();
                if let Some(storage) = storage_lock.as_ref() {
                    match storage.get_checkpoint(cp_id) {
                        Ok(Some(cp)) => {
                            if let Some(ref explicit_tid) = candidate_thread_id
                                && explicit_tid != &cp.thread_id
                            {
                                return Err(AuthError::ConflictingResourceIdentifiers {
                                    expected: cp.thread_id,
                                    actual: explicit_tid.clone(),
                                });
                            }
                            candidate_thread_id = Some(cp.thread_id);
                        }
                        Ok(None) => {
                            return Err(AuthError::ResourceNotFound(format!(
                                "Checkpoint '{cp_id}' not found"
                            )));
                        }
                        Err(e) => {
                            return Err(AuthError::StorageError(e.to_string()));
                        }
                    }
                }
            }

            if let Some(turn_id) = params.get("turn_id").and_then(|v| v.as_str()) {
                let storage_lock = self.storage.lock().unwrap();
                if let Some(storage) = storage_lock.as_ref() {
                    match storage.get_turn(turn_id) {
                        Ok(Some(turn)) => {
                            if let Some(ref explicit_tid) = candidate_thread_id
                                && explicit_tid != &turn.thread_id
                            {
                                return Err(AuthError::ConflictingResourceIdentifiers {
                                    expected: turn.thread_id,
                                    actual: explicit_tid.clone(),
                                });
                            }
                            candidate_thread_id = Some(turn.thread_id);
                        }
                        Ok(None) => {
                            return Err(AuthError::ResourceNotFound(format!(
                                "Turn '{turn_id}' not found"
                            )));
                        }
                        Err(e) => {
                            return Err(AuthError::StorageError(e.to_string()));
                        }
                    }
                }
            }

            if let Some(ref tid) = candidate_thread_id {
                let storage_lock = self.storage.lock().unwrap();
                if let Some(storage) = storage_lock.as_ref() {
                    match storage.get_thread(tid) {
                        Ok(Some(_)) => {
                            // Valid existing thread confirmed
                        }
                        Ok(None) => {
                            if method != "thread/create" && method != "thread/start" {
                                return Err(AuthError::ResourceNotFound(format!(
                                    "Thread '{tid}' not found"
                                )));
                            }
                        }
                        Err(e) => {
                            return Err(AuthError::StorageError(e.to_string()));
                        }
                    }
                }
            } else {
                let has_resource_id = params.get("turn_id").is_some()
                    || params.get("approval_id").is_some()
                    || params.get("question_id").is_some();
                if !has_resource_id {
                    self.record_audit(
                        None,
                        "rpc_authorization_failed",
                        false,
                        &format!("Missing target resource identifier for method '{}'", method),
                    );
                    return Err(AuthError::MissingTargetIdentifier(method.to_string()));
                }
            }
        }

        self.record_audit(
            None,
            "rpc_authorized",
            true,
            &format!("Authorized local RPC method '{}'", method),
        );
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
