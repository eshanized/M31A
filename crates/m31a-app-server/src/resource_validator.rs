//! Resource Validation, Method Allowlisting, and RPC Integrity for M31A App Server.
//!
//! Enforces method allowlisting, parameter validation, thread/turn/checkpoint existence
//! and correlation, and diagnostic audit logging in a local-first, single-user environment.
//!
//! ## Local-First Security Boundary Threat Model
//! M31A is a local-first, single-user autonomous coding agent. It does not implement
//! first-party user authentication tokens (no accounts, registration, login/logout,
//! or bearer tokens). Instead, local single-user security is enforced via:
//! - OS-level user boundaries and process isolation.
//! - Private filesystem permissions (mode 0700 for directories, 0600 for socket/vault files).
//! - Loopback-only network binding (127.0.0.1 and ::1) to prevent remote unauthenticated access.
//! - Strict Host header validation to prevent DNS rebinding attacks.
//! - Strict Origin header validation to prevent cross-origin drive-by attacks from browsers.
//! - Explicit workspace trust policies to prevent arbitrary directory access.
//! - Host-mediated human approval for privileged operations.
//! - Sandboxed execution policies.

use chrono::{DateTime, Utc};
use m31a_storage::Storage;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use thiserror::Error;

/// Declarative metadata for a registered RPC method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodMetadata {
    pub requires_target_resource: bool,
    pub mutates_workspace: bool,
    pub external_side_effects: bool,
    pub is_public: bool,
}

/// Registry mapping registered RPC method names to their validation metadata.
pub fn get_method_metadata(method: &str) -> Option<MethodMetadata> {
    match method {
        // Public initialization
        "initialize" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: true,
        }),

        // Diagnostics & read-only catalog introspection
        "server/diagnostics" | "models/list" | "model/list" | "skills/list"
        | "instructions/list" | "tools/list" | "config/read" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Thread lifecycle operations
        "thread/list" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/read" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/create" | "thread/start" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: true,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/archive" | "thread/unarchive" | "thread/delete" | "thread/fork"
        | "thread/resume" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/section/create" | "thread/section/list" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "thread/section/assign" | "thread/section/move" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Turn & Execution operations
        "turn/start" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: true,
            external_side_effects: true,
            is_public: false,
        }),
        "turn/steer" | "turn/interrupt" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "turn/approve" | "turn/decision" | "approval/respond" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: true,
            external_side_effects: true,
            is_public: false,
        }),
        "turn/deny" | "turn/answer" | "question/respond" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Checkpoints & Session Management
        "checkpoint/create" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "checkpoint/list" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "checkpoint/restore" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: true,
            external_side_effects: false,
            is_public: false,
        }),
        "session/compact" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Path Locks & Board
        "locks/list" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "locks/acquire" | "locks/release" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: true,
            external_side_effects: false,
            is_public: false,
        }),
        "board/post" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: false,
            external_side_effects: true,
            is_public: false,
        }),
        "board/list" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),

        // Goals, Inbox, and Claims
        "goal/set"
        | "session/goal/set"
        | "goal/get"
        | "session/goal/get"
        | "goal/clear"
        | "session/goal/clear"
        | "goal/update"
        | "session/goal/update" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "inbox/enqueue"
        | "session/inbox/enqueue"
        | "inbox/dequeue"
        | "session/inbox/dequeue"
        | "inbox/list"
        | "session/inbox/list" => Some(MethodMetadata {
            requires_target_resource: true,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "claims/active" | "session/claims/active" | "claims/recover" | "session/claims/recover" => {
            Some(MethodMetadata {
                requires_target_resource: true,
                mutates_workspace: false,
                external_side_effects: false,
                is_public: false,
            })
        }

        // Workspace Trust Management
        "workspace/trust_request" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: false,
            external_side_effects: false,
            is_public: false,
        }),
        "workspace/trust_grant" => Some(MethodMetadata {
            requires_target_resource: false,
            mutates_workspace: true,
            external_side_effects: false,
            is_public: false,
        }),

        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub action: String,
    pub success: bool,
    pub details: String,
}

#[derive(Debug, Error)]
pub enum RpcValidationError {
    #[error("Unknown or unclassified RPC method '{0}'")]
    UnknownMethod(String),
    #[error("Missing target resource identifier for method '{0}'")]
    MissingTargetIdentifier(String),
    #[error("Conflicting resource identifiers: expected thread '{expected}', got '{actual}'")]
    ConflictingResourceIdentifiers { expected: String, actual: String },
    #[error("Target resource not found: {0}")]
    ResourceNotFound(String),
    #[error("Storage error during resource validation: {0}")]
    StorageError(String),
    #[error("Invalid request parameters: {0}")]
    InvalidParameters(String),
}

impl RpcValidationError {
    pub fn jsonrpc_code(&self) -> i64 {
        -32003
    }
}

/// Local-first application resource validator and audit logger.
///
/// Enforces method allowlisting, resource existence checks, thread-to-checkpoint correlation,
/// turn resolution, and comprehensive audit trails without first-party user authentication tokens.
pub struct ResourceValidator {
    storage: Mutex<Option<Storage>>,
    audit_log: Mutex<Vec<AuditEvent>>,
}

impl std::fmt::Debug for ResourceValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceValidator")
            .field("audit_log_count", &self.audit_log.lock().unwrap().len())
            .finish()
    }
}

impl Default for ResourceValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceValidator {
    pub fn new() -> Self {
        Self {
            storage: Mutex::new(None),
            audit_log: Mutex::new(Vec::new()),
        }
    }

    pub fn with_storage(storage: Storage) -> Self {
        let val = Self::new();
        *val.storage.lock().unwrap() = Some(storage);
        val
    }

    pub fn set_storage(&self, storage: Storage) {
        *self.storage.lock().unwrap() = Some(storage);
    }

    /// Centralized RPC method and resource integrity validation before dispatch.
    ///
    /// Evaluates requested RPC method against the declared `MethodMetadata`.
    /// Validates required resource identifiers, correlations, and existence checks.
    pub fn validate_rpc(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<(), RpcValidationError> {
        let meta = match get_method_metadata(method) {
            Some(m) => m,
            None => {
                self.record_audit(
                    "rpc_validation_failed",
                    false,
                    &format!("Unknown RPC method '{}'", method),
                );
                return Err(RpcValidationError::UnknownMethod(method.to_string()));
            }
        };

        if meta.is_public {
            return Ok(());
        }

        if meta.requires_target_resource {
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
                                return Err(RpcValidationError::ConflictingResourceIdentifiers {
                                    expected: cp.thread_id,
                                    actual: explicit_tid.clone(),
                                });
                            }
                            candidate_thread_id = Some(cp.thread_id);
                        }
                        Ok(None) => {
                            return Err(RpcValidationError::ResourceNotFound(format!(
                                "Checkpoint '{cp_id}' not found"
                            )));
                        }
                        Err(e) => {
                            return Err(RpcValidationError::StorageError(e.to_string()));
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
                                return Err(RpcValidationError::ConflictingResourceIdentifiers {
                                    expected: turn.thread_id,
                                    actual: explicit_tid.clone(),
                                });
                            }
                            candidate_thread_id = Some(turn.thread_id);
                        }
                        Ok(None) => {
                            return Err(RpcValidationError::ResourceNotFound(format!(
                                "Turn '{turn_id}' not found"
                            )));
                        }
                        Err(e) => {
                            return Err(RpcValidationError::StorageError(e.to_string()));
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
                                return Err(RpcValidationError::ResourceNotFound(format!(
                                    "Thread '{tid}' not found"
                                )));
                            }
                        }
                        Err(e) => {
                            return Err(RpcValidationError::StorageError(e.to_string()));
                        }
                    }
                }
            } else {
                let has_resource_id = params.get("turn_id").is_some()
                    || params.get("approval_id").is_some()
                    || params.get("question_id").is_some();
                if !has_resource_id {
                    self.record_audit(
                        "rpc_validation_failed",
                        false,
                        &format!("Missing target resource identifier for method '{}'", method),
                    );
                    return Err(RpcValidationError::MissingTargetIdentifier(
                        method.to_string(),
                    ));
                }
            }
        }

        self.record_audit(
            "rpc_validated",
            true,
            &format!("Validated local RPC method '{}'", method),
        );
        Ok(())
    }

    pub fn record_audit(&self, action: &str, success: bool, details: &str) {
        let mut log = self.audit_log.lock().unwrap();
        log.push(AuditEvent {
            timestamp: Utc::now(),
            action: action.to_string(),
            success,
            details: details.to_string(),
        });
        if log.len() > 1000 {
            let excess = log.len() - 1000;
            log.drain(0..excess);
        }
    }

    pub fn audit_events(&self) -> Vec<AuditEvent> {
        self.audit_log.lock().unwrap().clone()
    }
}
