//! Deny-by-default child-process environment builder and sanitizer (CTL-01, TL-02, per D-15).
//!
//! Non-negotiable invariants:
//! - Child processes NEVER inherit the host process environment wholesale.
//! - Minimal trusted baseline contains only platform/toolchain essentials.
//! - Dangerous dynamic loader variables (`LD_PRELOAD`, etc.) are stripped.
//! - Host secrets and API credentials (`NVIDIA_API_KEY`, etc.) are strictly excluded.
//! - Working directory is pinned to the authorized task workspace root.

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Standard baseline environment variable keys permitted in child processes on Unix.
pub const UNIX_BASELINE_VARS: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LC_ALL",
    "TERM",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "TMPDIR",
];

/// Standard baseline environment variable keys permitted in child processes on Windows.
pub const WINDOWS_BASELINE_VARS: &[&str] = &[
    "PATH",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "PATHEXT",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "USERNAME",
    "USERDOMAIN",
    "APPDATA",
    "LOCALAPPDATA",
    "ALLUSERSPROFILE",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "COMMONPROGRAMFILES",
    "COMMONPROGRAMFILES(X86)",
    "CARGO_HOME",
    "RUSTUP_HOME",
];

/// Retained for backwards compatibility with existing Unix-specific references.
pub const TRUSTED_BASELINE_VARS: &[&str] = UNIX_BASELINE_VARS;

/// Dangerous dynamic loader and script execution variables that must never leak.
pub const DANGEROUS_OVERRIDE_VARS: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "PYTHONPATH",
    "PYTHONHOME",
    "NODE_OPTIONS",
    "RUBYLIB",
    "RUBYOPT",
    "PERL5LIB",
    "RUSTC_WRAPPER",
    // Windows injection / profiler hijacking variables
    "__COMPAT_LAYER",
    "COR_ENABLE_PROFILING",
    "COR_PROFILER",
    "COR_PROFILER_PATH",
    "APP_POOL_ID",
];

/// Known credential and API token patterns that are strictly blocked (D-15).
const FORBIDDEN_SECRET_PATTERNS: &[&str] = &[
    "API_KEY",
    "SECRET",
    "TOKEN",
    "PASSWORD",
    "CREDENTIAL",
    "AUTH",
    "NVIDIA",
    "OPENAI",
    "AWS",
    "AZURE",
    "GCP",
    "GITHUB",
    "GITLAB",
    "DATABASE_URL",
];

/// Dangerous Git redirection environment variables that must never be inherited or set.
pub const FORBIDDEN_GIT_REDIRECT_VARS: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_EXEC_PATH",
    "GIT_CEILING_DIRECTORIES",
    "GIT_COMMON_DIR",
];

/// Deny-by-default environment builder for child process isolation (TL-02, D-15).
#[derive(Debug, Clone)]
pub struct EnvironmentBuilder {
    workspace_root: PathBuf,
    variables: HashMap<String, String>,
    blocked_keys: HashSet<String>,
    is_windows: bool,
}

impl EnvironmentBuilder {
    /// Create a new EnvironmentBuilder rooted in the authorized task workspace.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self::new_with_platform(workspace_root, cfg!(windows))
    }

    /// Create an EnvironmentBuilder with explicit platform configuration.
    /// Pure constructor enables cross-platform contract testing.
    pub fn new_with_platform(workspace_root: impl Into<PathBuf>, is_windows: bool) -> Self {
        let mut builder = Self {
            workspace_root: workspace_root.into(),
            variables: HashMap::new(),
            blocked_keys: HashSet::new(),
            is_windows,
        };

        if is_windows {
            // Case-insensitive lookup of Windows baseline variables from host environment
            let host_env: HashMap<String, String> = env::vars().collect();
            for (k, v) in host_env {
                let upper = k.to_ascii_uppercase();
                if WINDOWS_BASELINE_VARS.contains(&upper.as_str()) && !builder.is_forbidden_key(&k)
                {
                    builder.variables.insert(k, v);
                }
            }

            // Ensure critical Windows variables have safe defaults if not in host environment
            let has_key = |vars: &HashMap<String, String>, name: &str| {
                vars.keys().any(|k| k.eq_ignore_ascii_case(name))
            };
            if !has_key(&builder.variables, "PATH") {
                builder.variables.insert(
                    "PATH".to_string(),
                    crate::platform::filesystem::HostFilesystem::default_path_value(),
                );
            }
            if !has_key(&builder.variables, "SystemRoot") {
                builder
                    .variables
                    .insert("SystemRoot".to_string(), "C:\\Windows".to_string());
            }
            if !has_key(&builder.variables, "ComSpec") {
                builder.variables.insert(
                    "ComSpec".to_string(),
                    "C:\\Windows\\System32\\cmd.exe".to_string(),
                );
            }
            if !has_key(&builder.variables, "PATHEXT") {
                builder.variables.insert(
                    "PATHEXT".to_string(),
                    ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC".to_string(),
                );
            }
            if !has_key(&builder.variables, "TEMP") {
                let tmp = crate::platform::filesystem::HostFilesystem::temp_root();
                builder
                    .variables
                    .insert("TEMP".to_string(), tmp.to_string_lossy().to_string());
            }
            if !has_key(&builder.variables, "TMP") {
                let tmp = crate::platform::filesystem::HostFilesystem::temp_root();
                builder
                    .variables
                    .insert("TMP".to_string(), tmp.to_string_lossy().to_string());
            }
        } else {
            // Populate minimal trusted baseline from host environment where available
            for &key in UNIX_BASELINE_VARS {
                if let Ok(val) = env::var(key)
                    && !builder.is_forbidden_key(key)
                {
                    builder.variables.insert(key.to_string(), val);
                }
            }

            // Ensure reasonable defaults for critical variables if missing
            if !builder.variables.contains_key("PATH") {
                builder.variables.insert(
                    "PATH".to_string(),
                    crate::platform::filesystem::HostFilesystem::default_path_value(),
                );
            }
            if !builder.variables.contains_key("TMPDIR") {
                let tmp = crate::platform::filesystem::HostFilesystem::temp_root();
                builder
                    .variables
                    .insert("TMPDIR".to_string(), tmp.to_string_lossy().to_string());
            }
        }

        builder
    }

    /// Access the target workspace root.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Set or override an explicit authorized environment variable.
    /// Fails closed if variable matches forbidden credential patterns or dangerous overrides.
    pub fn set_var(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<&mut Self, String> {
        let key_str = key.into();
        let val_str = value.into();

        if self.is_forbidden_key(&key_str) {
            return Err(format!(
                "Environment variable '{}' matches forbidden credential pattern or loader override",
                key_str
            ));
        }

        if self.is_windows {
            if let Some(existing) = self
                .variables
                .keys()
                .find(|k| k.eq_ignore_ascii_case(&key_str))
                .cloned()
            {
                self.variables.remove(&existing);
            }
        }

        self.variables.insert(key_str, val_str);
        Ok(self)
    }

    /// Check if a variable key is forbidden by safety rules.
    pub fn is_forbidden_key(&self, key: &str) -> bool {
        let upper = key.to_ascii_uppercase();

        // 1. Dangerous loader variables
        for dangerous in DANGEROUS_OVERRIDE_VARS {
            if upper == *dangerous {
                return true;
            }
        }

        // 2. Secret and credential patterns (specifically NVIDIA_API_KEY, AWS, etc.)
        for pattern in FORBIDDEN_SECRET_PATTERNS {
            if upper.contains(pattern) {
                return true;
            }
        }

        // 3. Git repository and worktree redirection variables
        for redirect_var in FORBIDDEN_GIT_REDIRECT_VARS {
            if upper == *redirect_var {
                return true;
            }
        }

        if self.is_windows {
            self.blocked_keys
                .iter()
                .any(|b| b.eq_ignore_ascii_case(key))
        } else {
            self.blocked_keys.contains(key)
        }
    }

    /// Explicitly block an environment variable name.
    pub fn block_key(&mut self, key: impl Into<String>) -> &mut Self {
        let k = key.into();
        if self.is_windows {
            if let Some(existing) = self
                .variables
                .keys()
                .find(|ek| ek.eq_ignore_ascii_case(&k))
                .cloned()
            {
                self.variables.remove(&existing);
            }
        } else {
            self.variables.remove(&k);
        }
        self.blocked_keys.insert(k);
        self
    }

    /// Snapshot the built sanitized environment map.
    pub fn build_map(&self) -> HashMap<String, String> {
        self.variables.clone()
    }

    /// Apply the sanitized environment and working directory to a `tokio::process::Command`.
    /// Strips all inherited host variables via `env_clear()`.
    pub fn apply(&self, cmd: &mut Command) {
        cmd.env_clear();
        cmd.envs(&self.variables);
        cmd.current_dir(&self.workspace_root);
    }
}

/// Security violation detected during process or shell command inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessSecurityViolation {
    ProtectedPathAccess(String),
    GitRedirection(String),
    WorkspaceEscape(String),
}

impl std::fmt::Display for ProcessSecurityViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProtectedPathAccess(msg) => write!(f, "access to protected path denied: {msg}"),
            Self::GitRedirection(msg) => write!(f, "git repository redirection denied: {msg}"),
            Self::WorkspaceEscape(msg) => write!(f, "workspace boundary escape denied: {msg}"),
        }
    }
}

impl std::error::Error for ProcessSecurityViolation {}

/// Validate whether a command line or argument list attempts to access protected paths or redirect Git.
pub fn check_command_safety(
    program: &str,
    args: &[String],
) -> Result<(), ProcessSecurityViolation> {
    let prog_path = Path::new(program);
    if crate::kernel::invariants::contains_protected_component(prog_path) {
        return Err(ProcessSecurityViolation::ProtectedPathAccess(
            program.to_string(),
        ));
    }

    let is_shell = crate::platform::shell::is_shell_program(program);

    for (idx, arg) in args.iter().enumerate() {
        let arg_trimmed = arg.trim();
        if arg_trimmed.starts_with("--git-dir")
            || arg_trimmed.starts_with("--work-tree")
            || arg_trimmed.starts_with("--separate-git-dir")
            || arg_trimmed.contains("core.gitDir")
            || arg_trimmed.contains("core.worktree")
            || arg_trimmed.starts_with("GIT_DIR=")
            || arg_trimmed.starts_with("GIT_WORK_TREE=")
            || arg_trimmed.starts_with("GIT_INDEX_FILE=")
        {
            return Err(ProcessSecurityViolation::GitRedirection(arg.clone()));
        }

        let prev = if idx > 0 { args[idx - 1].as_str() } else { "" };
        if is_shell
            && idx > 0
            && (prev == "-c"
                || prev == "-lc"
                || prev.eq_ignore_ascii_case("/c")
                || prev.eq_ignore_ascii_case("/k")
                || prev.eq_ignore_ascii_case("-command")
                || prev.eq_ignore_ascii_case("--command"))
        {
            tokenize_and_validate_shell_string(arg_trimmed)?;
            continue;
        }

        let clean_arg = arg_trimmed.trim_matches(|c| c == '\'' || c == '"');
        if crate::kernel::invariants::contains_protected_component(Path::new(clean_arg)) {
            return Err(ProcessSecurityViolation::ProtectedPathAccess(arg.clone()));
        }
    }

    Ok(())
}

/// Tokenize and inspect shell string for protected paths and Git redirection.
pub fn tokenize_and_validate_shell_string(shell_str: &str) -> Result<(), ProcessSecurityViolation> {
    for token in shell_str.split(|c: char| {
        c.is_whitespace()
            || c == ';'
            || c == '&'
            || c == '|'
            || c == '>'
            || c == '<'
            || c == '('
            || c == ')'
            || c == '{'
            || c == '}'
            || c == '`'
    }) {
        let t = token.trim().trim_matches(|c| c == '\'' || c == '"');
        if t.is_empty() {
            continue;
        }

        if t.starts_with("--git-dir")
            || t.starts_with("--work-tree")
            || t.starts_with("--separate-git-dir")
            || t.contains("core.gitDir")
            || t.contains("core.worktree")
            || t.starts_with("GIT_DIR=")
            || t.starts_with("GIT_WORK_TREE=")
            || t.starts_with("GIT_INDEX_FILE=")
        {
            return Err(ProcessSecurityViolation::GitRedirection(t.to_string()));
        }

        let path = Path::new(t);
        if crate::kernel::invariants::contains_protected_component(path) {
            return Err(ProcessSecurityViolation::ProtectedPathAccess(t.to_string()));
        }
    }
    Ok(())
}

/// Validate that a working directory does not escape the workspace root and does not target protected paths.
pub fn validate_working_directory(
    working_dir: Option<&Path>,
    workspace_root: &Path,
) -> Result<PathBuf, ProcessSecurityViolation> {
    let cwd = working_dir.unwrap_or(workspace_root);
    let full = if cwd.is_relative() {
        workspace_root.join(cwd)
    } else {
        cwd.to_path_buf()
    };

    let norm = crate::policy::matcher::lexical_normalize(&full);
    let canon_ws = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| crate::policy::matcher::lexical_normalize(workspace_root));

    if !norm.starts_with(&canon_ws) && !norm.starts_with(workspace_root) {
        return Err(ProcessSecurityViolation::WorkspaceEscape(format!(
            "working directory '{}' escapes workspace root '{}'",
            cwd.display(),
            workspace_root.display()
        )));
    }

    let has_protected_rel = norm
        .strip_prefix(&canon_ws)
        .or_else(|_| norm.strip_prefix(workspace_root))
        .map(crate::kernel::invariants::contains_protected_component)
        .unwrap_or(false);

    if has_protected_rel {
        return Err(ProcessSecurityViolation::ProtectedPathAccess(format!(
            "working directory '{}' targets protected path (.git or .m31a)",
            cwd.display()
        )));
    }

    if norm.exists() {
        match norm.canonicalize() {
            Ok(canon) => {
                if !canon.starts_with(&canon_ws) {
                    return Err(ProcessSecurityViolation::WorkspaceEscape(format!(
                        "working directory '{}' symlink escapes workspace root '{}'",
                        cwd.display(),
                        workspace_root.display()
                    )));
                }
                if canon
                    .strip_prefix(&canon_ws)
                    .map(crate::kernel::invariants::contains_protected_component)
                    .unwrap_or(false)
                {
                    return Err(ProcessSecurityViolation::ProtectedPathAccess(format!(
                        "working directory '{}' resolves through symlink to protected path (.git or .m31a)",
                        cwd.display()
                    )));
                }
            }
            Err(e) => {
                return Err(ProcessSecurityViolation::WorkspaceEscape(format!(
                    "working directory '{}' canonicalization failed: {e}",
                    cwd.display()
                )));
            }
        }
    }

    Ok(norm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_builder_rejects_forbidden_secrets_and_loader_vars() {
        let ws = PathBuf::from("/workspace");
        let mut builder = EnvironmentBuilder::new(&ws);

        // Setting dangerous loader variables must fail closed
        assert!(builder.set_var("LD_PRELOAD", "/lib/evil.so").is_err());
        assert!(builder.set_var("LD_LIBRARY_PATH", "/tmp").is_err());
        assert!(
            builder
                .set_var("DYLD_INSERT_LIBRARIES", "/lib/evil.dylib")
                .is_err()
        );

        // Setting secret / credential variables must fail closed
        assert!(builder.set_var("NVIDIA_API_KEY", "secret-token").is_err());
        assert!(
            builder
                .set_var("AWS_SECRET_ACCESS_KEY", "aws-secret")
                .is_err()
        );
        assert!(builder.set_var("GITHUB_TOKEN", "ghp_123456").is_err());
        assert!(
            builder
                .set_var("DATABASE_URL", "postgres://user:pass@localhost/db")
                .is_err()
        );

        // Setting git redirection variables must fail closed
        assert!(builder.set_var("GIT_DIR", "/evil/.git").is_err());
        assert!(builder.set_var("GIT_WORK_TREE", "/evil").is_err());

        // Setting safe variable succeeds
        assert!(builder.set_var("APP_ENV", "production").is_ok());
        let map = builder.build_map();
        assert_eq!(map.get("APP_ENV").unwrap(), "production");
    }

    #[test]
    fn test_check_command_safety_posix_and_windows_shell_flags() {
        // Direct command safety
        assert!(check_command_safety("cargo", &["test".to_string()]).is_ok());
        assert!(check_command_safety("git", &["--git-dir=/evil/.git".to_string()]).is_err());
        assert!(check_command_safety("cat", &[".git/config".to_string()]).is_err());
        assert!(check_command_safety("cat", &[".m31a/credentials".to_string()]).is_err());

        // Shell -c inspection (POSIX)
        assert!(check_command_safety("sh", &["-c".to_string(), "echo hello".to_string()]).is_ok());
        assert!(
            check_command_safety("sh", &["-c".to_string(), "cat .git/config".to_string()]).is_err()
        );
        assert!(
            check_command_safety(
                "bash",
                &["-lc".to_string(), "git --git-dir=/tmp status".to_string()]
            )
            .is_err()
        );

        // Windows shell inspection (/C, /c, /k, -Command)
        assert!(
            check_command_safety("cmd.exe", &["/c".to_string(), "echo safe".to_string()]).is_ok()
        );
        assert!(
            check_command_safety("cmd.exe", &["/C".to_string(), "type .git/HEAD".to_string()])
                .is_err()
        );
        assert!(
            check_command_safety(
                "powershell.exe",
                &[
                    "-Command".to_string(),
                    "Get-Content .git/config".to_string()
                ]
            )
            .is_err()
        );
        assert!(
            check_command_safety(
                "cmd.exe",
                &["/c".to_string(), "git --work-tree=/tmp commit".to_string()]
            )
            .is_err()
        );
    }

    #[test]
    fn test_tokenize_and_validate_shell_string() {
        assert!(tokenize_and_validate_shell_string("echo ok && ls -la").is_ok());
        assert!(tokenize_and_validate_shell_string("cat .git/config").is_err());
        assert!(tokenize_and_validate_shell_string("cargo test; cat .m31a/keys").is_err());
        assert!(tokenize_and_validate_shell_string("eval `git --git-dir=/evil status`").is_err());
    }

    #[test]
    fn test_environment_builder_windows_baseline_and_dangerous_vars() {
        let ws = PathBuf::from("C:\\workspace");
        let mut builder = EnvironmentBuilder::new_with_platform(&ws, true);
        let map = builder.build_map();

        // Must provide critical Windows defaults
        assert!(map.contains_key("SystemRoot") || map.contains_key("SYSTEMROOT"));
        assert!(map.contains_key("ComSpec") || map.contains_key("COMSPEC"));
        assert!(map.contains_key("PATHEXT"));
        assert!(map.contains_key("TEMP") || map.contains_key("TMP"));

        // Must reject Windows injection and profiler hijacking variables
        assert!(builder.set_var("__COMPAT_LAYER", "RunAsInvoker").is_err());
        assert!(builder.set_var("COR_ENABLE_PROFILING", "1").is_err());
        assert!(builder.set_var("COR_PROFILER", "{GUID}").is_err());
        assert!(builder.set_var("APP_POOL_ID", "DefaultAppPool").is_err());

        // Case-insensitive blocking and override on Windows
        builder.block_key("CUSTOM_KEY");
        assert!(builder.is_forbidden_key("custom_key"));
        assert!(builder.is_forbidden_key("CUSTOM_KEY"));
    }
}
