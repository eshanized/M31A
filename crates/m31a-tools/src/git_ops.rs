//! Git repository operations, status inspect, branch analysis, and safety trust checks.
//!
//! High-level Git operations: status inspection, branch tracking, diff inspection, and safe commits.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum GitToolingError {
    #[error("not a git repository: {0}")]
    NotAGitRepository(PathBuf),

    #[error("git command failed: {cmd} (exit status: {status:?}, stderr: {stderr})")]
    GitCommand {
        cmd: String,
        status: Option<i32>,
        stderr: String,
    },

    #[error("repository trust check failed: {0}")]
    UntrustedRepository(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GitFileStatusKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitFileChange {
    pub path: String,
    pub kind: GitFileStatusKind,
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GitRepoStatus {
    pub repo_root: PathBuf,
    pub current_branch: Option<String>,
    pub head_commit: Option<String>,
    pub ahead_count: usize,
    pub behind_count: usize,
    pub staged: Vec<GitFileChange>,
    pub unstaged: Vec<GitFileChange>,
    pub untracked: Vec<String>,
    pub has_changes: bool,
}

/// Discovers the root directory of the enclosing Git repository without invoking the `git` binary.
pub fn find_git_repo_root(start_dir: &Path) -> Option<PathBuf> {
    let mut current = fs::canonicalize(start_dir).ok()?;

    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            return Some(current);
        }

        if !current.pop() {
            break;
        }
    }

    None
}

/// Ensures the path belongs to a valid Git repository, returning the repository root.
pub fn ensure_git_repository(path: &Path) -> Result<PathBuf, GitToolingError> {
    find_git_repo_root(path).ok_or_else(|| GitToolingError::NotAGitRepository(path.to_path_buf()))
}

/// Resolves the current HEAD commit hash.
pub fn resolve_head(repo_root: &Path) -> Result<Option<String>, GitToolingError> {
    let output = run_git(repo_root, &["rev-parse", "HEAD"])?;
    let trimmed = output.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

/// Resolves the current branch name, or detached HEAD description.
pub fn resolve_current_branch(repo_root: &Path) -> Result<Option<String>, GitToolingError> {
    let output = run_git(repo_root, &["branch", "--show-current"])?;
    let trimmed = output.trim();
    if !trimmed.is_empty() {
        return Ok(Some(trimmed.to_string()));
    }

    // If empty, check if detached HEAD
    if let Ok(rev) = run_git(repo_root, &["rev-parse", "--short", "HEAD"]) {
        let rev = rev.trim();
        if !rev.is_empty() {
            return Ok(Some(format!("HEAD (detached at {rev})")));
        }
    }

    Ok(None)
}

/// Computes ahead and behind commit counts between the specified branch and its upstream.
pub fn branch_ahead_behind(repo_root: &Path, branch: &str) -> (usize, usize) {
    let upstream_res = run_git(
        repo_root,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            &format!("{branch}@{{upstream}}"),
        ],
    );

    let Ok(upstream) = upstream_res else {
        return (0, 0);
    };
    let upstream = upstream.trim();
    if upstream.is_empty() {
        return (0, 0);
    }

    let counts_res = run_git(
        repo_root,
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("{branch}...{upstream}"),
        ],
    );

    let Ok(counts) = counts_res else {
        return (0, 0);
    };

    let mut parts = counts.split_whitespace();
    let ahead = parts
        .next()
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(0);
    let behind = parts
        .next()
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(0);

    (ahead, behind)
}

/// Collects complete repository status including staged, unstaged, untracked, branch, and ahead/behind counts.
pub fn get_repository_status(cwd: &Path) -> Result<GitRepoStatus, GitToolingError> {
    let repo_root = ensure_git_repository(cwd)?;
    let branch = resolve_current_branch(&repo_root).ok().flatten();
    let head = resolve_head(&repo_root).ok().flatten();

    let (ahead, behind) = if let Some(ref b) = branch {
        if !b.starts_with("HEAD (detached") {
            branch_ahead_behind(&repo_root, b)
        } else {
            (0, 0)
        }
    } else {
        (0, 0)
    };

    let porcelain = run_git(&repo_root, &["status", "--porcelain=v1", "-uall"])?;

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    for line in porcelain.lines() {
        if line.len() < 3 {
            continue;
        }

        let index_status = line.as_bytes()[0] as char;
        let worktree_status = line.as_bytes()[1] as char;
        let file_path = line[3..].trim().to_string();

        if index_status == '?' && worktree_status == '?' {
            untracked.push(file_path);
            continue;
        }

        // Staged changes
        if index_status != ' ' && index_status != '?' {
            staged.push(GitFileChange {
                path: file_path.clone(),
                kind: parse_status_code(index_status),
                staged: true,
            });
        }

        // Unstaged changes
        if worktree_status != ' ' && worktree_status != '?' {
            unstaged.push(GitFileChange {
                path: file_path,
                kind: parse_status_code(worktree_status),
                staged: false,
            });
        }
    }

    let has_changes = !staged.is_empty() || !unstaged.is_empty() || !untracked.is_empty();

    Ok(GitRepoStatus {
        repo_root,
        current_branch: branch,
        head_commit: head,
        ahead_count: ahead,
        behind_count: behind,
        staged,
        unstaged,
        untracked,
        has_changes,
    })
}

/// Checks repository directory ownership against current process user ID to protect against CVE-2022-24765.
#[cfg(unix)]
pub fn verify_repo_trust(repo_root: &Path) -> Result<(), GitToolingError> {
    use std::os::unix::fs::MetadataExt;

    let meta = fs::metadata(repo_root)?;
    let owner_uid = meta.uid();
    let current_uid = unsafe { libc::geteuid() };

    // Root user (0) or matching owner UID is trusted
    if current_uid == 0 || owner_uid == current_uid {
        Ok(())
    } else {
        Err(GitToolingError::UntrustedRepository(format!(
            "repository root `{}` is owned by UID {} but current process is UID {}",
            repo_root.display(),
            owner_uid,
            current_uid
        )))
    }
}

#[cfg(not(unix))]
pub fn verify_repo_trust(_repo_root: &Path) -> Result<(), GitToolingError> {
    Ok(())
}

/// Returns unified diff output for the repository.
pub fn get_git_diff(
    repo_root: &Path,
    staged: bool,
    file_path: Option<&str>,
) -> Result<String, GitToolingError> {
    let mut args = vec!["diff"];
    if staged {
        args.push("--staged");
    }
    if let Some(path) = file_path {
        args.push("--");
        args.push(path);
    }

    run_git(repo_root, &args)
}

fn parse_status_code(c: char) -> GitFileStatusKind {
    match c {
        'M' => GitFileStatusKind::Modified,
        'A' => GitFileStatusKind::Added,
        'D' => GitFileStatusKind::Deleted,
        'R' => GitFileStatusKind::Renamed,
        'C' => GitFileStatusKind::Copied,
        '?' => GitFileStatusKind::Untracked,
        other => GitFileStatusKind::Unknown(other.to_string()),
    }
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, GitToolingError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(GitToolingError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(GitToolingError::GitCommand {
            cmd: format!("git {}", args.join(" ")),
            status: output.status.code(),
            stderr,
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Restores a Git repository to a specific commit or ref.
/// Optionally creates a safety backup ref (`refs/m31a/backups/<timestamp>`).
pub fn safe_revert_to_commit(
    repo_root: &Path,
    commit_sha: &str,
    force: bool,
    backup: bool,
) -> Result<String, GitToolingError> {
    let status = get_repository_status(repo_root)?;
    if status.has_changes && !force {
        return Err(GitToolingError::GitCommand {
            cmd: "check_clean".to_string(),
            status: Some(1),
            stderr: "Working tree has uncommitted modifications. Use --force to overwrite or commit/stash them first.".to_string(),
        });
    }

    if backup {
        let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_ref = format!("refs/m31a/backups/{}", ts);
        let _ = run_git(repo_root, &["update-ref", &backup_ref, "HEAD"]);
    }

    let out = run_git(repo_root, &["checkout", commit_sha])?;
    Ok(out)
}

/// Creates a git commit with AI co-authorship metadata and session attribution.
/// Performs a regression prevention audit if the commit represents a bug fix.
pub fn create_ai_commit(
    repo_root: &Path,
    message: &str,
    session_id: Option<&str>,
    model_id: Option<&str>,
) -> Result<String, GitToolingError> {
    // Audit regression test compliance for bug fixes
    if let Ok(status) = get_repository_status(repo_root) {
        let touched: Vec<PathBuf> = status
            .staged
            .iter()
            .map(|f| PathBuf::from(&f.path))
            .collect();
        let audit = m31a_verify::regression::RegressionGuard::audit_changeset(message, &touched);
        if let m31a_verify::regression::RegressionAuditStatus::MissingTest { ref reason } =
            audit.status
        {
            eprintln!("[m31a-tools] Warning: Regression guard alert: {}", reason);
        }
    }

    let mut builder = m31a_git_attribution::CommitTrailerBuilder::new();
    if let Some(sid) = session_id {
        builder = builder.with_session_id(sid);
    }
    if let Some(mid) = model_id {
        builder = builder.with_model_id(mid);
    }
    let full_message = builder.compose_commit_message(message, None);
    run_git(repo_root, &["commit", "-m", &full_message])
}

/// Creates a git commit with strict regression test gating.
/// If `enforce_regression_test` is true and the commit indicates a bug fix without tests,
/// the commit is rejected.
pub fn create_ai_commit_guarded(
    repo_root: &Path,
    message: &str,
    session_id: Option<&str>,
    model_id: Option<&str>,
    enforce_regression_test: bool,
) -> Result<String, GitToolingError> {
    if enforce_regression_test {
        let status = get_repository_status(repo_root)?;
        let touched: Vec<PathBuf> = status
            .staged
            .iter()
            .map(|f| PathBuf::from(&f.path))
            .collect();
        let audit = m31a_verify::regression::RegressionGuard::audit_changeset(message, &touched);
        if let m31a_verify::regression::RegressionAuditStatus::MissingTest { reason } = audit.status
        {
            return Err(GitToolingError::GitCommand {
                cmd: "regression_guard".to_string(),
                status: Some(1),
                stderr: format!("Regression Guard: Commit rejected. {}", reason),
            });
        }
    }
    create_ai_commit(repo_root, message, session_id, model_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_find_git_repo_root_in_current() {
        let root = find_git_repo_root(Path::new("."));
        assert!(root.is_some());
    }

    #[test]
    fn test_ensure_git_repository_non_git() {
        let tmp = tempdir().unwrap();
        let res = ensure_git_repository(tmp.path());
        assert!(matches!(res, Err(GitToolingError::NotAGitRepository(_))));
    }

    #[test]
    fn test_status_code_parsing() {
        assert_eq!(parse_status_code('M'), GitFileStatusKind::Modified);
        assert_eq!(parse_status_code('A'), GitFileStatusKind::Added);
        assert_eq!(parse_status_code('D'), GitFileStatusKind::Deleted);
        assert_eq!(parse_status_code('R'), GitFileStatusKind::Renamed);
        assert_eq!(parse_status_code('?'), GitFileStatusKind::Untracked);
    }

    #[test]
    fn test_repo_trust_check_on_current_repo() {
        let root = find_git_repo_root(Path::new(".")).unwrap();
        let trust = verify_repo_trust(&root);
        assert!(trust.is_ok());
    }

    #[test]
    fn test_get_repository_status_current() {
        let status = get_repository_status(Path::new(".")).expect("repo status");
        assert!(status.current_branch.is_some());
        assert!(status.head_commit.is_some());
    }
}
