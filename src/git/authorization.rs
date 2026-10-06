//! Non-forgeable runtime authorization for Git mutations (SEC-GIT-01).
//!
//! `GitGate::authorized()` as a zero-argument boolean is removed as a security
//! primitive. Git mutations require a [`GitMutationAuthorization`] minted by
//! the [`AuthorizationAuthority`] held by the runtime composition root and
//! bound to the exact execution being performed:
//!
//! ```text
//! authorization_id / mission_id / task_id / agent_id / request_id /
//! policy_hash / workspace_root / operation / resource_scope / expiry /
//! approval provenance
//! ```
//!
//! The binding tag is an HMAC-style SHA-256 over a per-instance secret and
//! the canonical field encoding. A gate minted for one mission, workspace,
//! operation, or policy generation does not verify for any other. Callers
//! cannot fabricate authorization without the authority instance.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::git::{GitError, GitOperation};
use crate::ids::{AgentId, MissionId, TaskId};

/// How long a minted Git authorization remains valid.
pub const GIT_AUTH_DEFAULT_TTL: Duration =
    Duration::from_secs(crate::config::canonical::DEFAULT_GIT_AUTH_TTL_SECS);

/// Per-runtime authorization minting authority.
///
/// Holds a process-unique secret generated at construction. Only this
/// instance can mint authorizations that [`GitGate`](crate::git::GitGate)
/// accepts; only code with access to this instance (the runtime composition
/// root and its governed helpers) can authorize Git mutation.
#[derive(Debug, Clone)]
pub struct AuthorizationAuthority {
    secret: [u8; 32],
}

impl Default for AuthorizationAuthority {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthorizationAuthority {
    /// Create a new authority with a fresh process-unique secret.
    pub fn new() -> Self {
        // Process-unique secret without extra dependencies: UUIDv7 material
        // hashed with process/time entropy.
        let uuid = uuid::Uuid::now_v7();
        let mut hasher = Sha256::new();
        hasher.update(uuid.as_bytes());
        hasher.update(std::process::id().to_be_bytes());
        hasher.update(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
                .to_be_bytes(),
        );
        let digest = hasher.finalize();
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&digest);
        Self { secret }
    }

    /// Mint a bound Git mutation authorization for one exact execution.
    ///
    /// `operation` is the primary mutation; `additional_operations` lists
    /// every other concrete mutation the same governed step may perform
    /// (e.g. an integration workflow authorizes `Merge` plus its bounded
    /// `WorktreeRemove` / `UpdateRef` / `SyncCheckout` finalization). The
    /// whole set is covered by one policy decision on the workflow action,
    /// and every covered operation is enumerated — no open-ended mutation.
    #[allow(clippy::too_many_arguments)]
    pub fn mint_git_authorization(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        agent_id: Option<AgentId>,
        policy_hash: impl Into<String>,
        workspace_root: PathBuf,
        operation: GitOperation,
        additional_operations: Vec<GitOperation>,
        resource_scope: impl Into<String>,
        provenance: impl Into<String>,
        ttl: Duration,
    ) -> GitMutationAuthorization {
        let policy_hash = policy_hash.into();
        let resource_scope = resource_scope.into();
        let provenance = provenance.into();
        let now = SystemTime::now();
        let expires_at = now
            .checked_add(ttl)
            .unwrap_or(now)
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let authorization_id = uuid::Uuid::now_v7().to_string();
        let request_id = uuid::Uuid::now_v7().to_string();
        let mut auth = GitMutationAuthorization {
            authorization_id,
            mission_id,
            task_id,
            agent_id,
            request_id,
            policy_hash,
            workspace_root,
            operation,
            additional_operations,
            resource_scope,
            expires_at_secs: expires_at,
            provenance,
            tag: String::new(),
        };
        auth.tag = self.tag_for(&auth);
        auth
    }

    /// Verify that `auth` was minted by this authority and is unmodified.
    pub fn verify(&self, auth: &GitMutationAuthorization) -> bool {
        if auth.tag.is_empty() {
            return false;
        }
        let expected = self.tag_for(auth);
        constant_time_eq(expected.as_bytes(), auth.tag.as_bytes())
    }

    fn tag_for(&self, auth: &GitMutationAuthorization) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.secret);
        hasher.update(auth.authorization_id.as_bytes());
        hasher.update(auth.mission_id.as_bytes());
        if let Some(task) = &auth.task_id {
            hasher.update(task.as_bytes());
        }
        if let Some(agent) = &auth.agent_id {
            hasher.update(agent.as_bytes());
        }
        hasher.update(auth.request_id.as_bytes());
        hasher.update(auth.policy_hash.as_bytes());
        hasher.update(auth.workspace_root.to_string_lossy().as_bytes());
        hasher.update(format!("{:?}", auth.operation).as_bytes());
        for extra in &auth.additional_operations {
            hasher.update(format!("{:?}", extra).as_bytes());
        }
        hasher.update(auth.resource_scope.as_bytes());
        hasher.update(auth.expires_at_secs.to_be_bytes());
        hasher.update(auth.provenance.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Non-forgeable authorization for exactly one Git mutation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GitMutationAuthorization {
    pub authorization_id: String,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
    pub request_id: String,
    pub policy_hash: String,
    pub workspace_root: PathBuf,
    pub operation: GitOperation,
    pub additional_operations: Vec<GitOperation>,
    pub resource_scope: String,
    /// Expiry as seconds since the Unix epoch.
    pub expires_at_secs: u64,
    pub provenance: String,
    pub tag: String,
}

impl GitMutationAuthorization {
    /// Whether the authorization has expired.
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(u64::MAX);
        now > self.expires_at_secs
    }

    /// Whether this authorization covers `op` in `workspace_root`.
    ///
    /// The operation must be the minted primary operation or one of the
    /// enumerated additional operations (exact variant and parameters); the
    /// workspace must be the minted workspace. Any mismatch fails closed.
    ///
    /// `Commit` operations bind the message excluding the self-describing
    /// `M31A-Authorization` trailer line: the gate is minted over the base
    /// message, the trailer names the very authorization being verified, and
    /// enforcement requires the trailer (when present) to name this exact
    /// authorization. All other message content binds exactly.
    pub fn covers(&self, op: &GitOperation, workspace_root: &Path) -> bool {
        if self.is_expired() {
            return false;
        }
        if !operations_match(&self.operation, op)
            && !self
                .additional_operations
                .iter()
                .any(|a| operations_match(a, op))
        {
            return false;
        }
        // For commits, the authorization trailer must name this authorization.
        if let GitOperation::Commit { message } = op
            && let Some(trailer_id) = commit_auth_trailer_id(message)
            && trailer_id != self.authorization_id
        {
            return false;
        }
        normalize(&self.workspace_root) == normalize(workspace_root)
    }

    /// Verify tag, expiry, operation, and workspace in one fail-closed check.
    pub fn verify_for(
        &self,
        authority: &AuthorizationAuthority,
        op: &GitOperation,
        workspace_root: &Path,
    ) -> Result<(), GitError> {
        if !authority.verify(self) {
            return Err(GitError::PolicyViolation(
                "git authorization tag mismatch: not minted by this runtime authority".to_string(),
            ));
        }
        if self.is_expired() {
            return Err(GitError::PolicyViolation(
                "git authorization expired".to_string(),
            ));
        }
        if !operations_match(&self.operation, op)
            && !self
                .additional_operations
                .iter()
                .any(|a| operations_match(a, op))
        {
            return Err(GitError::PolicyViolation(format!(
                "git authorization covers {:?} (+{:?}), not {:?}",
                self.operation, self.additional_operations, op
            )));
        }
        if let GitOperation::Commit { message } = op
            && let Some(trailer_id) = commit_auth_trailer_id(message)
            && trailer_id != self.authorization_id
        {
            return Err(GitError::PolicyViolation(
                "git commit authorization trailer names a different authorization".to_string(),
            ));
        }
        if normalize(&self.workspace_root) != normalize(workspace_root) {
            return Err(GitError::PolicyViolation(format!(
                "git authorization workspace '{}' does not match '{}'",
                self.workspace_root.display(),
                workspace_root.display()
            )));
        }
        if self.policy_hash.trim().is_empty() {
            return Err(GitError::PolicyViolation(
                "git authorization carries no policy hash".to_string(),
            ));
        }
        if self.provenance.trim().is_empty() {
            return Err(GitError::PolicyViolation(
                "git authorization carries no approval provenance".to_string(),
            ));
        }
        Ok(())
    }
}

/// Base commit message used for authorization binding: the message with the
/// self-describing `M31A-Authorization` trailer line removed and trailing
/// whitespace trimmed. Mint a commit gate over this base, then commit the
/// final message naming the minted authorization.
pub fn authorization_commit_base(message: &str) -> String {
    strip_commit_auth_trailer(message).0
}

fn normalize(p: &Path) -> PathBuf {
    let s = p.to_string_lossy().replace('\\', "/");
    let trimmed = s.trim_end_matches('/');
    PathBuf::from(trimmed)
}

/// Compare two operations for coverage, with `Commit` messages compared
/// on the base content only (excluding the self-describing
/// `M31A-Authorization` trailer line, whose binding to THIS authorization is
/// checked separately — comparing full tuples would reject every message
/// that names its own authorization).
fn operations_match(stored: &GitOperation, candidate: &GitOperation) -> bool {
    match (stored, candidate) {
        (GitOperation::Commit { message: a }, GitOperation::Commit { message: b }) => {
            strip_commit_auth_trailer(a).0 == strip_commit_auth_trailer(b).0
        }
        (a, b) => a == b,
    }
}

/// Split a commit message into (base, authorization trailer id).
fn strip_commit_auth_trailer(message: &str) -> (String, Option<String>) {
    let mut base_lines = Vec::new();
    let mut auth_id = None;
    for line in message.lines() {
        if let Some(val) = line.trim().strip_prefix("M31A-Authorization:") {
            let id = val.trim();
            if !id.is_empty() {
                auth_id = Some(id.to_string());
            }
        } else {
            base_lines.push(line);
        }
    }
    // Trailing blank line left by trailer removal is not significant.
    let mut base = base_lines.join("\n");
    while base.ends_with('\n') {
        base.pop();
    }
    (base, auth_id)
}

/// The authorization id named by a commit message's `M31A-Authorization`
/// trailer, if any.
fn commit_auth_trailer_id(message: &str) -> Option<String> {
    strip_commit_auth_trailer(message).1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authority() -> AuthorizationAuthority {
        AuthorizationAuthority::new()
    }

    #[test]
    fn minted_authorization_verifies() {
        let auth_holder = authority();
        let op = GitOperation::WorktreeRemove { force: true };
        let auth = auth_holder.mint_git_authorization(
            MissionId::new(),
            Some(TaskId::new()),
            None,
            "hash1",
            PathBuf::from("/tmp/ws"),
            op.clone(),
            Vec::new(),
            "worktree:/tmp/ws/wt",
            "test",
            GIT_AUTH_DEFAULT_TTL,
        );
        assert!(auth_holder.verify(&auth));
        assert!(
            auth.verify_for(&auth_holder, &op, Path::new("/tmp/ws"))
                .is_ok()
        );
    }

    #[test]
    fn tampered_operation_fails_closed() {
        let auth_holder = authority();
        let op = GitOperation::WorktreeRemove { force: true };
        let auth = auth_holder.mint_git_authorization(
            MissionId::new(),
            Some(TaskId::new()),
            None,
            "hash1",
            PathBuf::from("/tmp/ws"),
            op,
            Vec::new(),
            "scope",
            "test",
            GIT_AUTH_DEFAULT_TTL,
        );
        let other = GitOperation::Merge {
            source: "feature".to_string(),
        };
        assert!(
            auth.verify_for(&auth_holder, &other, Path::new("/tmp/ws"))
                .is_err()
        );
    }

    #[test]
    fn wrong_workspace_fails_closed() {
        let auth_holder = authority();
        let op = GitOperation::WorktreeRemove { force: false };
        let auth = auth_holder.mint_git_authorization(
            MissionId::new(),
            Some(TaskId::new()),
            None,
            "hash1",
            PathBuf::from("/tmp/ws"),
            op.clone(),
            Vec::new(),
            "scope",
            "test",
            GIT_AUTH_DEFAULT_TTL,
        );
        assert!(
            auth.verify_for(&auth_holder, &op, Path::new("/tmp/other"))
                .is_err()
        );
    }

    #[test]
    fn foreign_authority_fails_closed() {
        let a = authority();
        let b = authority();
        let op = GitOperation::WorktreeRemove { force: false };
        let auth = a.mint_git_authorization(
            MissionId::new(),
            Some(TaskId::new()),
            None,
            "hash1",
            PathBuf::from("/tmp/ws"),
            op.clone(),
            Vec::new(),
            "scope",
            "test",
            GIT_AUTH_DEFAULT_TTL,
        );
        assert!(auth.verify_for(&b, &op, Path::new("/tmp/ws")).is_err());
    }

    #[test]
    fn expired_authorization_fails_closed() {
        let auth_holder = authority();
        let op = GitOperation::WorktreeRemove { force: false };
        let mut auth = auth_holder.mint_git_authorization(
            MissionId::new(),
            Some(TaskId::new()),
            None,
            "hash1",
            PathBuf::from("/tmp/ws"),
            op.clone(),
            Vec::new(),
            "scope",
            "test",
            Duration::from_secs(0),
        );
        // Force expiry into the past and re-tag so only expiry (not tag) trips.
        auth.expires_at_secs = 1;
        auth.tag = auth_holder.tag_for(&auth);
        assert!(auth.is_expired());
        assert!(
            auth.verify_for(&auth_holder, &op, Path::new("/tmp/ws"))
                .is_err()
        );
    }
}
