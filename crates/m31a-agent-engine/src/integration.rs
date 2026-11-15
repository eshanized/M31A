//! Authoritative Unified Integration & Approval Service
//!
//! Provides a single, unified pipeline for promoting candidate solutions,
//! integrating child-agent diffs, and applying workflow step integrations.
//!
//! Enforces:
//! 1. Approval gating (`require_approval_for_integration`)
//! 2. Workspace cleanliness checking (`is_dirty`)
//! 3. Pre-integration file snapshotting (content & existence)
//! 4. Strict path containment (no directory traversal `..` or root escaping)
//! 5. Code review gating (`ReviewEngine`)
//! 6. Post-integration compilation & test verification (`AutonomousVerifier`)
//! 7. Exact per-file rollback on verification failure (restores snapshot bytes,
//!    unlinks created files; never executes broad destructive Git cleanups)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use m31a_core::workflow::{
    ChangeSetManifest, ExecutionJournalEvent, IntegrationLifecycleStatus, IntegrationRecord,
    PreIntegrationSnapshotRecord, PreIntegrationSnapshotStatus,
};
use m31a_storage::{WorkflowStore, WorktreeManager, WorktreeSettings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};
use uuid::Uuid;

use m31a_verify::AutonomousVerifier;

/// Static mutual exclusion lock guaranteeing serialized integration onto target workspaces.
static INTEGRATION_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Computes immutable content-addressed SHA-256 hash for patch change sets.
pub fn compute_change_set_hash(patch: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(patch.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Request payload describing an integration operation.
#[derive(Debug, Clone)]
pub struct IntegrationRequest {
    /// Root directory of the target workspace into which changes will be integrated.
    pub workspace_root: PathBuf,
    /// Patch text to apply (either `*** Begin Patch` or unified diff format).
    pub patch_content: String,
    /// Identifier of the agent or candidate producing the changes.
    pub source_label: String,
    /// Optional commit message title.
    pub commit_message: Option<String>,
    /// Whether explicit human approval is required before touching the workspace.
    pub require_approval: bool,
    /// Whether explicit approval has been granted.
    pub is_approved: bool,
    /// Role of the author agent, if known.
    pub author_role: Option<String>,
    /// Whether to skip workspace cleanliness check (useful in non-git test fixtures).
    pub skip_dirty_check: bool,
    /// Whether to skip compiler/test verification.
    pub skip_verification: bool,
    /// Maximum duration for post-integration verification commands.
    pub verification_timeout: Option<Duration>,
    /// Associated workflow identifier, if part of a managed workflow.
    pub workflow_id: Option<String>,
    /// Associated task identifier, if part of a managed workflow.
    pub task_id: Option<String>,
    /// Attempt identifier for fencing checks.
    pub attempt_id: Option<u32>,
    /// Worker identifier for fencing checks.
    pub worker_id: Option<String>,
    /// Immutable ChangeSetManifest if promoting a canonical candidate.
    pub manifest: Option<ChangeSetManifest>,
    /// Expected change-set hash for immutable binding.
    pub expected_change_set_hash: Option<String>,
    /// Expected target repository HEAD commit SHA immediately prior to integration (Compare-And-Swap).
    pub expected_target_head_sha: Option<String>,
    /// Source Git branch of the child worktree, if applicable.
    pub source_branch: Option<String>,
    /// Base commit SHA from which the child worktree was branched.
    pub source_base_sha: Option<String>,
    /// Expected target repository base SHA at the time the child worktree was created.
    pub target_base_sha: Option<String>,
}

impl Default for IntegrationRequest {
    fn default() -> Self {
        Self {
            workspace_root: PathBuf::new(),
            patch_content: String::new(),
            source_label: String::new(),
            commit_message: None,
            require_approval: false,
            is_approved: false,
            author_role: None,
            skip_dirty_check: false,
            skip_verification: false,
            verification_timeout: None,
            workflow_id: None,
            task_id: None,
            attempt_id: None,
            worker_id: None,
            manifest: None,
            expected_change_set_hash: None,
            expected_target_head_sha: None,
            source_branch: None,
            source_base_sha: None,
            target_base_sha: None,
        }
    }
}

/// Detailed outcome of an integration attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationOutcome {
    pub success: bool,
    pub pre_check_clean: bool,
    pub approval_granted: bool,
    pub patch_applied: bool,
    pub verification_passed: bool,
    #[serde(default)]
    pub commit_completed: bool,
    pub rollback_performed: bool,
    pub commit_sha: Option<String>,
    pub modified_files: Vec<PathBuf>,
    pub error: Option<String>,
    pub summary: String,
}

/// Structured outcome of a verified exact rollback operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub restored_files: Vec<PathBuf>,
    pub removed_created_files: Vec<PathBuf>,
    pub failed_files: Vec<(PathBuf, String)>,
    pub complete: bool,
    pub diagnostics: Option<String>,
}

/// Snapshot of an individual file prior to patch application.
#[derive(Debug, Clone)]
pub enum FileSnapshot {
    Existing {
        path: PathBuf,
        relative_path: PathBuf,
        content: Vec<u8>,
        sha256: String,
        mode: Option<u32>,
    },
    NonExistent {
        path: PathBuf,
        relative_path: PathBuf,
    },
}

impl FileSnapshot {
    /// Capture snapshot of a file relative to workspace root with symlink containment checks.
    pub fn snapshot(workspace_root: &Path, rel_path: &Path) -> Result<Self> {
        let full_path = workspace_root.join(rel_path);

        // Symlink containment check
        if full_path.is_symlink() {
            let target = fs::read_link(&full_path)?;
            let resolved = if target.is_absolute() {
                target
            } else {
                full_path.parent().unwrap_or(workspace_root).join(target)
            };
            if let (Ok(can_ws), Ok(can_res)) =
                (workspace_root.canonicalize(), resolved.canonicalize())
                && !can_res.starts_with(&can_ws)
            {
                bail!(
                    "Symlink escape detected: {} points outside workspace",
                    rel_path.display()
                );
            }
        }

        if full_path.exists() && full_path.is_file() {
            let content = fs::read(&full_path).with_context(|| {
                format!("Failed to read file for snapshot: {}", full_path.display())
            })?;
            let mut hasher = Sha256::new();
            hasher.update(&content);
            let sha256 = format!("{:x}", hasher.finalize());

            let mode = fs::metadata(&full_path).ok().map(|m| {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    m.permissions().mode()
                }
                #[cfg(not(unix))]
                {
                    0
                }
            });

            Ok(Self::Existing {
                path: full_path,
                relative_path: rel_path.to_path_buf(),
                content,
                sha256,
                mode,
            })
        } else {
            Ok(Self::NonExistent {
                path: full_path,
                relative_path: rel_path.to_path_buf(),
            })
        }
    }

    /// Restore file to exact pre-integration state and verify restoration.
    pub fn restore(&self) -> Result<()> {
        match self {
            Self::Existing {
                path,
                content,
                sha256,
                mode,
                ..
            } => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }

                // Write via safe atomic restoration
                let tmp_path =
                    path.with_extension(format!("tmp_restore_{}", uuid::Uuid::new_v4().simple()));
                fs::write(&tmp_path, content).with_context(|| {
                    format!(
                        "Failed to write tmp file for restoration: {}",
                        tmp_path.display()
                    )
                })?;

                #[cfg(unix)]
                if let Some(m) = mode {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(*m));
                }

                fs::rename(&tmp_path, path).with_context(|| {
                    format!("Failed to rename restored file: {}", path.display())
                })?;

                // Verify restoration hash
                let restored = fs::read(path)?;
                let mut hasher = Sha256::new();
                hasher.update(&restored);
                let current_sha256 = format!("{:x}", hasher.finalize());
                if &current_sha256 != sha256 {
                    bail!(
                        "Rollback verification failed for {}: sha256 mismatch (expected {}, got {})",
                        path.display(),
                        sha256,
                        current_sha256
                    );
                }
            }
            Self::NonExistent { path, .. } => {
                if path.exists() || path.is_symlink() {
                    if path.is_dir() {
                        let _ = fs::remove_dir_all(path);
                    } else {
                        let _ = fs::remove_file(path);
                    }
                }
                if path.exists() || path.is_symlink() {
                    bail!(
                        "Rollback verification failed: file {} still exists after removal",
                        path.display()
                    );
                }
            }
        }
        Ok(())
    }
}

/// Service providing unified, authoritative integration logic across direct agents,
/// speculative candidates, and workflow execution steps.
#[derive(Debug, Default, Clone)]
pub struct WorkflowIntegrationService {
    store: Option<WorkflowStore>,
}

impl WorkflowIntegrationService {
    pub fn new() -> Self {
        Self { store: None }
    }

    pub fn with_store(mut self, store: WorkflowStore) -> Self {
        self.store = Some(store);
        self
    }

    pub fn store(&self) -> Option<&WorkflowStore> {
        self.store.as_ref()
    }

    /// Resolve HEAD commit SHA of a git repository directory.
    pub fn get_current_head(&self, workspace_root: &Path) -> Result<String> {
        let output = Command::new("git")
            .current_dir(workspace_root)
            .args(["rev-parse", "HEAD"])
            .output()
            .context("Failed running git rev-parse HEAD")?;
        if !output.status.success() {
            bail!("git rev-parse HEAD failed");
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Returns list of files modified between two commit revisions.
    pub fn get_modified_files_between(
        &self,
        workspace_root: &Path,
        from_sha: &str,
        to_sha: &str,
    ) -> Result<Vec<PathBuf>> {
        let output = Command::new("git")
            .current_dir(workspace_root)
            .args(["diff", "--name-only", &format!("{from_sha}..{to_sha}")])
            .output()
            .context("Failed running git diff --name-only")?;
        if !output.status.success() {
            bail!("git diff between revisions failed");
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let files = stdout
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect();
        Ok(files)
    }

    /// Evaluates whether a patch diff can apply cleanly to the workspace (preflight check).
    pub fn git_preflight_check(&self, workspace_root: &Path, patch_content: &str) -> Result<bool> {
        if patch_content.contains("*** Begin Patch") {
            let parsed = match m31a_tools::patch::parse_patch(patch_content) {
                Ok(p) => p,
                Err(_) => return Ok(false),
            };
            for hunk in &parsed.hunks {
                let target_path = workspace_root.join(hunk.path());
                if let m31a_tools::patch::Hunk::UpdateFile { chunks, .. } = hunk {
                    if !target_path.exists() {
                        return Ok(false);
                    }
                    if let Ok(file_content) = fs::read_to_string(&target_path) {
                        for chunk in chunks {
                            for old_line in &chunk.old_lines {
                                if !file_content.contains(old_line) {
                                    return Ok(false);
                                }
                            }
                        }
                    } else {
                        return Ok(false);
                    }
                }
            }
            Ok(true)
        } else {
            let mut child = Command::new("git")
                .current_dir(workspace_root)
                .args(["apply", "--check", "--ignore-whitespace", "-"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .context("Failed to spawn git apply --check preflight")?;

            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(patch_content.as_bytes());
            }

            let output = child.wait_with_output()?;
            Ok(output.status.success())
        }
    }

    /// Preserves a failed or rejected integration patch on disk and in durable storage for diagnostics.
    fn preserve_failed_patch(&self, req: &IntegrationRequest, change_set_hash: &str, reason: &str) {
        if let (Some(store), Some(wf_id)) = (&self.store, &req.workflow_id) {
            let task_part = req.task_id.as_deref().unwrap_or("unknown");
            let hash_prefix = if change_set_hash.len() >= 8 {
                &change_set_hash[..8]
            } else {
                change_set_hash
            };
            let artifact_name = format!("failed_integration_{task_part}_{hash_prefix}.diff");
            let _ = store.store_artifact_file(
                &req.workspace_root,
                wf_id,
                req.task_id.as_deref(),
                &artifact_name,
                "failed_integration_patch",
                req.patch_content.as_bytes(),
                "text/x-diff",
            );
            warn!(
                workflow_id = %wf_id,
                task_id = ?req.task_id,
                reason = %reason,
                artifact = %artifact_name,
                "Preserved failed integration patch artifact"
            );
        }
    }

    fn update_record_status(
        &self,
        id: &str,
        status: IntegrationLifecycleStatus,
        target_sha: Option<&str>,
        error: Option<&str>,
    ) {
        if let Some(ref store) = self.store {
            let _ = store.update_integration_status(id, status, target_sha, error);
        }
    }

    /// Execute an authoritative integration workflow.
    pub async fn integrate(&self, req: &IntegrationRequest) -> Result<IntegrationOutcome> {
        let _integration_guard = INTEGRATION_LOCK.lock().await;
        let is_git = req.workspace_root.join(".git").exists();
        let now_ms = Utc::now().timestamp_millis();
        let integration_id = format!("integ-{}", Uuid::new_v4().simple());
        let current_target_sha = if is_git {
            self.get_current_head(&req.workspace_root).ok()
        } else {
            None
        };

        // 0. Change-set hash verification
        let actual_hash = compute_change_set_hash(&req.patch_content);
        if let Some(ref expected) = req.expected_change_set_hash
            && expected != &actual_hash
        {
            return Ok(IntegrationOutcome {
                success: false,
                pre_check_clean: false,
                approval_granted: false,
                patch_applied: false,
                verification_passed: false,
                commit_completed: false,
                rollback_performed: false,
                commit_sha: None,
                modified_files: Vec::new(),
                error: Some(format!(
                    "Change-set hash mismatch: expected {expected}, got {actual_hash}"
                )),
                summary:
                    "Integration rejected: change-set hash does not match authorized change set"
                        .into(),
            });
        }

        // 0.1 Idempotent deduplication check: If already promoted, return success immediately
        if let (Some(store), Some(wf_id)) = (&self.store, &req.workflow_id)
            && let Ok(Some(promoted)) = store.find_promoted_integration_by_hash(wf_id, &actual_hash)
        {
            info!(
                workflow_id = %wf_id,
                change_set_hash = %actual_hash,
                promoted_id = %promoted.id,
                "Idempotent skip: change-set hash already promoted"
            );
            return Ok(IntegrationOutcome {
                success: true,
                pre_check_clean: true,
                approval_granted: true,
                patch_applied: false,
                verification_passed: true,
                commit_completed: true,
                rollback_performed: false,
                commit_sha: promoted.current_target_sha,
                modified_files: promoted.modified_files,
                error: None,
                summary: "Idempotent skip: change set already promoted".into(),
            });
        }

        // Check for empty diff
        if req.patch_content.trim().is_empty() {
            return Ok(IntegrationOutcome {
                success: true,
                pre_check_clean: true,
                approval_granted: true,
                patch_applied: false,
                verification_passed: true,
                commit_completed: true,
                rollback_performed: false,
                commit_sha: None,
                modified_files: Vec::new(),
                error: None,
                summary: "No changes to integrate (patch is empty)".into(),
            });
        }

        // 0.2 Authoritative worker attempt fencing check
        if let (Some(store), Some(wf_id), Some(task_id)) =
            (&self.store, &req.workflow_id, &req.task_id)
            && let Ok(Some(task)) = store.get_task(wf_id, task_id)
        {
            if let Some(req_attempt) = req.attempt_id
                && task.attempt_id != req_attempt
            {
                let err_msg = format!(
                    "Fencing violation: integration request attempt {} is superseded by authoritative attempt {}",
                    req_attempt, task.attempt_id
                );
                warn!(workflow_id = %wf_id, task_id = %task_id, "{}", err_msg);
                return Ok(IntegrationOutcome {
                    success: false,
                    pre_check_clean: false,
                    approval_granted: false,
                    patch_applied: false,
                    verification_passed: false,
                    commit_completed: false,
                    rollback_performed: false,
                    commit_sha: None,
                    modified_files: Vec::new(),
                    error: Some(err_msg.clone()),
                    summary: err_msg,
                });
            }
            if let Some(ref req_worker) = req.worker_id
                && task.worker_id.as_deref() != Some(req_worker)
            {
                let err_msg = format!(
                    "Fencing violation: worker '{}' does not own task (authoritative worker: '{:?}')",
                    req_worker, task.worker_id
                );
                warn!(workflow_id = %wf_id, task_id = %task_id, "{}", err_msg);
                return Ok(IntegrationOutcome {
                    success: false,
                    pre_check_clean: false,
                    approval_granted: false,
                    patch_applied: false,
                    verification_passed: false,
                    commit_completed: false,
                    rollback_performed: false,
                    commit_sha: None,
                    modified_files: Vec::new(),
                    error: Some(err_msg.clone()),
                    summary: err_msg,
                });
            }
        }

        // 0.3 ChangeSetManifest integrity and freeze validation
        if let Some(ref manifest) = req.manifest {
            if !manifest.is_frozen {
                let err_msg = "Integration rejected: Candidate manifest is not frozen".to_string();
                return Ok(IntegrationOutcome {
                    success: false,
                    pre_check_clean: false,
                    approval_granted: false,
                    patch_applied: false,
                    verification_passed: false,
                    commit_completed: false,
                    rollback_performed: false,
                    commit_sha: None,
                    modified_files: Vec::new(),
                    error: Some(err_msg.clone()),
                    summary: err_msg,
                });
            }
            if let Err(e) = manifest.validate_integrity() {
                let err_msg =
                    format!("Integration rejected: Manifest integrity validation failed: {e}");
                return Ok(IntegrationOutcome {
                    success: false,
                    pre_check_clean: false,
                    approval_granted: false,
                    patch_applied: false,
                    verification_passed: false,
                    commit_completed: false,
                    rollback_performed: false,
                    commit_sha: None,
                    modified_files: Vec::new(),
                    error: Some(err_msg.clone()),
                    summary: err_msg,
                });
            }
        }

        // 0.4 Target Revision Compare-And-Swap (CAS) Precondition Check
        if is_git
            && let (Some(expected_head), Some(actual_head)) =
                (&req.expected_target_head_sha, &current_target_sha)
            && expected_head != actual_head
        {
            let err_msg = format!(
                "Target revision mismatch (CAS check failed): target HEAD advanced from expected {expected_head} to {actual_head}; re-verification required"
            );
            warn!(workflow_id = ?req.workflow_id, "{}", err_msg);
            return Ok(IntegrationOutcome {
                success: false,
                pre_check_clean: false,
                approval_granted: false,
                patch_applied: false,
                verification_passed: false,
                commit_completed: false,
                rollback_performed: false,
                commit_sha: None,
                modified_files: Vec::new(),
                error: Some(err_msg.clone()),
                summary: err_msg,
            });
        }

        // 0.5 Dirty workspace check (fail closed if target workspace contains uncommitted changes)
        let is_clean = if is_git && !req.skip_dirty_check {
            let settings = WorktreeSettings::new(&req.workspace_root);
            let mgr = WorktreeManager::new(settings);
            match mgr.is_dirty(&req.workspace_root) {
                Ok(dirty) => !dirty,
                Err(e) => {
                    warn!(err = %e, "Failed inspecting workspace cleanliness; failing closed");
                    false
                }
            }
        } else {
            true
        };

        if !is_clean {
            let err_msg = "Target workspace contains uncommitted changes or cleanliness check failed; clean workspace required".to_string();
            return Ok(IntegrationOutcome {
                success: false,
                pre_check_clean: false,
                approval_granted: true,
                patch_applied: false,
                verification_passed: false,
                commit_completed: false,
                rollback_performed: false,
                commit_sha: None,
                modified_files: Vec::new(),
                error: Some(err_msg),
                summary: "Integration blocked: target workspace is dirty".into(),
            });
        }

        // 1. Path extraction and containment verification
        let affected_rel_paths =
            match self.extract_and_validate_paths(&req.workspace_root, &req.patch_content) {
                Ok(paths) => paths,
                Err(e) => {
                    return Ok(IntegrationOutcome {
                        success: false,
                        pre_check_clean: true,
                        approval_granted: true,
                        patch_applied: false,
                        verification_passed: false,
                        commit_completed: false,
                        rollback_performed: false,
                        commit_sha: None,
                        modified_files: Vec::new(),
                        error: Some(format!("Patch validation failed: {e}")),
                        summary: format!("Integration rejected: invalid patch paths: {e}"),
                    });
                }
            };

        // Initialize durable IntegrationRecord in SQLite (CREATED -> EXECUTING)
        if let (Some(store), Some(wf_id), Some(task_id)) =
            (&self.store, &req.workflow_id, &req.task_id)
        {
            let record = IntegrationRecord {
                id: integration_id.clone(),
                workflow_id: wf_id.clone(),
                task_id: task_id.clone(),
                attempt_id: req.attempt_id.unwrap_or(1),
                status: IntegrationLifecycleStatus::Executing,
                source_branch: req
                    .source_branch
                    .clone()
                    .unwrap_or_else(|| "unknown".into()),
                source_base_sha: req
                    .source_base_sha
                    .clone()
                    .unwrap_or_else(|| current_target_sha.clone().unwrap_or_default()),
                target_base_sha: req
                    .target_base_sha
                    .clone()
                    .unwrap_or_else(|| current_target_sha.clone().unwrap_or_default()),
                current_target_sha: current_target_sha.clone(),
                change_set_hash: actual_hash.clone(),
                patch_artifact_path: None,
                modified_files: affected_rel_paths.clone(),
                verification_passed: false,
                error: None,
                created_at_epoch_ms: now_ms,
                updated_at_epoch_ms: now_ms,
            };
            let _ = store.create_integration_record(&record);
        }

        // 2. Approval gating
        if req.require_approval && !req.is_approved {
            self.update_record_status(
                &integration_id,
                IntegrationLifecycleStatus::IntegrationPending,
                current_target_sha.as_deref(),
                Some("Pending human approval"),
            );
            return Ok(IntegrationOutcome {
                success: false,
                pre_check_clean: false,
                approval_granted: false,
                patch_applied: false,
                verification_passed: false,
                commit_completed: false,
                rollback_performed: false,
                commit_sha: None,
                modified_files: Vec::new(),
                error: Some("Human approval required for integration but not granted".into()),
                summary: "Integration blocked: pending human approval".into(),
            });
        }

        // 3. Stale base revision and overlapping modification detection
        if is_git
            && let (Some(base_sha), Some(target_head)) = (&req.source_base_sha, &current_target_sha)
            && base_sha != target_head
        {
            // Target HEAD has advanced since source worktree was created!
            let intermediate_files =
                self.get_modified_files_between(&req.workspace_root, base_sha, target_head)?;
            let has_overlap = affected_rel_paths
                .iter()
                .any(|p| intermediate_files.contains(p));
            if has_overlap {
                // If overlapping files were modified in target HEAD, check if git apply preflight succeeds
                if !self.git_preflight_check(&req.workspace_root, &req.patch_content)? {
                    let err_msg = format!(
                        "Stale base revision: target repository advanced from {base_sha} to {target_head} with conflicting overlapping modifications"
                    );
                    self.preserve_failed_patch(req, &actual_hash, &err_msg);
                    self.update_record_status(
                        &integration_id,
                        IntegrationLifecycleStatus::Rejected,
                        current_target_sha.as_deref(),
                        Some(&err_msg),
                    );
                    return Ok(IntegrationOutcome {
                        success: false,
                        pre_check_clean: true,
                        approval_granted: true,
                        patch_applied: false,
                        verification_passed: false,
                        commit_completed: false,
                        rollback_performed: false,
                        commit_sha: None,
                        modified_files: affected_rel_paths,
                        error: Some(err_msg.clone()),
                        summary: err_msg,
                    });
                }
            }
        }

        // 4. Git preflight check
        if is_git && !self.git_preflight_check(&req.workspace_root, &req.patch_content)? {
            let err_msg =
                "Git preflight validation failed: child worktree diff conflicts with target HEAD"
                    .to_string();
            self.preserve_failed_patch(req, &actual_hash, &err_msg);
            self.update_record_status(
                &integration_id,
                IntegrationLifecycleStatus::Failed,
                current_target_sha.as_deref(),
                Some(&err_msg),
            );
            return Ok(IntegrationOutcome {
                success: false,
                pre_check_clean: true,
                approval_granted: true,
                patch_applied: false,
                verification_passed: false,
                commit_completed: false,
                rollback_performed: false,
                commit_sha: None,
                modified_files: affected_rel_paths,
                error: Some(err_msg.clone()),
                summary: err_msg,
            });
        }

        // 5. Code review engine gate
        let review_report = m31a_review::ReviewEngine::analyze(&req.patch_content);
        if review_report.verdict == m31a_review::ReviewVerdict::RequestChanges {
            let reasons: Vec<String> = review_report
                .findings
                .iter()
                .filter(|f| {
                    f.severity == m31a_review::ReviewSeverity::Critical
                        || f.severity == m31a_review::ReviewSeverity::High
                })
                .map(|f| format!("{}: {}", f.title, f.description))
                .collect();
            let err_text = format!(
                "Integration rejected by Code Review Engine: {}",
                reasons.join("; ")
            );
            self.preserve_failed_patch(req, &actual_hash, &err_text);
            self.update_record_status(
                &integration_id,
                IntegrationLifecycleStatus::Rejected,
                current_target_sha.as_deref(),
                Some(&err_text),
            );
            return Ok(IntegrationOutcome {
                success: false,
                pre_check_clean: true,
                approval_granted: true,
                patch_applied: false,
                verification_passed: false,
                commit_completed: false,
                rollback_performed: false,
                commit_sha: None,
                modified_files: Vec::new(),
                error: Some(err_text.clone()),
                summary: err_text,
            });
        }

        // 6. Verification preflight manifest check
        if !req.skip_verification {
            let is_rust = req.workspace_root.join("Cargo.toml").exists();
            let is_node = req.workspace_root.join("package.json").exists();
            let is_python = req.workspace_root.join("pyproject.toml").exists()
                || req.workspace_root.join("setup.py").exists()
                || req.workspace_root.join("requirements.txt").exists();
            let is_go = req.workspace_root.join("go.mod").exists();
            let is_bazel = req.workspace_root.join("BUILD.bazel").exists()
                || req.workspace_root.join("MODULE.bazel").exists()
                || req.workspace_root.join("WORKSPACE").exists();
            let is_cmake = req.workspace_root.join("CMakeLists.txt").exists();
            let is_gradle = req.workspace_root.join("build.gradle").exists()
                || req.workspace_root.join("build.gradle.kts").exists();

            if !is_rust && !is_node && !is_python && !is_go && !is_bazel && !is_cmake && !is_gradle
            {
                let err_msg = "VerificationIncomplete: unsupported project type; automated verification cannot prove correctness without recognized build manifest (Cargo.toml, package.json, pyproject.toml, or go.mod)".to_string();
                self.update_record_status(
                    &integration_id,
                    IntegrationLifecycleStatus::Failed,
                    current_target_sha.as_deref(),
                    Some(&err_msg),
                );
                return Ok(IntegrationOutcome {
                    success: false,
                    pre_check_clean: true,
                    approval_granted: true,
                    patch_applied: false,
                    verification_passed: false,
                    commit_completed: false,
                    rollback_performed: false,
                    commit_sha: None,
                    modified_files: Vec::new(),
                    error: Some(err_msg.clone()),
                    summary: err_msg,
                });
            }
        }

        // 8. Pre-integration snapshotting and durable preimage persistence
        let mut snapshots = Vec::new();
        for rel_path in &affected_rel_paths {
            match FileSnapshot::snapshot(&req.workspace_root, rel_path) {
                Ok(snap) => {
                    // Durable snapshotting: persist preimage into SQLite before target workspace is modified
                    if let (Some(store), Some(wf_id), Some(task_id)) =
                        (&self.store, &req.workflow_id, &req.task_id)
                    {
                        let snap_id = format!("snap-{}", Uuid::new_v4().simple());
                        let (orig_exists, orig_hash, orig_mode, preimage_bytes) = match &snap {
                            FileSnapshot::Existing {
                                content,
                                sha256,
                                mode,
                                ..
                            } => (true, Some(sha256.clone()), *mode, Some(content.as_slice())),
                            FileSnapshot::NonExistent { .. } => (false, None, None, None),
                        };
                        let snap_record = PreIntegrationSnapshotRecord {
                            id: snap_id,
                            integration_id: integration_id.clone(),
                            workflow_id: wf_id.clone(),
                            task_id: task_id.clone(),
                            attempt_id: req.attempt_id.unwrap_or(1),
                            relative_path: rel_path.clone(),
                            original_exists: orig_exists,
                            original_content_hash: orig_hash,
                            original_mode: orig_mode,
                            preimage_storage_path: None,
                            status: PreIntegrationSnapshotStatus::Prepared,
                            created_at_epoch_ms: now_ms,
                        };
                        let _ = store.save_pre_integration_snapshot(&snap_record, preimage_bytes);
                    }
                    snapshots.push(snap);
                }
                Err(e) => {
                    let err_msg = format!("Failed taking pre-integration snapshot: {e}");
                    self.update_record_status(
                        &integration_id,
                        IntegrationLifecycleStatus::Failed,
                        current_target_sha.as_deref(),
                        Some(&err_msg),
                    );
                    return Ok(IntegrationOutcome {
                        success: false,
                        pre_check_clean: true,
                        approval_granted: true,
                        patch_applied: false,
                        verification_passed: false,
                        commit_completed: false,
                        rollback_performed: false,
                        commit_sha: None,
                        modified_files: Vec::new(),
                        error: Some(err_msg),
                        summary: "Integration aborted during pre-integration snapshotting".into(),
                    });
                }
            }
        }

        // 9. Apply patch -> RESULT_READY
        let apply_result = self.apply_patch(&req.workspace_root, &req.patch_content);
        if let Err(e) = apply_result {
            let rb = self.execute_exact_rollback(&snapshots);
            let err_msg = format!("Patch application failed: {e}");
            self.preserve_failed_patch(req, &actual_hash, &err_msg);
            self.update_record_status(
                &integration_id,
                IntegrationLifecycleStatus::Failed,
                current_target_sha.as_deref(),
                Some(&err_msg),
            );
            return Ok(IntegrationOutcome {
                success: false,
                pre_check_clean: true,
                approval_granted: true,
                patch_applied: false,
                verification_passed: false,
                commit_completed: false,
                rollback_performed: true,
                commit_sha: None,
                modified_files: affected_rel_paths,
                error: Some(err_msg.clone()),
                summary: format!(
                    "Patch application failed; rolled back changes (complete={}): {e}",
                    rb.complete
                ),
            });
        }

        self.update_record_status(
            &integration_id,
            IntegrationLifecycleStatus::ResultReady,
            current_target_sha.as_deref(),
            None,
        );

        // 10. Post-integration verification gate -> INTEGRATION_PENDING -> VERIFIED
        self.update_record_status(
            &integration_id,
            IntegrationLifecycleStatus::IntegrationPending,
            current_target_sha.as_deref(),
            None,
        );

        if !req.skip_verification {
            let timeout = req.verification_timeout.unwrap_or(Duration::from_secs(30));

            // Compiler check
            match AutonomousVerifier::verify_check(&req.workspace_root, timeout).await {
                Ok(report) if !report.success => {
                    let rb = self.execute_exact_rollback(&snapshots);
                    let err_msg = format!(
                        "Post-integration build check failed: {}",
                        report
                            .self_healing_prompt
                            .as_deref()
                            .unwrap_or(&report.raw_output)
                    );
                    self.preserve_failed_patch(req, &actual_hash, &err_msg);
                    self.update_record_status(
                        &integration_id,
                        IntegrationLifecycleStatus::Failed,
                        current_target_sha.as_deref(),
                        Some(&err_msg),
                    );
                    return Ok(IntegrationOutcome {
                        success: false,
                        pre_check_clean: true,
                        approval_granted: true,
                        patch_applied: true,
                        verification_passed: false,
                        commit_completed: false,
                        rollback_performed: true,
                        commit_sha: None,
                        modified_files: affected_rel_paths,
                        error: Some(err_msg),
                        summary: format!(
                            "Verification failed (compiler check); exact per-file rollback executed (complete={})",
                            rb.complete
                        ),
                    });
                }
                Err(e) => {
                    let rb = self.execute_exact_rollback(&snapshots);
                    let err_msg = format!("Failed executing post-integration check: {e}");
                    self.preserve_failed_patch(req, &actual_hash, &err_msg);
                    self.update_record_status(
                        &integration_id,
                        IntegrationLifecycleStatus::Failed,
                        current_target_sha.as_deref(),
                        Some(&err_msg),
                    );
                    return Ok(IntegrationOutcome {
                        success: false,
                        pre_check_clean: true,
                        approval_granted: true,
                        patch_applied: true,
                        verification_passed: false,
                        commit_completed: false,
                        rollback_performed: true,
                        commit_sha: None,
                        modified_files: affected_rel_paths,
                        error: Some(err_msg),
                        summary: format!(
                            "Verification error during compiler check; exact per-file rollback executed (complete={})",
                            rb.complete
                        ),
                    });
                }
                Ok(_) => {}
            }

            // Test execution
            match AutonomousVerifier::verify_tests(&req.workspace_root, timeout).await {
                Ok(report) if !report.success => {
                    let rb = self.execute_exact_rollback(&snapshots);
                    let err_msg = format!(
                        "Post-integration test suite failed: {}",
                        report
                            .self_healing_prompt
                            .as_deref()
                            .unwrap_or(&report.raw_output)
                    );
                    self.preserve_failed_patch(req, &actual_hash, &err_msg);
                    self.update_record_status(
                        &integration_id,
                        IntegrationLifecycleStatus::Failed,
                        current_target_sha.as_deref(),
                        Some(&err_msg),
                    );
                    return Ok(IntegrationOutcome {
                        success: false,
                        pre_check_clean: true,
                        approval_granted: true,
                        patch_applied: true,
                        verification_passed: false,
                        commit_completed: false,
                        rollback_performed: true,
                        commit_sha: None,
                        modified_files: affected_rel_paths,
                        error: Some(err_msg),
                        summary: format!(
                            "Verification failed (tests); exact per-file rollback executed (complete={})",
                            rb.complete
                        ),
                    });
                }
                Err(e) => {
                    let rb = self.execute_exact_rollback(&snapshots);
                    let err_msg = format!("Failed executing post-integration tests: {e}");
                    self.preserve_failed_patch(req, &actual_hash, &err_msg);
                    self.update_record_status(
                        &integration_id,
                        IntegrationLifecycleStatus::Failed,
                        current_target_sha.as_deref(),
                        Some(&err_msg),
                    );
                    return Ok(IntegrationOutcome {
                        success: false,
                        pre_check_clean: true,
                        approval_granted: true,
                        patch_applied: true,
                        verification_passed: false,
                        commit_completed: false,
                        rollback_performed: true,
                        commit_sha: None,
                        modified_files: affected_rel_paths,
                        error: Some(err_msg),
                        summary: format!(
                            "Verification error during test suite; exact per-file rollback executed (complete={})",
                            rb.complete
                        ),
                    });
                }
                Ok(_) => {}
            }
        }

        self.update_record_status(
            &integration_id,
            IntegrationLifecycleStatus::Verified,
            current_target_sha.as_deref(),
            None,
        );

        // 11. Commit changes to Git -> PROMOTED
        let (commit_sha, commit_completed) = if is_git {
            match self.commit_integrated_changes(
                &req.workspace_root,
                &affected_rel_paths,
                req.commit_message.as_deref(),
                &req.source_label,
                req.author_role.as_deref().unwrap_or("Autonomous Agent"),
            ) {
                Ok(sha) => (Some(sha), true),
                Err(e) => {
                    warn!(err = %e, "Git commit after verification failed; triggering exact per-file rollback");
                    let rb = self.execute_exact_rollback(&snapshots);
                    let err_msg = format!("Git commit failed: {e}");
                    self.preserve_failed_patch(req, &actual_hash, &err_msg);
                    self.update_record_status(
                        &integration_id,
                        IntegrationLifecycleStatus::Failed,
                        current_target_sha.as_deref(),
                        Some(&err_msg),
                    );
                    return Ok(IntegrationOutcome {
                        success: false,
                        pre_check_clean: true,
                        approval_granted: true,
                        patch_applied: true,
                        verification_passed: true,
                        commit_completed: false,
                        rollback_performed: true,
                        commit_sha: None,
                        modified_files: affected_rel_paths,
                        error: Some(err_msg),
                        summary: format!(
                            "Integration failed: git commit error: {e}; exact rollback executed (complete={})",
                            rb.complete
                        ),
                    });
                }
            }
        } else {
            (None, true)
        };

        self.update_record_status(
            &integration_id,
            IntegrationLifecycleStatus::Promoted,
            commit_sha.as_deref(),
            None,
        );

        // Mark pre-integration snapshots as reconciled in durable storage
        if let Some(ref store) = self.store {
            if let Ok(snaps) = store.list_pre_integration_snapshots(&integration_id) {
                for (snap, _) in snaps {
                    let _ = store.mark_pre_integration_snapshot_status(
                        &snap.id,
                        PreIntegrationSnapshotStatus::Restored,
                    );
                }
            }
            if let (Some(wf_id), Some(task_id)) = (&req.workflow_id, &req.task_id) {
                let _ = store.record_execution_journal_event(&ExecutionJournalEvent {
                    id: format!("ej-{}", Uuid::new_v4().simple()),
                    workflow_id: wf_id.clone(),
                    task_id: task_id.clone(),
                    attempt_id: req.attempt_id.unwrap_or(1),
                    worker_id: req.worker_id.clone().unwrap_or_else(|| "promoter".into()),
                    operation_id: Some("promote".into()),
                    event_type: "candidate_promoted".into(),
                    previous_state: Some("verified".into()),
                    new_state: Some("promoted".into()),
                    resource_identity: commit_sha.clone(),
                    manifest_hash: Some(actual_hash.clone()),
                    payload_json: serde_json::json!({
                        "commit_sha": commit_sha,
                        "files_count": affected_rel_paths.len(),
                        "integration_id": integration_id,
                    })
                    .to_string(),
                    timestamp_epoch_ms: Utc::now().timestamp_millis(),
                });
            }
        }

        info!(
            source = %req.source_label,
            files_count = affected_rel_paths.len(),
            commit = ?commit_sha,
            commit_completed,
            "Integration successfully completed and verified"
        );

        Ok(IntegrationOutcome {
            success: true,
            pre_check_clean: true,
            approval_granted: true,
            patch_applied: true,
            verification_passed: true,
            commit_completed,
            rollback_performed: false,
            commit_sha,
            modified_files: affected_rel_paths,
            error: None,
            summary: "Integration verified and applied cleanly".into(),
        })
    }

    /// Extract and strictly validate paths affected by a patch, rejecting traversal and symlink escapes.
    pub fn extract_and_validate_paths(
        &self,
        workspace_root: &Path,
        patch_content: &str,
    ) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        if patch_content.contains("*** Begin Patch") {
            let parsed = m31a_tools::patch::parse_patch(patch_content)
                .map_err(|e| anyhow::anyhow!("Patch parse error: {e}"))?;
            for hunk in parsed.hunks {
                paths.push(hunk.path().clone());
            }
        } else {
            paths = WorktreeManager::validate_patch_paths(patch_content)?;
        }

        // Strict path containment: reject absolute paths and '..' components
        for p in &paths {
            if p.is_absolute() {
                bail!("Path in patch is absolute: {}", p.display());
            }
            for comp in p.components() {
                if matches!(comp, std::path::Component::ParentDir) {
                    bail!(
                        "Path traversal attempt ('..') detected in path: {}",
                        p.display()
                    );
                }
            }

            // Symlink escape check: ensure parent/file does not escape workspace via symlinks
            let target_path = workspace_root.join(p);
            if target_path.is_symlink() {
                let target = fs::read_link(&target_path)?;
                let resolved = if target.is_absolute() {
                    target
                } else {
                    target_path.parent().unwrap_or(workspace_root).join(target)
                };
                if let (Ok(can_ws), Ok(can_res)) =
                    (workspace_root.canonicalize(), resolved.canonicalize())
                    && !can_res.starts_with(&can_ws)
                {
                    bail!(
                        "Symlink escape attempt detected: {} escapes workspace boundary",
                        p.display()
                    );
                }
            }
        }

        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    /// Apply patch to target workspace root.
    fn apply_patch(&self, workspace_root: &Path, patch_content: &str) -> Result<()> {
        if patch_content.contains("*** Begin Patch") {
            let parsed = m31a_tools::patch::parse_patch(patch_content)
                .map_err(|e| anyhow::anyhow!("Patch parse error: {e}"))?;
            m31a_tools::patch::apply_patch_to_workspace(workspace_root, &parsed)?;
            Ok(())
        } else {
            let mut apply_cmd = Command::new("git");
            apply_cmd
                .current_dir(workspace_root)
                .args(["apply", "--ignore-whitespace", "-"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let mut child = apply_cmd
                .spawn()
                .context("Failed to spawn git apply in workspace")?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(patch_content.as_bytes());
            }
            let out = child.wait_with_output()?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                bail!("git apply failed: {}", err);
            }
            Ok(())
        }
    }

    /// Execute exact per-file rollback using pre-integration snapshots.
    pub fn execute_exact_rollback(&self, snapshots: &[FileSnapshot]) -> RollbackResult {
        let mut restored_files = Vec::new();
        let mut removed_created_files = Vec::new();
        let mut failed_files = Vec::new();

        for snap in snapshots {
            match snap {
                FileSnapshot::Existing { relative_path, .. } => match snap.restore() {
                    Ok(()) => restored_files.push(relative_path.clone()),
                    Err(e) => {
                        error!(err = %e, path = %relative_path.display(), "Failed restoring snapshot during exact per-file rollback");
                        failed_files.push((relative_path.clone(), e.to_string()));
                    }
                },
                FileSnapshot::NonExistent { relative_path, .. } => match snap.restore() {
                    Ok(()) => removed_created_files.push(relative_path.clone()),
                    Err(e) => {
                        error!(err = %e, path = %relative_path.display(), "Failed removing created file during exact per-file rollback");
                        failed_files.push((relative_path.clone(), e.to_string()));
                    }
                },
            }
        }

        let complete = failed_files.is_empty();
        let diagnostics = if complete {
            None
        } else {
            Some(format!(
                "Rollback failed for {} files: {:?}",
                failed_files.len(),
                failed_files
            ))
        };

        RollbackResult {
            restored_files,
            removed_created_files,
            failed_files,
            complete,
            diagnostics,
        }
    }

    /// Resumes incomplete rollback operations from durable SQLite pre-integration snapshots after process restart.
    pub fn resume_incomplete_rollbacks(
        &self,
        workspace_root: &Path,
        integration_id: &str,
    ) -> Result<RollbackResult> {
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| anyhow!("WorkflowStore required to resume incomplete rollbacks"))?;

        let snapshot_entries = store.list_pre_integration_snapshots(integration_id)?;
        let mut restored_files = Vec::new();
        let mut removed_created_files = Vec::new();
        let mut failed_files = Vec::new();

        for (rec, blob_opt) in snapshot_entries {
            if rec.status == PreIntegrationSnapshotStatus::Restored {
                continue;
            }

            let full_path = workspace_root.join(&rec.relative_path);
            if rec.original_exists {
                if let Some(content) = blob_opt {
                    if let Some(parent) = full_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let tmp = full_path
                        .with_extension(format!("tmp_recover_{}", Uuid::new_v4().simple()));
                    if let Err(e) = fs::write(&tmp, &content) {
                        failed_files.push((rec.relative_path.clone(), e.to_string()));
                        let _ = store.mark_pre_integration_snapshot_status(
                            &rec.id,
                            PreIntegrationSnapshotStatus::Failed,
                        );
                        continue;
                    }
                    #[cfg(unix)]
                    if let Some(m) = rec.original_mode {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(m));
                    }
                    if let Err(e) = fs::rename(&tmp, &full_path) {
                        failed_files.push((rec.relative_path.clone(), e.to_string()));
                        let _ = store.mark_pre_integration_snapshot_status(
                            &rec.id,
                            PreIntegrationSnapshotStatus::Failed,
                        );
                        continue;
                    }
                    // Verify sha256
                    if let (Ok(bytes), Some(expected_hash)) =
                        (fs::read(&full_path), rec.original_content_hash.as_ref())
                    {
                        let mut h = Sha256::new();
                        h.update(&bytes);
                        let actual_hash = format!("{:x}", h.finalize());
                        if &actual_hash != expected_hash {
                            failed_files
                                .push((rec.relative_path.clone(), "Restored hash mismatch".into()));
                            let _ = store.mark_pre_integration_snapshot_status(
                                &rec.id,
                                PreIntegrationSnapshotStatus::Failed,
                            );
                            continue;
                        }
                    }
                    let _ = store.mark_pre_integration_snapshot_status(
                        &rec.id,
                        PreIntegrationSnapshotStatus::Restored,
                    );
                    restored_files.push(rec.relative_path);
                } else {
                    failed_files.push((
                        rec.relative_path.clone(),
                        "Preimage bytes missing in storage".into(),
                    ));
                    let _ = store.mark_pre_integration_snapshot_status(
                        &rec.id,
                        PreIntegrationSnapshotStatus::Failed,
                    );
                }
            } else {
                if full_path.exists() || full_path.is_symlink() {
                    let _ = if full_path.is_dir() {
                        fs::remove_dir_all(&full_path)
                    } else {
                        fs::remove_file(&full_path)
                    };
                }
                if full_path.exists() || full_path.is_symlink() {
                    failed_files.push((
                        rec.relative_path.clone(),
                        "Failed to remove newly created file".into(),
                    ));
                    let _ = store.mark_pre_integration_snapshot_status(
                        &rec.id,
                        PreIntegrationSnapshotStatus::Failed,
                    );
                } else {
                    let _ = store.mark_pre_integration_snapshot_status(
                        &rec.id,
                        PreIntegrationSnapshotStatus::Restored,
                    );
                    removed_created_files.push(rec.relative_path);
                }
            }
        }

        let complete = failed_files.is_empty();
        let diagnostics = if complete {
            None
        } else {
            Some(format!(
                "Incomplete rollback recovery: {} failures",
                failed_files.len()
            ))
        };

        Ok(RollbackResult {
            restored_files,
            removed_created_files,
            failed_files,
            complete,
            diagnostics,
        })
    }

    /// Commit verified changes with structured M31A trailers.
    fn commit_integrated_changes(
        &self,
        workspace_root: &Path,
        files: &[PathBuf],
        custom_msg: Option<&str>,
        source_label: &str,
        role: &str,
    ) -> Result<String> {
        let mut add_cmd = Command::new("git");
        add_cmd.current_dir(workspace_root).arg("add");
        for f in files {
            add_cmd.arg(f);
        }
        let add_out = add_cmd.output()?;
        if !add_out.status.success() {
            bail!(
                "git add failed: {}",
                String::from_utf8_lossy(&add_out.stderr)
            );
        }

        let default_title = format!("feat(agent): integrate changes from {source_label}");
        let title = custom_msg.unwrap_or(&default_title).trim();
        let full_message = format!(
            "{title}\n\nAgent-Id: {source_label}\nAgent-Role: {role}\nCo-authored-by: M31A Agent <agent-{source_label}@m31a.local>"
        );

        let commit_out = Command::new("git")
            .current_dir(workspace_root)
            .args(["commit", "-m", &full_message])
            .output()?;
        if !commit_out.status.success() {
            bail!(
                "git commit failed: {}",
                String::from_utf8_lossy(&commit_out.stderr)
            );
        }

        let head_out = Command::new("git")
            .current_dir(workspace_root)
            .args(["rev-parse", "HEAD"])
            .output()?;
        if !head_out.status.success() {
            bail!(
                "git rev-parse HEAD failed: {}",
                String::from_utf8_lossy(&head_out.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&head_out.stdout).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_approval_gating_blocks_unapproved_integration() {
        let temp = tempdir().unwrap();
        let svc = WorkflowIntegrationService::new();

        let req = IntegrationRequest {
            workspace_root: temp.path().to_path_buf(),
            patch_content: "dummy patch".into(),
            source_label: "test-agent".into(),
            commit_message: None,
            require_approval: true,
            is_approved: false, // Not approved!
            author_role: None,
            skip_dirty_check: true,
            skip_verification: true,
            verification_timeout: None,
            workflow_id: None,
            task_id: None,
            attempt_id: None,
            expected_change_set_hash: None,
            source_branch: None,
            source_base_sha: None,
            target_base_sha: None,
            worker_id: None,
            manifest: None,
            expected_target_head_sha: None,
        };

        let res = svc.integrate(&req).await.unwrap();
        assert!(!res.success);
        assert!(!res.approval_granted);
        assert!(res.error.unwrap().contains("approval required"));
    }

    #[tokio::test]
    async fn test_exact_per_file_rollback_on_failed_integration() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        // Setup an existing file
        let existing_path = root.join("existing.txt");
        fs::write(&existing_path, "original content\n").unwrap();

        // Create tool patch modifying existing.txt and adding new.txt
        let patch_text = r#"*** Begin Patch
*** Update File: existing.txt
-original content
+modified content
*** Add File: new.txt
+newly created content
*** End Patch
"#;

        let svc = WorkflowIntegrationService::new();
        let affected = svc.extract_and_validate_paths(root, patch_text).unwrap();
        assert_eq!(affected.len(), 2);

        // Take pre-integration snapshots
        let mut snapshots = Vec::new();
        for p in &affected {
            snapshots.push(FileSnapshot::snapshot(root, p).unwrap());
        }

        // Apply the patch
        svc.apply_patch(root, patch_text).unwrap();
        assert_eq!(
            fs::read_to_string(&existing_path).unwrap(),
            "modified content\n"
        );
        assert!(root.join("new.txt").exists());

        // Now execute exact per-file rollback
        svc.execute_exact_rollback(&snapshots);

        // Verify restoration: original content is back, newly created file is removed!
        assert_eq!(
            fs::read_to_string(&existing_path).unwrap(),
            "original content\n"
        );
        assert!(!root.join("new.txt").exists());
    }

    #[tokio::test]
    async fn test_path_traversal_rejected() {
        let temp = tempdir().unwrap();
        let svc = WorkflowIntegrationService::new();

        let malicious_patch = r#"*** Begin Patch
*** Update File: ../outside.txt
+hack
*** End Patch
"#;

        let req = IntegrationRequest {
            workspace_root: temp.path().to_path_buf(),
            patch_content: malicious_patch.into(),
            source_label: "malicious".into(),
            commit_message: None,
            require_approval: false,
            is_approved: true,
            author_role: None,
            skip_dirty_check: true,
            skip_verification: true,
            verification_timeout: None,
            workflow_id: None,
            task_id: None,
            attempt_id: None,
            expected_change_set_hash: None,
            source_branch: None,
            source_base_sha: None,
            target_base_sha: None,
            worker_id: None,
            manifest: None,
            expected_target_head_sha: None,
        };

        let res = svc.integrate(&req).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("traversal"));
    }

    #[tokio::test]
    async fn test_fencing_rejects_stale_worker_attempt() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("test.db");
        let storage = m31a_storage::Storage::open(&db_path).unwrap();
        let store = storage.workflow_store();

        let wf_id = "wf-fence-1";
        let task_id = "task-fence-1";
        let run = m31a_core::workflow::WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: temp.path().to_path_buf(),
            objective: "Fencing test".into(),
            workflow_type: "autonomous".into(),
            state: m31a_core::workflow::WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let plan = m31a_core::workflow::WorkflowPlan {
            workflow_id: wf_id.to_string(),
            revision: 1,
            rationale: "Initial plan".into(),
            tasks: vec![m31a_core::workflow::WorkflowTask {
                id: task_id.into(),
                workflow_id: wf_id.into(),
                plan_revision: 1,
                task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
                name: "Fenced Task".into(),
                description: "Test".into(),
                assigned_role: "coder".into(),
                assigned_agent_id: None,
                priority: 10,
                wave_index: 1,
                dependencies: vec![],
                acceptance_criteria: vec![],
                required_verification: vec![],
                declared_read_paths: vec![],
                declared_write_paths: vec![],
                workspace_isolation: m31a_core::workflow::WorkspaceIsolationStrategy::Worktree,
                status: m31a_core::workflow::WorkflowTaskStatus::Ready,
                retry_count: 0,
                max_retries: 3,
                attempt_id: 1,
                started_at_epoch_ms: None,
                completed_at_epoch_ms: None,
                worker_id: None,
                lease_expires_at_epoch_ms: None,
                error_details: None,
                input_artifacts: vec![],
                output_artifacts: vec![],
                verification_result_json: None,
                execution_result_json: None,
            }],
            created_at_epoch_ms: 1000,
        };
        store.save_workflow_plan(&plan).unwrap();

        // Worker A claims attempt 1
        store.claim_task(wf_id, task_id, "worker-A", 60).unwrap();

        let svc = WorkflowIntegrationService::new().with_store(store);

        // Worker B attempts to integrate with attempt 1 -> Rejected
        let req = IntegrationRequest {
            workspace_root: temp.path().to_path_buf(),
            patch_content: "*** Begin Patch\n*** End Patch\n".into(),
            source_label: "test".into(),
            workflow_id: Some(wf_id.to_string()),
            task_id: Some(task_id.to_string()),
            attempt_id: Some(1),
            worker_id: Some("worker-B".into()), // Stale worker!
            skip_dirty_check: true,
            skip_verification: true,
            is_approved: true,
            ..Default::default()
        };

        let res = svc.integrate(&req).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("Fencing violation"));
    }

    #[tokio::test]
    async fn test_unfrozen_manifest_rejected() {
        let temp = tempdir().unwrap();
        let svc = WorkflowIntegrationService::new();

        let manifest = ChangeSetManifest {
            manifest_version: 1,
            id: "manifest-1".into(),
            workflow_id: "wf-1".into(),
            plan_revision: 1,
            task_id: "task-1".into(),
            attempt_id: 1,
            worker_id: "worker-1".into(),
            agent_id: None,
            repository_root: temp.path().to_path_buf(),
            canonical_workspace_id: "ws-1".into(),
            source_base_sha: "base-sha".into(),
            target_commit_sha: None,
            files: vec![],
            isolation_strategy: m31a_core::workflow::WorkspaceIsolationStrategy::Worktree,
            created_at_epoch_ms: 1000,
            frozen_at_epoch_ms: None,
            is_frozen: false,
            manifest_hash: String::new(),
        };

        let req = IntegrationRequest {
            workspace_root: temp.path().to_path_buf(),
            patch_content: "*** Begin Patch\n*** End Patch\n".into(),
            source_label: "test".into(),
            manifest: Some(manifest),
            skip_dirty_check: true,
            skip_verification: true,
            is_approved: true,
            ..Default::default()
        };

        let res = svc.integrate(&req).await.unwrap();
        assert!(!res.success);
        assert!(
            res.error
                .unwrap()
                .to_lowercase()
                .contains("manifest is not frozen")
        );
    }

    #[tokio::test]
    async fn test_target_head_cas_rejection() {
        let temp = tempdir().unwrap();
        let svc = WorkflowIntegrationService::new();

        // Initialize git repo in temp
        let _ = Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["config", "user.name", "Tester"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["config", "user.email", "tester@test.local"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        fs::write(temp.path().join("init.txt"), "init").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp.path())
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let req = IntegrationRequest {
            workspace_root: temp.path().to_path_buf(),
            patch_content: "*** Begin Patch\n*** End Patch\n".into(),
            source_label: "test".into(),
            expected_target_head_sha: Some("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef".into()),
            skip_dirty_check: true,
            skip_verification: true,
            is_approved: true,
            ..Default::default()
        };

        let res = svc.integrate(&req).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("CAS check failed"));
    }

    #[tokio::test]
    async fn test_resume_incomplete_rollbacks_restores_snapshot_bytes() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("test.db");
        let storage = m31a_storage::Storage::open(&db_path).unwrap();
        let store = storage.workflow_store();

        let wf_id = "wf-rollback-1";
        let run = m31a_core::workflow::WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: temp.path().to_path_buf(),
            objective: "Rollback test".into(),
            workflow_type: "autonomous".into(),
            state: m31a_core::workflow::WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let integration_id = "test-integ-123";
        let file1_rel = PathBuf::from("src/main.rs");
        let file2_rel = PathBuf::from("src/unwanted.rs");

        let file1_abs = temp.path().join(&file1_rel);
        let file2_abs = temp.path().join(&file2_rel);
        fs::create_dir_all(file1_abs.parent().unwrap()).unwrap();

        // Mutate files as if a crash occurred mid-patch
        fs::write(&file1_abs, "corrupted content").unwrap();
        fs::write(&file2_abs, "unwanted new file").unwrap();

        let clean_bytes = b"original clean content\n";
        let mut hasher = Sha256::new();
        hasher.update(clean_bytes);
        let expected_hash = format!("{:x}", hasher.finalize());

        // Record pre-integration snapshots in SQLite
        let rec1 = PreIntegrationSnapshotRecord {
            id: Uuid::new_v4().to_string(),
            integration_id: integration_id.to_string(),
            workflow_id: wf_id.into(),
            task_id: "task-1".into(),
            attempt_id: 1,
            relative_path: file1_rel,
            original_exists: true,
            original_content_hash: Some(expected_hash),
            original_mode: None,
            preimage_storage_path: None,
            status: PreIntegrationSnapshotStatus::Prepared,
            created_at_epoch_ms: 1000,
        };
        store
            .save_pre_integration_snapshot(&rec1, Some(clean_bytes))
            .unwrap();

        let rec2 = PreIntegrationSnapshotRecord {
            id: Uuid::new_v4().to_string(),
            integration_id: integration_id.to_string(),
            workflow_id: wf_id.into(),
            task_id: "task-1".into(),
            attempt_id: 1,
            relative_path: file2_rel,
            original_exists: false,
            original_content_hash: None,
            original_mode: None,
            preimage_storage_path: None,
            status: PreIntegrationSnapshotStatus::Prepared,
            created_at_epoch_ms: 1000,
        };
        store.save_pre_integration_snapshot(&rec2, None).unwrap();

        // Service resumes rollback
        let svc = WorkflowIntegrationService::new().with_store(store.clone());
        let outcome = svc
            .resume_incomplete_rollbacks(temp.path(), integration_id)
            .unwrap();

        assert_eq!(outcome.restored_files.len(), 1);
        assert_eq!(outcome.removed_created_files.len(), 1);
        assert!(outcome.failed_files.is_empty());

        // Verify disk state
        assert_eq!(
            fs::read_to_string(&file1_abs).unwrap(),
            "original clean content\n"
        );
        assert!(!file2_abs.exists());

        // Verify SQLite state is now Restored
        let list = store
            .list_pre_integration_snapshots(integration_id)
            .unwrap();
        for (rec, _) in list {
            assert_eq!(rec.status, PreIntegrationSnapshotStatus::Restored);
        }
    }
}
