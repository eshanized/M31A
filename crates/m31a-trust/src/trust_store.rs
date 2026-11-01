//! Persistent workspace trust store and directory boundary evaluator.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::capability::{AuthorizationOutcome, OperationCapability};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Trusted,
    SessionOnly,
    Untrusted,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedProjectRecord {
    pub path: PathBuf,
    pub trust_level: TrustLevel,
    pub trusted_at: DateTime<Utc>,
    pub git_remote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustAuthorization {
    pub workspace_path: PathBuf,
    pub trust_level: TrustLevel,
    pub is_session_scoped: bool,
    pub decision_source: String,
    pub authorized_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustDecision {
    Trusted,
    SessionOnly,
    RequiresConfirmation { path: PathBuf, reason: String },
    Denied { path: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTrustData {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub projects: HashMap<String, TrustedProjectRecord>,
}

fn default_schema_version() -> u32 {
    1
}

impl Default for StoredTrustData {
    fn default() -> Self {
        Self {
            schema_version: 1,
            projects: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrustStore {
    custom_store_path: Option<PathBuf>,
    session_trusts: HashMap<PathBuf, TrustLevel>,
}

impl TrustStore {
    pub fn new() -> Self {
        Self {
            custom_store_path: None,
            session_trusts: HashMap::new(),
        }
    }

    pub fn with_store_path(path: PathBuf) -> Self {
        Self {
            custom_store_path: Some(path),
            session_trusts: HashMap::new(),
        }
    }

    fn store_path(&self) -> PathBuf {
        if let Some(ref p) = self.custom_store_path {
            p.clone()
        } else if let Ok(m31a_home) = std::env::var("M31A_HOME") {
            let path = PathBuf::from(m31a_home);
            if path.ends_with("trusted_directories.json") {
                path
            } else {
                path.join("trusted_directories.json")
            }
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home)
                .join(".m31a")
                .join("trusted_directories.json")
        }
    }

    /// Loads the stored trust data.
    ///
    /// Fails closed with an error on corruption or I/O failure.
    pub fn load_data(&self) -> Result<StoredTrustData> {
        let path = self.store_path();
        if !path.exists() {
            return Ok(StoredTrustData::default());
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read trust store file at '{}'", path.display()))?;
        let data: StoredTrustData = serde_json::from_str(&content)
            .with_context(|| format!("Trust database corrupted at '{}'", path.display()))?;
        Ok(data)
    }

    /// Atomically persists trust data with 0600 permissions and path validation.
    fn save_data(&self, data: &StoredTrustData) -> Result<()> {
        let path = self.store_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
            }
        }

        // Validate all paths in data
        for (p_str, record) in &data.projects {
            let p = Path::new(p_str);
            if !p.is_absolute() {
                anyhow::bail!("Trust store path must be absolute: '{}'", p_str);
            }
            if p != record.path {
                anyhow::bail!(
                    "Trust store key '{}' does not match record path '{}'",
                    p_str,
                    record.path.display()
                );
            }
        }

        let serialized = serde_json::to_string_pretty(data)?;
        let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));

        // Write to temp file and set mode 0600 on unix
        {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(0o600))?;
            }
            file.write_all(serialized.as_bytes())?;
            file.sync_all()?;
        }

        // Atomic replace
        fs::rename(&tmp_path, &path)?;
        Ok(())
    }

    /// Evaluates trust for a given target directory.
    ///
    /// Fails closed: if the underlying trust store file is corrupted or unreadable,
    /// it evaluates strictly as `TrustDecision::Denied`.
    pub fn evaluate(&self, target: &Path) -> TrustDecision {
        let canonical = target
            .canonicalize()
            .unwrap_or_else(|_| target.to_path_buf());

        // 1. Check persistent trust records first for Denied status:
        // A Denied workspace takes absolute precedence over session grants.
        let data = match self.load_data() {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(
                    "Trust store read/corruption failure: {:#}. Failing closed with Denied for '{}'.",
                    e,
                    canonical.display()
                );
                return TrustDecision::Denied { path: canonical };
            }
        };

        for (p_str, record) in &data.projects {
            let stored_path = PathBuf::from(p_str);
            if canonical.starts_with(&stored_path) && record.trust_level == TrustLevel::Denied {
                return TrustDecision::Denied { path: canonical };
            }
        }

        // 2. Check in-memory session trust overrides
        for (session_path, level) in &self.session_trusts {
            if canonical.starts_with(session_path) {
                return match level {
                    TrustLevel::Trusted | TrustLevel::SessionOnly => TrustDecision::SessionOnly,
                    TrustLevel::Denied => TrustDecision::Denied { path: canonical },
                    TrustLevel::Untrusted => TrustDecision::RequiresConfirmation {
                        path: canonical,
                        reason: "Explicitly marked untrusted for session.".into(),
                    },
                };
            }
        }

        // 3. Check persistent trust records
        for (p_str, record) in &data.projects {
            let stored_path = PathBuf::from(p_str);
            if canonical.starts_with(&stored_path) {
                return match record.trust_level {
                    TrustLevel::Trusted => TrustDecision::Trusted,
                    TrustLevel::SessionOnly => TrustDecision::SessionOnly,
                    TrustLevel::Denied => TrustDecision::Denied { path: canonical },
                    TrustLevel::Untrusted => TrustDecision::RequiresConfirmation {
                        path: canonical,
                        reason: "Repository was previously marked untrusted.".into(),
                    },
                };
            }
        }

        // 4. Fallback: Unrecognized directory requires explicit confirmation
        TrustDecision::RequiresConfirmation {
            path: canonical,
            reason: "First-time execution in this directory.".into(),
        }
    }

    /// Central authorization evaluator for all execution entry points (CLI, TUI, app server, subagents).
    ///
    /// Evaluates trust and enforces fail-closed behavior for headless or non-interactive execution:
    /// - Denied workspaces are ALWAYS rejected.
    /// - Trusted and SessionOnly workspaces proceed with valid authorization.
    /// - RequiresConfirmation in headless/non-interactive mode FAILS CLOSED unless explicit session authorization is provided.
    pub fn authorize(
        &mut self,
        target: &Path,
        is_headless: bool,
        allow_untrusted_flag: bool,
    ) -> Result<TrustAuthorization> {
        let canonical = target
            .canonicalize()
            .unwrap_or_else(|_| target.to_path_buf());
        let decision = self.evaluate(&canonical);

        match decision {
            TrustDecision::Denied { path } => {
                anyhow::bail!(
                    "Execution DENIED: Working directory '{}' is explicitly marked as Denied by security trust policy.",
                    path.display()
                );
            }
            TrustDecision::Trusted => Ok(TrustAuthorization {
                workspace_path: canonical,
                trust_level: TrustLevel::Trusted,
                is_session_scoped: false,
                decision_source: "persistent_store".to_string(),
                authorized_at: Utc::now(),
            }),
            TrustDecision::SessionOnly => Ok(TrustAuthorization {
                workspace_path: canonical,
                trust_level: TrustLevel::SessionOnly,
                is_session_scoped: true,
                decision_source: "session_override".to_string(),
                authorized_at: Utc::now(),
            }),
            TrustDecision::RequiresConfirmation { path, reason } => {
                if allow_untrusted_flag {
                    // Explicit session-scoped grant
                    self.session_trusts
                        .insert(canonical.clone(), TrustLevel::SessionOnly);
                    Ok(TrustAuthorization {
                        workspace_path: canonical,
                        trust_level: TrustLevel::SessionOnly,
                        is_session_scoped: true,
                        decision_source: "explicit_session_authorization".to_string(),
                        authorized_at: Utc::now(),
                    })
                } else if is_headless {
                    anyhow::bail!(
                        "Execution REJECTED: Working directory '{}' is untrusted ({}). In headless or non-interactive mode, authorization must be explicitly granted via session flag or permanent trust registration (`m31a trust add`).",
                        path.display(),
                        reason
                    );
                } else {
                    anyhow::bail!(
                        "Confirmation required for untrusted directory '{}' ({}).",
                        path.display(),
                        reason
                    );
                }
            }
        }
    }

    /// Evaluates whether an operation with a specific capability is authorized in the given directory.
    pub fn authorize_operation(
        &self,
        target: &Path,
        capability: OperationCapability,
    ) -> AuthorizationOutcome {
        let canonical = target
            .canonicalize()
            .unwrap_or_else(|_| target.to_path_buf());

        // 1. Trust management operations are handled by their own security flow,
        // but must be scoped and fail closed if the workspace is explicitly Denied.
        if capability == OperationCapability::TrustManagement {
            let decision = self.evaluate(&canonical);
            if let TrustDecision::Denied { path } = decision {
                return AuthorizationOutcome::Denied {
                    path,
                    capability,
                    reason: "Workspace is explicitly marked as Denied by security trust policy."
                        .into(),
                };
            }
            return AuthorizationOutcome::Allowed;
        }

        // 2. Harmless UI actions (help, settings viewing, palette) are permitted without trust.
        if capability == OperationCapability::HarmlessUI {
            return AuthorizationOutcome::Allowed;
        }

        // 3. Denied workspace: ALWAYS rejected for any workspace operation.
        let decision = self.evaluate(&canonical);
        match decision {
            TrustDecision::Denied { path } => AuthorizationOutcome::Denied {
                path,
                capability,
                reason: "Workspace is explicitly marked as Denied by security trust policy.".into(),
            },
            TrustDecision::Trusted | TrustDecision::SessionOnly => AuthorizationOutcome::Allowed,
            TrustDecision::RequiresConfirmation { path, reason } => {
                if capability == OperationCapability::WorkspaceMetadataRead {
                    // Safe directory metadata inspection allowed for doctor/status
                    AuthorizationOutcome::Allowed
                } else {
                    AuthorizationOutcome::RequiresConfirmation {
                        path,
                        capability,
                        reason,
                    }
                }
            }
        }
    }

    /// Authorizes execution for a child agent within a parent agent's context.
    ///
    /// Invariants:
    /// - Denied workspaces are ALWAYS rejected.
    /// - Parent must be authorized (Trusted or SessionOnly). If parent is untrusted/denied, spawn is rejected.
    /// - Child agent runs non-interactively/headless: RequiresConfirmation fails closed.
    /// - A child agent must not inherit authorization broader than its parent's permitted scope.
    ///   If parent is SessionOnly, child is at most SessionOnly.
    pub fn authorize_child_agent(
        &mut self,
        parent_cwd: &Path,
        child_cwd: &Path,
    ) -> Result<TrustAuthorization> {
        let parent_canonical = parent_cwd
            .canonicalize()
            .unwrap_or_else(|_| parent_cwd.to_path_buf());
        let child_canonical = child_cwd
            .canonicalize()
            .unwrap_or_else(|_| child_cwd.to_path_buf());

        let parent_decision = self.evaluate(&parent_canonical);
        match parent_decision {
            TrustDecision::Denied { path } => {
                anyhow::bail!(
                    "Child agent spawn DENIED: Parent workspace '{}' is marked as Denied by security trust policy.",
                    path.display()
                );
            }
            TrustDecision::RequiresConfirmation { path, reason } => {
                anyhow::bail!(
                    "Child agent spawn REJECTED: Parent workspace '{}' is untrusted ({}).",
                    path.display(),
                    reason
                );
            }
            TrustDecision::Trusted | TrustDecision::SessionOnly => {}
        }

        // Child directory evaluation
        let child_decision = self.evaluate(&child_canonical);
        match child_decision {
            TrustDecision::Denied { path } => {
                anyhow::bail!(
                    "Child agent spawn DENIED: Target workspace '{}' is marked as Denied by security trust policy.",
                    path.display()
                );
            }
            TrustDecision::RequiresConfirmation { path, reason } => {
                // Child in a worktree or subdirectory under parent's directory inherits parent's trust level
                if child_canonical.starts_with(&parent_canonical) {
                    let level = match parent_decision {
                        TrustDecision::Trusted => TrustLevel::Trusted,
                        _ => TrustLevel::SessionOnly,
                    };
                    Ok(TrustAuthorization {
                        workspace_path: child_canonical,
                        trust_level: level,
                        is_session_scoped: level == TrustLevel::SessionOnly,
                        decision_source: "inherited_parent_trust".to_string(),
                        authorized_at: Utc::now(),
                    })
                } else {
                    anyhow::bail!(
                        "Child agent spawn REJECTED: Working directory '{}' is outside parent scope and untrusted ({}).",
                        path.display(),
                        reason
                    );
                }
            }
            TrustDecision::SessionOnly => Ok(TrustAuthorization {
                workspace_path: child_canonical,
                trust_level: TrustLevel::SessionOnly,
                is_session_scoped: true,
                decision_source: "session_override".to_string(),
                authorized_at: Utc::now(),
            }),
            TrustDecision::Trusted => {
                // If parent only has SessionOnly trust, cap child at SessionOnly
                let level = match parent_decision {
                    TrustDecision::SessionOnly => TrustLevel::SessionOnly,
                    _ => TrustLevel::Trusted,
                };
                Ok(TrustAuthorization {
                    workspace_path: child_canonical,
                    trust_level: level,
                    is_session_scoped: level == TrustLevel::SessionOnly,
                    decision_source: "evaluated_trust".to_string(),
                    authorized_at: Utc::now(),
                })
            }
        }
    }

    /// Grants or updates trust for a directory.
    pub fn trust(&mut self, target: &Path, level: TrustLevel) -> Result<()> {
        let canonical = target
            .canonicalize()
            .unwrap_or_else(|_| target.to_path_buf());

        if level == TrustLevel::SessionOnly {
            self.session_trusts
                .insert(canonical, TrustLevel::SessionOnly);
            return Ok(());
        }

        // When updating persistent trust level, remove or synchronize any conflicting session entry
        self.session_trusts.remove(&canonical);
        if level == TrustLevel::Denied {
            self.session_trusts
                .insert(canonical.clone(), TrustLevel::Denied);
        }

        let mut data = self.load_data()?;
        let record = TrustedProjectRecord {
            path: canonical.clone(),
            trust_level: level,
            trusted_at: Utc::now(),
            git_remote: detect_git_remote(&canonical),
        };

        let key = canonical.to_string_lossy().to_string();
        data.projects.insert(key.clone(), record);
        self.save_data(&data)?;

        // Truthful verification: ensure the persistent write actually succeeded and matches
        let reloaded = self.load_data()?;
        match reloaded.projects.get(&key) {
            Some(r) if r.trust_level == level => Ok(()),
            Some(r) => anyhow::bail!(
                "Trust persistence verification failed: record has level {:?}, expected {:?}",
                r.trust_level,
                level
            ),
            None => anyhow::bail!(
                "Trust persistence verification failed: record not found in trust store"
            ),
        }
    }

    /// Revokes trust for a directory.
    pub fn revoke(&mut self, target: &Path) -> Result<bool> {
        let canonical = target
            .canonicalize()
            .unwrap_or_else(|_| target.to_path_buf());
        self.session_trusts.remove(&canonical);

        let mut data = self.load_data()?;
        let key = canonical.to_string_lossy().to_string();
        let removed = data.projects.remove(&key).is_some();
        if removed {
            self.save_data(&data)?;
            // Truthful verification: ensure the record was actually removed
            let reloaded = self.load_data()?;
            if reloaded.projects.contains_key(&key) {
                anyhow::bail!(
                    "Trust revocation verification failed: record still present in trust store"
                );
            }
        }
        Ok(removed)
    }

    /// Lists all persisted trusted projects.
    pub fn list(&self) -> Vec<TrustedProjectRecord> {
        let data = self.load_data().unwrap_or_default();
        let mut list: Vec<TrustedProjectRecord> = data.projects.into_values().collect();
        list.sort_by(|a, b| a.path.cmp(&b.path));
        list
    }
}

impl Default for TrustStore {
    fn default() -> Self {
        Self::new()
    }
}

fn detect_git_remote(dir: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(dir)
        .output()
        .ok()?;
    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !url.is_empty() {
            return Some(url);
        }
    }
    None
}
