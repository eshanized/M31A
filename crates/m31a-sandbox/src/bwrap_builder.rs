//! Bubblewrap argument generator and filesystem sandbox builder.
//!
//! Bubblewrap sandbox filesystem architecture:
//! - Default read-only platform or root mounts
//! - Layered writable workspace roots
//! - Mandatory secret and credential masking (tmpfs/null) for sensitive metadata (`.git/hooks`, `.m31a`, `.codex`, `.ssh`, `.agents`)
//! - Path denies / masking with 0000 permissions empty tmpfs mounts
//! - WSL interop / WSLg distro masking
//! - IPC, PID, UTS, and Network namespace unsharing

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// System binaries and shared libraries kept readable when constructing a restricted view.
pub const LINUX_PLATFORM_DEFAULT_READ_ROOTS: &[&str] = &[
    "/bin",
    "/sbin",
    "/usr",
    "/etc",
    "/lib",
    "/lib64",
    "/nix/store",
    "/run/current-system/sw",
];

pub const WSL_INTEROP_DIR: &str = "/run/WSL";
pub const WSLG_DISTRO_ROOT: &str = "/mnt/wslg/distro";

/// Sensitive directory basenames that must be masked inside writable roots.
pub const PROTECTED_METADATA_NAMES: &[&str] = &[
    ".git/hooks",
    ".ssh",
    ".gnupg",
    ".aws",
    ".m31a",
    ".codex",
    ".agents",
];

/// Synthetic mount target representing a protected path that may not yet exist on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticMountTarget {
    pub path: PathBuf,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BwrapNetworkMode {
    FullAccess,
    Isolated,
    ProxyBridge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BwrapOptions {
    pub mount_proc: bool,
    pub inherit_pid_namespace: bool,
    pub network_mode: BwrapNetworkMode,
    pub mask_wsl_interop: bool,
    pub mask_wslg_distro: bool,
    pub die_with_parent: bool,
}

impl Default for BwrapOptions {
    fn default() -> Self {
        Self {
            mount_proc: true,
            inherit_pid_namespace: false,
            network_mode: BwrapNetworkMode::FullAccess,
            mask_wsl_interop: false,
            mask_wslg_distro: false,
            die_with_parent: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BwrapMountPlan {
    pub ro_binds: BTreeSet<PathBuf>,
    pub rw_binds: BTreeSet<PathBuf>,
    pub tmpfs_masks: BTreeSet<PathBuf>,
    pub file_masks: BTreeSet<PathBuf>,
    pub synthetic_mounts: Vec<SyntheticMountTarget>,
    pub fd_mounts: Vec<(i32, PathBuf)>,
}

impl BwrapMountPlan {
    pub fn new() -> Self {
        Self {
            ro_binds: BTreeSet::new(),
            rw_binds: BTreeSet::new(),
            tmpfs_masks: BTreeSet::new(),
            file_masks: BTreeSet::new(),
            synthetic_mounts: Vec::new(),
            fd_mounts: Vec::new(),
        }
    }

    /// Adds a read-only bind mount.
    pub fn add_ro_bind(&mut self, path: impl Into<PathBuf>) {
        self.ro_binds.insert(path.into());
    }

    /// Adds a read-write bind mount. Also automatically protects any nested sensitive metadata,
    /// creating synthetic mount targets if they do not exist yet to prevent metadata injection attacks.
    pub fn add_rw_bind(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        for meta in PROTECTED_METADATA_NAMES {
            let meta_path = path.join(meta);
            if meta_path.exists() {
                self.tmpfs_masks.insert(meta_path);
            } else {
                // Synthetic mount target: seal uncreated metadata paths
                // from child creation/hook injection attacks.
                self.synthetic_mounts.push(SyntheticMountTarget {
                    path: meta_path,
                    is_directory: true,
                });
            }
        }
        self.rw_binds.insert(path);
    }

    /// Masks a path with a blank 0000-permission tmpfs mount.
    pub fn add_mask(&mut self, path: impl Into<PathBuf>) {
        self.tmpfs_masks.insert(path.into());
    }

    /// Masks a single file with /dev/null.
    pub fn add_file_mask(&mut self, path: impl Into<PathBuf>) {
        self.file_masks.insert(path.into());
    }

    /// Adds a descriptor-backed mount.
    pub fn add_fd_mount(&mut self, fd: i32, destination: impl Into<PathBuf>) {
        self.fd_mounts.push((fd, destination.into()));
    }

    /// Marks specific paths within the sandbox as read-only (e.g. user-locked files).
    pub fn protect_paths<I, P>(&mut self, paths: I)
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        for p in paths {
            self.ro_binds.insert(p.into());
        }
    }
}

impl Default for BwrapMountPlan {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructs the list of command-line arguments to pass to `bwrap`.
pub fn build_bwrap_arguments(
    plan: &BwrapMountPlan,
    options: &BwrapOptions,
    command: &[String],
) -> Vec<String> {
    let mut args = Vec::new();

    // Standard unshares
    args.push("--unshare-user".to_string());
    args.push("--unshare-ipc".to_string());
    if !options.inherit_pid_namespace {
        args.push("--unshare-pid".to_string());
    }
    if options.network_mode == BwrapNetworkMode::Isolated {
        args.push("--unshare-net".to_string());
    }

    if options.die_with_parent {
        args.push("--die-with-parent".to_string());
    }

    // Devices & proc
    args.push("--dev".to_string());
    args.push("/dev".to_string());

    if options.mount_proc {
        args.push("--proc".to_string());
        args.push("/proc".to_string());
    }

    // Default ro mounts
    for ro in &plan.ro_binds {
        args.push("--ro-bind".to_string());
        args.push(ro.to_string_lossy().to_string());
        args.push(ro.to_string_lossy().to_string());
    }

    // Writable mounts
    for rw in &plan.rw_binds {
        args.push("--bind".to_string());
        args.push(rw.to_string_lossy().to_string());
        args.push(rw.to_string_lossy().to_string());
    }

    // Synthetic mounts (mask uncreated metadata like .git/hooks, .agents, .m31a)
    for syn in &plan.synthetic_mounts {
        if syn.is_directory {
            args.push("--dir".to_string());
            args.push(syn.path.to_string_lossy().to_string());
            args.push("--perms".to_string());
            args.push("0000".to_string());
            args.push("--tmpfs".to_string());
            args.push(syn.path.to_string_lossy().to_string());
        } else {
            args.push("--ro-bind".to_string());
            args.push("/dev/null".to_string());
            args.push(syn.path.to_string_lossy().to_string());
        }
    }

    // Tmpfs masks (deny rules with permission 0000)
    for mask in &plan.tmpfs_masks {
        args.push("--perms".to_string());
        args.push("0000".to_string());
        args.push("--tmpfs".to_string());
        args.push(mask.to_string_lossy().to_string());
    }

    // File masks (/dev/null mounts)
    for file in &plan.file_masks {
        args.push("--ro-bind".to_string());
        args.push("/dev/null".to_string());
        args.push(file.to_string_lossy().to_string());
    }

    // Descriptor mounts
    for (fd, dest) in &plan.fd_mounts {
        args.push(format!("--file-fd={fd}"));
        args.push(dest.to_string_lossy().to_string());
    }

    // WSL masking if requested
    if options.mask_wsl_interop && Path::new(WSL_INTEROP_DIR).exists() {
        args.push("--tmpfs".to_string());
        args.push(WSL_INTEROP_DIR.to_string());
    }
    if options.mask_wslg_distro && Path::new(WSLG_DISTRO_ROOT).exists() {
        args.push("--tmpfs".to_string());
        args.push(WSLG_DISTRO_ROOT.to_string());
    }

    // Trailing command
    args.push("--".to_string());
    args.extend(command.iter().cloned());

    args
}

/// Canonical sandbox specification and plan that produces identical
/// process isolation arguments across both Tokio and standard-library command builders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSandboxPlan {
    pub bwrap_binary: PathBuf,
    pub args: Vec<String>,
}

impl CanonicalSandboxPlan {
    /// Builds the validated canonical Bubblewrap sandbox plan.
    #[allow(clippy::too_many_arguments, clippy::vec_init_then_push)]
    pub fn build(
        bwrap_binary: &Path,
        program: &str,
        args: &[&str],
        cwd: &Path,
        policy: m31a_core::SandboxPolicy,
        network_mode: BwrapNetworkMode,
        extra_ro_paths: &[PathBuf],
        hardened_env: &HashMap<String, String>,
    ) -> anyhow::Result<Self> {
        let abs_cwd = if cwd.is_absolute() {
            cwd.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("/"))
                .join(cwd)
        };
        let abs_cwd = std::fs::canonicalize(&abs_cwd).unwrap_or(abs_cwd);
        let cwd = &abs_cwd;

        let mut bwrap_args = Vec::new();

        // 1. Core security unshares
        bwrap_args.push("--unshare-user".to_string());
        bwrap_args.push("--unshare-ipc".to_string());
        bwrap_args.push("--unshare-pid".to_string());
        bwrap_args.push("--unshare-uts".to_string());
        bwrap_args.push("--die-with-parent".to_string());

        // 2. Base platform mounts
        bwrap_args.push("--ro-bind".to_string());
        bwrap_args.push("/".to_string());
        bwrap_args.push("/".to_string());
        bwrap_args.push("--dev".to_string());
        bwrap_args.push("/dev".to_string());
        bwrap_args.push("--proc".to_string());
        bwrap_args.push("/proc".to_string());
        bwrap_args.push("--tmpfs".to_string());
        bwrap_args.push("/tmp".to_string());

        // 3. Network isolation policy
        match policy {
            m31a_core::SandboxPolicy::ReadOnly => {
                bwrap_args.push("--unshare-net".to_string());
            }
            m31a_core::SandboxPolicy::WorkspaceWrite => {
                if network_mode == BwrapNetworkMode::Isolated {
                    bwrap_args.push("--unshare-net".to_string());
                }
            }
            m31a_core::SandboxPolicy::DangerFullAccess => {}
        }

        // 4. Workspace filesystem binding
        let cwd_str = cwd.to_string_lossy().to_string();
        match policy {
            m31a_core::SandboxPolicy::ReadOnly => {
                bwrap_args.push("--ro-bind".to_string());
                bwrap_args.push(cwd_str.clone());
                bwrap_args.push(cwd_str.clone());
            }
            m31a_core::SandboxPolicy::WorkspaceWrite => {
                bwrap_args.push("--bind".to_string());
                bwrap_args.push(cwd_str.clone());
                bwrap_args.push(cwd_str.clone());
            }
            m31a_core::SandboxPolicy::DangerFullAccess => {}
        }

        // 5. User-configured extra read-only paths:
        // Must be bound BEFORE security masks are applied so extra read-only mounts
        // can never override or expose protected resource masks.
        for path in extra_ro_paths {
            if path.exists() {
                let p_str = path.to_string_lossy().to_string();
                bwrap_args.push("--ro-bind".to_string());
                bwrap_args.push(p_str.clone());
                bwrap_args.push(p_str);
            }
        }

        // 6. Host & workspace credential and socket masking (applied in both ReadOnly and WorkspaceWrite)
        // Masks are placed after all ro-binds and rw-binds to guarantee they cannot be unmasked.
        if policy != m31a_core::SandboxPolicy::DangerFullAccess {
            crate::daemon_isolation::DaemonIsolation::append_bwrap_args(&mut bwrap_args, cwd);
        }

        // 7. Synthetic mounts for sensitive subpaths inside workspace to prevent creation/injection
        if policy == m31a_core::SandboxPolicy::WorkspaceWrite {
            let sensitive_subdirs = [
                ".ssh",
                ".gnupg",
                ".aws",
                ".m31a",
                ".codex",
                ".agents",
                ".git/hooks",
            ];
            for sub in sensitive_subdirs {
                let subpath = cwd.join(sub);
                // In git worktrees or submodules, .git is a pointer file rather than a directory.
                // Creating a directory inside a file parent causes bwrap to fail with "Not a directory".
                if let Some(parent) = subpath.parent()
                    && parent.is_file()
                {
                    continue;
                }
                if !subpath.exists() {
                    let s = subpath.to_string_lossy().to_string();
                    bwrap_args.push("--dir".to_string());
                    bwrap_args.push(s.clone());
                    bwrap_args.push("--perms".to_string());
                    bwrap_args.push("0000".to_string());
                    bwrap_args.push("--tmpfs".to_string());
                    bwrap_args.push(s);
                }
            }
        }

        // 8. WSL Interop masking
        if Path::new(WSL_INTEROP_DIR).exists() {
            bwrap_args.push("--tmpfs".to_string());
            bwrap_args.push(WSL_INTEROP_DIR.to_string());
        }
        if Path::new(WSLG_DISTRO_ROOT).exists() {
            bwrap_args.push("--tmpfs".to_string());
            bwrap_args.push(WSLG_DISTRO_ROOT.to_string());
        }

        // 8. Sanitized environment
        bwrap_args.push("--clearenv".to_string());
        for (k, v) in hardened_env {
            bwrap_args.push("--setenv".to_string());
            bwrap_args.push(k.clone());
            bwrap_args.push(v.clone());
        }

        // 9. Working directory and trailing command
        bwrap_args.push("--chdir".to_string());
        bwrap_args.push(cwd_str);
        bwrap_args.push("--".to_string());
        bwrap_args.push(program.to_string());
        for arg in args {
            bwrap_args.push(arg.to_string());
        }

        Ok(Self {
            bwrap_binary: bwrap_binary.to_path_buf(),
            args: bwrap_args,
        })
    }

    /// Spawns a configured Tokio Command from this plan.
    pub fn to_tokio_command(&self) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&self.bwrap_binary);
        cmd.args(&self.args);
        cmd
    }

    /// Spawns a configured standard library Command from this plan.
    pub fn to_std_command(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new(&self.bwrap_binary);
        cmd.args(&self.args);
        cmd
    }

    /// Returns the exact arguments generated for bwrap.
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

/// Checks if any component of `path` is a symlink pointing across writable boundaries.
/// Returns `Some(symlink_path)` if a crossing symlink is detected.
pub fn first_writable_symlink_component_in_path(path: &Path) -> Option<PathBuf> {
    let mut current = PathBuf::new();
    for comp in path.components() {
        current.push(comp);
        if let Ok(metadata) = std::fs::symlink_metadata(&current)
            && metadata.file_type().is_symlink()
        {
            return Some(current);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_mount_targets_generation() {
        let mut plan = BwrapMountPlan::new();
        let workspace = PathBuf::from("/tmp/nonexistent_test_workspace_dir_12345");
        plan.add_rw_bind(&workspace);

        assert!(!plan.synthetic_mounts.is_empty());
        let ssh_meta = workspace.join(".ssh");
        assert!(
            plan.synthetic_mounts
                .iter()
                .any(|m| m.path == ssh_meta && m.is_directory)
        );

        let options = BwrapOptions::default();
        let cmd = vec!["ls".to_string()];
        let args = build_bwrap_arguments(&plan, &options, &cmd);

        assert!(args.contains(&"--perms".to_string()));
        assert!(args.contains(&"0000".to_string()));
        assert!(args.contains(&"--tmpfs".to_string()));
    }

    #[test]
    fn test_bwrap_argument_generation() {
        let mut plan = BwrapMountPlan::new();
        plan.add_ro_bind("/usr");
        plan.add_rw_bind("/home/user/workspace");
        plan.add_mask("/home/user/.ssh");
        plan.add_file_mask("/home/user/workspace/.env");
        plan.add_fd_mount(7, "/etc/resolv.conf");

        let options = BwrapOptions {
            network_mode: BwrapNetworkMode::Isolated,
            mask_wsl_interop: true,
            ..Default::default()
        };

        let cmd = vec!["echo".to_string(), "hello".to_string()];
        let args = build_bwrap_arguments(&plan, &options, &cmd);

        assert!(args.contains(&"--unshare-net".to_string()));
        assert!(args.contains(&"--unshare-user".to_string()));
        assert!(args.contains(&"--die-with-parent".to_string()));
        assert!(args.contains(&"--proc".to_string()));
        assert!(args.contains(&"/home/user/workspace".to_string()));
        assert!(args.contains(&"/home/user/.ssh".to_string()));
        assert!(args.contains(&"/home/user/workspace/.env".to_string()));
        assert!(args.contains(&"--file-fd=7".to_string()));
        assert_eq!(&args[args.len() - 3..], &["--", "echo", "hello"]);
    }

    #[test]
    fn test_canonical_sandbox_plan_equivalence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bwrap = PathBuf::from("/usr/bin/bwrap");
        let cwd = temp_dir.path().to_path_buf();
        let env = HashMap::new();
        let plan = CanonicalSandboxPlan::build(
            &bwrap,
            "sh",
            &["-c", "echo test"],
            &cwd,
            m31a_core::SandboxPolicy::ReadOnly,
            BwrapNetworkMode::Isolated,
            &[],
            &env,
        )
        .unwrap();

        // Must unshare net on ReadOnly
        assert!(plan.args.contains(&"--unshare-net".to_string()));
        // Must contain base mounts
        assert!(plan.args.contains(&"--ro-bind".to_string()));
        assert!(plan.args.contains(&"/".to_string()));
    }
}
