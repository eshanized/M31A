//! In-process Linux Landlock LSM sandboxing fallback when user namespace containers are unavailable.
//!
//! When bubblewrap is unavailable, filesystem confinement falls back to the
//! kernel's Landlock LSM. Rules are built in the parent (paths pre-opened
//! `O_PATH|O_CLOEXEC`) and enforced in the child between `fork` and `exec`
//! via `pre_exec`, so the confined process starts with the ruleset active.
//!
//! Error policy (documented deviation from upstream): a kernel without
//! Landlock support downgrades to a warning + unconfined execution — M31A's
//! pre-Landlock contract was best-effort without bwrap, and a hard failure
//! would regress existing environments. When Landlock IS supported but rule
//! creation or restriction fails, the spawn fails hard: a misconfiguration
//! must never pass silently.
//!
//! Network access rights are deliberately unhandled, so networking behavior
//! is unchanged (the network proxy is a separate layer).

use std::collections::HashSet;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::{debug, warn};

// --- UAPI constants (linux/landlock.h; not exposed by the libc crate) ---

const LANDLOCK_CREATE_RULESET_VERSION: i32 = 1 << 0;
const LANDLOCK_RULE_PATH_BENEATH: i32 = 1;

const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
const LANDLOCK_ACCESS_FS_TRANSFORM: u64 = 1 << 11;
const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 12; // ABI 2 (Linux 5.19)
const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 13; // ABI 3 (Linux 6.2)
const LANDLOCK_ACCESS_FS_IOCTL_DEV: u64 = 1 << 14; // ABI 5 (Linux 6.12)

const PR_SET_NO_NEW_PRIVS: i32 = 38;

// --- UAPI structs ---

#[repr(C)]
struct LandlockRulesetAttr {
    handled_access_fs: u64,
    // Newer UAPI fields; unused (zero) — network and scoped rights are
    // deliberately not handled by this module.
    handled_access_net: u64,
    scoped: u64,
}

/// Matches `struct landlock_path_beneath_attr` from `/usr/include/linux/landlock.h`:
/// `__u64 allowed_access; __s32 parent_fd;` — packed, 12 bytes.
#[repr(C, packed)]
struct LandlockPathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

/// Read side: everything needed to traverse, read, and execute.
const READ_SIDE_ACCESS: u64 =
    LANDLOCK_ACCESS_FS_EXECUTE | LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR;

/// Full filesystem rights handled by this module (network bits excluded).
const ALL_FS_ACCESS: u64 = READ_SIDE_ACCESS
    | LANDLOCK_ACCESS_FS_WRITE_FILE
    | LANDLOCK_ACCESS_FS_REMOVE_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_FILE
    | LANDLOCK_ACCESS_FS_MAKE_CHAR
    | LANDLOCK_ACCESS_FS_MAKE_DIR
    | LANDLOCK_ACCESS_FS_MAKE_REG
    | LANDLOCK_ACCESS_FS_MAKE_SOCK
    | LANDLOCK_ACCESS_FS_MAKE_FIFO
    | LANDLOCK_ACCESS_FS_TRANSFORM
    | LANDLOCK_ACCESS_FS_REFER
    | LANDLOCK_ACCESS_FS_TRUNCATE
    | LANDLOCK_ACCESS_FS_IOCTL_DEV;

/// Returns the highest Landlock ABI version the kernel supports (0 = none).
pub fn landlock_abi() -> u64 {
    static ABI: OnceLock<u64> = OnceLock::new();
    *ABI.get_or_init(|| {
        let v = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<LandlockRulesetAttr>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        if v < 0 { 0 } else { v as u64 }
    })
}

/// One `path_beneath` rule: allow `access` under `parent` (parent fd opened
/// O_PATH in the parent process; O_CLOEXEC so it dies at exec).
struct PathBeneathRule {
    parent_fd: i32,
    allowed_access: u64,
}

/// A fully-built ruleset: rights the sandbox handles + rules to grant.
pub struct LandlockRuleset {
    handled_access_fs: u64,
    rules: Vec<PathBeneathRule>,
}

/// Builds a Landlock ruleset for the given policy, or `None` when the policy
/// requests no confinement (DangerFullAccess) or the kernel lacks Landlock.
/// Rules only materialize for paths that exist; missing roots are warned and
/// skipped.
pub fn build_landlock_rules(
    policy: &m31a_core::SandboxPolicy,
    cwd: &Path,
) -> Option<LandlockRuleset> {
    use m31a_core::SandboxPolicy;
    if *policy == SandboxPolicy::DangerFullAccess {
        return None;
    }

    let abi = landlock_abi();
    if abi == 0 {
        warn!(
            "Kernel has no Landlock support; running without filesystem confinement \
             (install bubblewrap for full isolation)"
        );
        return None;
    }

    // Handled rights limited to what this ABI supports.
    let mut handled = ALL_FS_ACCESS;
    if abi < 2 {
        handled &= !LANDLOCK_ACCESS_FS_REFER;
    }
    if abi < 3 {
        handled &= !LANDLOCK_ACCESS_FS_TRUNCATE;
    }
    if abi < 5 {
        handled &= !LANDLOCK_ACCESS_FS_IOCTL_DEV;
    }

    let mut rules: Vec<PathBeneathRule> = Vec::new();

    match policy {
        SandboxPolicy::ReadOnly => {
            // Read + execute everywhere; no write grants at all.
            rules.push(PathBeneathRule {
                parent_fd: open_path_fd(Path::new("/")),
                allowed_access: handled & READ_SIDE_ACCESS,
            });
        }
        SandboxPolicy::WorkspaceWrite => {
            // Read + execute everywhere...
            rules.push(PathBeneathRule {
                parent_fd: open_path_fd(Path::new("/")),
                allowed_access: handled & READ_SIDE_ACCESS,
            });
            // ...read-write inside the workspace and temp dirs.
            let mut roots: Vec<PathBuf> = vec![cwd.to_path_buf()];
            if let Some(tmp) = std::env::var_os("TMPDIR") {
                roots.push(PathBuf::from(tmp));
            } else {
                roots.push(std::env::temp_dir());
            }
            let mut seen: HashSet<PathBuf> = HashSet::new();
            for root in roots {
                if !seen.insert(root.clone()) {
                    continue;
                }
                if let Some(fd) = open_path_fd_checked(&root) {
                    rules.push(PathBeneathRule {
                        parent_fd: fd,
                        allowed_access: handled,
                    });
                } else {
                    warn!(
                        "Landlock: skipping nonexistent writable root {}",
                        root.display()
                    );
                }
            }
        }
        SandboxPolicy::DangerFullAccess => unreachable!("filtered above"),
    }

    debug!(
        "Landlock ruleset: abi v{}, handled=0x{:x}, {} rule(s)",
        abi,
        handled,
        rules.len()
    );
    Some(LandlockRuleset {
        handled_access_fs: handled,
        rules,
    })
}

/// Opens a path `O_PATH|O_CLOEXEC` for a Landlock rule. Returns the raw fd,
/// which stays open across `fork` and is closed by `exec` (O_CLOEXEC).
fn open_path_fd_checked(path: &Path) -> Option<i32> {
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_PATH | libc::O_CLOEXEC | libc::O_RDONLY) };
    if fd < 0 { None } else { Some(fd) }
}

fn open_path_fd(path: &Path) -> i32 {
    open_path_fd_checked(path).unwrap_or(-1)
}

/// Enforcement (runs inside `pre_exec`, after fork, before exec):
/// creates the ruleset, adds every rule, sets `no_new_privs`, restricts self.
/// Only syscalls happen here — no allocation.
///
/// SAFETY: called from `pre_exec`; must only perform async-signal-safe
/// syscalls on the carried data.
pub(crate) unsafe fn enforce(ruleset: &LandlockRuleset) -> std::io::Result<()> {
    unsafe {
        let attr = LandlockRulesetAttr {
            handled_access_fs: ruleset.handled_access_fs,
            handled_access_net: 0,
            scoped: 0,
        };
        let ruleset_fd = libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &attr as *const LandlockRulesetAttr,
            std::mem::size_of::<LandlockRulesetAttr>(),
            0i32,
        );
        if ruleset_fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let ruleset_fd = ruleset_fd as i32;

        for rule in &ruleset.rules {
            let rule_attr = LandlockPathBeneathAttr {
                allowed_access: rule.allowed_access,
                parent_fd: rule.parent_fd,
            };
            if libc::syscall(
                libc::SYS_landlock_add_rule,
                ruleset_fd,
                LANDLOCK_RULE_PATH_BENEATH,
                &rule_attr as *const LandlockPathBeneathAttr,
                0i32,
            ) < 0
            {
                let err = std::io::Error::last_os_error();
                libc::close(ruleset_fd);
                return Err(err);
            }
        }

        if libc::prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) < 0 {
            let err = std::io::Error::last_os_error();
            libc::close(ruleset_fd);
            return Err(err);
        }

        if libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0i32) < 0 {
            let err = std::io::Error::last_os_error();
            libc::close(ruleset_fd);
            return Err(err);
        }

        // ruleset_fd is intentionally leaked: it has no O_CLOEXEC and would
        // close at exec anyway; the child execs immediately after pre_exec.
        Ok(())
    }
}

/// Builds a `pre_exec` closure from the ruleset, mapping errors into
/// `io::Error` as `CommandExt::pre_exec` requires.
pub fn pre_exec_hook(
    ruleset: LandlockRuleset,
) -> impl FnMut() -> std::io::Result<()> + Send + 'static {
    move || unsafe { enforce(&ruleset) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::Command;
    use tempfile::tempdir;

    #[test]
    fn test_landlock_abi_probe() {
        // On Linux ≥ 5.13 this is ≥ 1; the probe must never panic either way.
        let abi = landlock_abi();
        if abi == 0 {
            eprintln!("kernel without Landlock — skipping dependent assertions");
        }
    }

    #[test]
    fn test_landlock_confines_child() {
        if landlock_abi() == 0 {
            eprintln!("kernel without Landlock — skipping enforcement test");
            return;
        }
        let dir = tempdir().unwrap();

        // Write INSIDE the workspace must succeed.
        let inside_marker = dir.path().join("inside.txt");
        let mut inside = Command::new("bash");
        inside
            .arg("-c")
            .arg("echo ok > inside.txt")
            .current_dir(dir.path());
        let inside_rules =
            build_landlock_rules(&m31a_core::SandboxPolicy::WorkspaceWrite, dir.path())
                .expect("ruleset for supported kernel");
        unsafe {
            inside.pre_exec(pre_exec_hook(inside_rules));
        }
        let out = inside.output().unwrap();
        assert!(
            out.status.success(),
            "write inside workspace failed: {:?}",
            out.stderr
        );
        assert!(inside_marker.exists());

        // Write OUTSIDE the workspace (and outside the granted temp dirs) must
        // be denied. /etc is never a granted root. Caveat: as non-root this is
        // also DAC-denied, so the discriminating non-root evidence lives in
        // test_landlock_read_only_policy_blocks_writes (where DAC would allow
        // the write). As root, Landlock is the only barrier here.
        let outside = format!("/etc/m31a-landlock-test-{}", std::process::id());
        let mut outside_cmd = Command::new("bash");
        outside_cmd
            .arg("-c")
            .arg(format!("echo pwned > {outside}"))
            .current_dir(dir.path());
        let outside_rules =
            build_landlock_rules(&m31a_core::SandboxPolicy::WorkspaceWrite, dir.path())
                .expect("ruleset for supported kernel");
        unsafe {
            outside_cmd.pre_exec(pre_exec_hook(outside_rules));
        }
        let out = outside_cmd.output().unwrap();
        let _ = std::fs::remove_file(&outside); // best-effort cleanup if somehow created
        assert!(
            !out.status.success(),
            "write outside workspace unexpectedly succeeded"
        );
        assert!(!std::path::Path::new(&outside).exists());
    }

    #[test]
    fn test_landlock_read_only_policy_blocks_writes() {
        if landlock_abi() == 0 {
            eprintln!("kernel without Landlock — skipping enforcement test");
            return;
        }
        let dir = tempdir().unwrap();
        let rules = build_landlock_rules(&m31a_core::SandboxPolicy::ReadOnly, dir.path()).unwrap();

        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg("echo nope > denied.txt")
            .current_dir(dir.path());
        unsafe {
            cmd.pre_exec(pre_exec_hook(rules));
        }
        let out = cmd.output().unwrap();
        assert!(!out.status.success(), "ReadOnly policy allowed a write");
        assert!(!dir.path().join("denied.txt").exists());
    }

    #[test]
    fn test_danger_full_access_skips_landlock() {
        assert!(
            build_landlock_rules(
                &m31a_core::SandboxPolicy::DangerFullAccess,
                Path::new("/tmp")
            )
            .is_none()
        );
    }
}
