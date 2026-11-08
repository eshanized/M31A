//! Daemon socket directory isolation and masking rules for containerized execution.
//!
//! Restricts sandboxed access to host daemon sockets and administrative IPC endpoints.

use std::path::{Path, PathBuf};
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskType {
    DirectoryTmpfs(PathBuf),
    FileDevNull(PathBuf),
}

/// Discovers host security-sensitive paths and daemon communication endpoints
/// that must be masked inside the sandbox namespace.
pub struct DaemonIsolation;

impl DaemonIsolation {
    /// Collects all host paths that must be masked to prevent sandbox breakout or credential exfiltration.
    ///
    /// IMPORTANT: A read-only mount is NOT secret protection. Sensitive paths are masked
    /// with empty tmpfs mounts or /dev/null regardless of whether they reside on the host
    /// or inside the workspace directory.
    pub fn get_security_masks(workspace_cwd: &Path) -> Vec<MaskType> {
        let mut masks = Vec::new();
        let home_path = match std::env::var("HOME") {
            Ok(h) if !h.trim().is_empty() && h != "." => {
                let p = PathBuf::from(h);
                if p.is_absolute() {
                    p
                } else {
                    std::env::current_dir().unwrap_or_default().join(p)
                }
            }
            _ => PathBuf::from("/tmp"),
        };
        let home_path = std::fs::canonicalize(&home_path).unwrap_or(home_path);

        let abs_workspace = if workspace_cwd.is_absolute() {
            workspace_cwd.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_default()
                .join(workspace_cwd)
        };
        let abs_workspace = std::fs::canonicalize(&abs_workspace).unwrap_or(abs_workspace);
        let workspace_cwd = &abs_workspace;

        // Helper to register masks for canonical target if existing.
        // We canonicalize all paths so bwrap always mounts onto real directories/files
        // rather than symlinks or paths with symlinked ancestors (like /var/run -> /run),
        // which causes bwrap to fail with "Can't mount on symlink destination" or "Can't mkdir".
        let add_dir_mask = |dir: PathBuf, target_masks: &mut Vec<MaskType>| {
            let abs_dir = if dir.is_absolute() {
                dir
            } else {
                std::env::current_dir().unwrap_or_default().join(dir)
            };
            if abs_dir.is_dir() {
                let target = abs_dir.canonicalize().unwrap_or(abs_dir);
                if target.is_dir() {
                    target_masks.push(MaskType::DirectoryTmpfs(target));
                }
            }
        };

        let add_file_mask = |file: PathBuf, target_masks: &mut Vec<MaskType>| {
            let abs_file = if file.is_absolute() {
                file
            } else {
                std::env::current_dir().unwrap_or_default().join(file)
            };
            if abs_file.exists() {
                if abs_file.is_dir() {
                    add_dir_mask(abs_file, target_masks);
                } else {
                    let target = abs_file.canonicalize().unwrap_or(abs_file);
                    if target.is_dir() {
                        target_masks.push(MaskType::DirectoryTmpfs(target));
                    } else {
                        target_masks.push(MaskType::FileDevNull(target));
                    }
                }
            }
        };

        // 1. Host credential & key stores (directories)
        let credential_dirs = [
            home_path.join(".ssh"),
            home_path.join(".gnupg"),
            home_path.join(".aws"),
            home_path.join(".azure"),
            home_path.join(".config").join("gcloud"),
            home_path.join(".config").join("gh"),
            home_path.join(".docker"),
            home_path.join(".kube"),
            home_path.join(".config").join("m31a"),
            home_path.join(".m31a"),
            home_path.join(".codex"),
            PathBuf::from("/var/run/containerd"),
        ];

        for p in credential_dirs {
            add_dir_mask(p, &mut masks);
        }

        // 2. Sensitive host sockets / files
        let uid = unsafe { libc::getuid() };
        let run_user = PathBuf::from(format!("/run/user/{}", uid));
        let sensitive_files = [
            PathBuf::from("/var/run/docker.sock"),
            PathBuf::from("/run/docker.sock"),
            PathBuf::from("/var/run/podman/podman.sock"),
            PathBuf::from("/run/podman/podman.sock"),
            run_user.join("docker.sock"),
            run_user.join("podman").join("podman.sock"),
            home_path.join(".docker").join("run").join("docker.sock"),
            home_path
                .join(".config")
                .join("m31a")
                .join("m31a-daemon.sock"),
            home_path
                .join(".config")
                .join("m31a")
                .join("m31a-daemon.pid"),
            home_path.join(".config").join("m31a").join("state.db"),
            home_path.join(".m31a").join("m31a-daemon.sock"),
            home_path.join(".m31a").join("state.db"),
        ];

        for p in sensitive_files {
            add_file_mask(p, &mut masks);
        }

        // 3. Workspace-internal secrets and protected paths
        if workspace_cwd.is_dir() {
            // Mask workspace .env* files and directories (covers .env, .env.*, .envrc, .env_*, etc.)
            if let Ok(entries) = std::fs::read_dir(workspace_cwd) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if file_name == ".env" || file_name.starts_with(".env") {
                        let path = entry.path();
                        if path.is_dir() {
                            add_dir_mask(path, &mut masks);
                        } else {
                            add_file_mask(path, &mut masks);
                        }
                    }
                }
            }

            // Check git worktree / submodule .git pointer file
            let dot_git = workspace_cwd.join(".git");
            if dot_git.is_file() {
                if let Ok(content) = std::fs::read_to_string(&dot_git)
                    && let Some(target) = content.trim().strip_prefix("gitdir:")
                {
                    let target_trimmed = target.trim();
                    let git_dir = if Path::new(target_trimmed).is_absolute() {
                        PathBuf::from(target_trimmed)
                    } else {
                        workspace_cwd.join(target_trimmed)
                    };
                    let hooks_dir = git_dir.join("hooks");
                    add_dir_mask(hooks_dir, &mut masks);

                    // Also check common-dir if present in worktree gitdir
                    let commondir_file = git_dir.join("commondir");
                    if commondir_file.is_file()
                        && let Ok(common_content) = std::fs::read_to_string(&commondir_file)
                    {
                        let common_trimmed = common_content.trim();
                        let common_dir = if Path::new(common_trimmed).is_absolute() {
                            PathBuf::from(common_trimmed)
                        } else {
                            git_dir.join(common_trimmed)
                        };
                        let common_hooks = common_dir.join("hooks");
                        add_dir_mask(common_hooks, &mut masks);
                    }
                }
            } else if dot_git.is_symlink()
                && let Ok(canon) = dot_git.canonicalize()
            {
                let hooks = canon.join("hooks");
                add_dir_mask(hooks, &mut masks);
            }

            // Mask workspace nested credential/metadata directories if present
            let workspace_sensitive_dirs = [
                workspace_cwd.join(".ssh"),
                workspace_cwd.join(".gnupg"),
                workspace_cwd.join(".aws"),
                workspace_cwd.join(".m31a"),
                workspace_cwd.join(".codex"),
                workspace_cwd.join(".agents"),
                workspace_cwd.join(".git").join("hooks"),
            ];

            for dir in workspace_sensitive_dirs {
                add_dir_mask(dir, &mut masks);
            }
        }

        // 4. WSL Interop if present
        let wsl_run = PathBuf::from("/run/WSL");
        if wsl_run.is_dir() {
            add_dir_mask(wsl_run, &mut masks);
        }

        // Deduplicate masks preserving insertion order
        let mut seen = std::collections::HashSet::new();
        masks.retain(|m| match m {
            MaskType::DirectoryTmpfs(p) => seen.insert(format!("dir:{}", p.display())),
            MaskType::FileDevNull(p) => seen.insert(format!("file:{}", p.display())),
        });

        debug!(
            count = masks.len(),
            "Identified sandbox isolation security masks"
        );
        masks
    }

    /// Appends `--perms 0000 --tmpfs` and `--ro-bind /dev/null` masking arguments to the bubblewrap argument list.
    pub fn append_bwrap_args(args: &mut Vec<String>, workspace_cwd: &Path) {
        let masks = Self::get_security_masks(workspace_cwd);
        for mask in masks {
            match mask {
                MaskType::DirectoryTmpfs(dir) => {
                    let path_str = dir.to_string_lossy().to_string();
                    args.push("--perms".to_string());
                    args.push("0000".to_string());
                    args.push("--tmpfs".to_string());
                    args.push(path_str);
                }
                MaskType::FileDevNull(file) => {
                    let path_str = file.to_string_lossy().to_string();
                    args.push("--ro-bind".to_string());
                    args.push("/dev/null".to_string());
                    args.push(path_str);
                }
            }
        }
    }

    /// Appends `--tmpfs` and `--ro-bind /dev/null` masking arguments to the tokio bubblewrap command.
    pub fn apply_to_bwrap(cmd: &mut tokio::process::Command, workspace_cwd: &Path) {
        let mut args = Vec::new();
        Self::append_bwrap_args(&mut args, workspace_cwd);
        cmd.args(args);
    }

    /// Appends `--tmpfs` and `--ro-bind /dev/null` masking arguments to a standard library command.
    pub fn apply_to_std_bwrap(cmd: &mut std::process::Command, workspace_cwd: &Path) {
        let mut args = Vec::new();
        Self::append_bwrap_args(&mut args, workspace_cwd);
        cmd.args(args);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_isolation_masks() {
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path();

        // Create a fake .env and .ssh inside the workspace
        let env_file = cwd.join(".env");
        std::fs::write(&env_file, "SECRET=123").unwrap();
        let ssh_dir = cwd.join(".ssh");
        std::fs::create_dir_all(&ssh_dir).unwrap();

        let masks = DaemonIsolation::get_security_masks(cwd);

        // Verify that .env is masked with FileDevNull
        assert!(
            masks
                .iter()
                .any(|m| matches!(m, MaskType::FileDevNull(p) if p == &env_file)),
            "Workspace .env must be masked with /dev/null"
        );

        // Verify that .ssh in workspace is masked with DirectoryTmpfs
        assert!(
            masks
                .iter()
                .any(|m| matches!(m, MaskType::DirectoryTmpfs(p) if p == &ssh_dir)),
            "Workspace .ssh must be masked with tmpfs"
        );
    }
}
