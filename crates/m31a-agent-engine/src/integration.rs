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

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};

use m31a_storage::{WorktreeManager, WorktreeSettings};
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
    /// Expected change-set hash for immutable binding.
    pub expected_change_set_hash: Option<String>,
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
pub struct WorkflowIntegrationService;

impl WorkflowIntegrationService {
    pub fn new() -> Self {
        Self
    }

    /// Execute an authoritative integration workflow.
    pub async fn integrate(&self, req: &IntegrationRequest) -> Result<IntegrationOutcome> {
        let _integration_guard = INTEGRATION_LOCK.lock().await;
        let is_git = req.workspace_root.join(".git").exists();

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

        // 1. Approval gating
        if req.require_approval && !req.is_approved {
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

        // 2. Path extraction and containment verification
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

        // 3. Code review engine gate
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

        // 4. Verification preflight manifest check
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

        // 5. Dirty workspace check (fail closed if target workspace contains uncommitted changes)
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
                error: Some(
                    "Target workspace contains uncommitted changes or cleanliness check failed; clean workspace required"
                        .into(),
                ),
                summary: "Integration blocked: target workspace is dirty".into(),
            });
        }

        // 5. Pre-integration snapshotting
        let mut snapshots = Vec::new();
        for rel_path in &affected_rel_paths {
            match FileSnapshot::snapshot(&req.workspace_root, rel_path) {
                Ok(snap) => snapshots.push(snap),
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
                        error: Some(format!("Failed taking pre-integration snapshot: {e}")),
                        summary: "Integration aborted during pre-integration snapshotting".into(),
                    });
                }
            }
        }

        // 6. Apply patch
        let apply_result = self.apply_patch(&req.workspace_root, &req.patch_content);
        if let Err(e) = apply_result {
            // Rollback any partial file changes made
            let rb = self.execute_exact_rollback(&snapshots);
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
                error: Some(format!("Patch application failed: {e}")),
                summary: format!(
                    "Patch application failed; rolled back changes (complete={}): {e}",
                    rb.complete
                ),
            });
        }

        // 7. Post-integration verification gate
        if !req.skip_verification {
            let timeout = req.verification_timeout.unwrap_or(Duration::from_secs(30));

            // Compiler check
            match AutonomousVerifier::verify_check(&req.workspace_root, timeout).await {
                Ok(report) if !report.success => {
                    let rb = self.execute_exact_rollback(&snapshots);
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
                        error: Some(format!(
                            "Post-integration build check failed: {}",
                            report
                                .self_healing_prompt
                                .as_deref()
                                .unwrap_or(&report.raw_output)
                        )),
                        summary: format!(
                            "Verification failed (compiler check); exact per-file rollback executed (complete={})",
                            rb.complete
                        ),
                    });
                }
                Err(e) => {
                    let rb = self.execute_exact_rollback(&snapshots);
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
                        error: Some(format!("Failed executing post-integration check: {e}")),
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
                        error: Some(format!(
                            "Post-integration test suite failed: {}",
                            report
                                .self_healing_prompt
                                .as_deref()
                                .unwrap_or(&report.raw_output)
                        )),
                        summary: format!(
                            "Verification failed (tests); exact per-file rollback executed (complete={})",
                            rb.complete
                        ),
                    });
                }
                Err(e) => {
                    let rb = self.execute_exact_rollback(&snapshots);
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
                        error: Some(format!("Failed executing post-integration tests: {e}")),
                        summary: format!(
                            "Verification error during test suite; exact per-file rollback executed (complete={})",
                            rb.complete
                        ),
                    });
                }
                Ok(_) => {}
            }
        }

        // 8. Commit changes if working inside a Git repository
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
                        error: Some(format!("Git commit failed: {e}")),
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
    fn extract_and_validate_paths(
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
        };

        let res = svc.integrate(&req).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("traversal"));
    }
}
