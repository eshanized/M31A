//! Executable path discovery and binary invocation argument inspection.
//!
//! Resolves the absolute filesystem path and invocation context of the running M31A executable.

use anyhow::{Context, Result};
use std::env;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tracing::debug;

pub const APPLY_PATCH_ARG0: &str = "apply_patch";
pub const MISSPELLED_APPLY_PATCH_ARG0: &str = "applypatch";
pub const LINUX_SANDBOX_ARG0: &str = "m31a-linux-sandbox";
const LOCK_FILENAME: &str = ".lock";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Arg0DispatchPaths {
    /// Stable path to the current executable for child re-execs.
    pub self_exe: Option<PathBuf>,
    /// Directory containing the arg0 dispatch symlinks.
    pub dispatch_bin_dir: Option<PathBuf>,
}

/// Keeps the per-session PATH entry alive and locked for the process lifetime.
pub struct Arg0PathEntryGuard {
    _lock_file: Option<File>,
    _temp_dir: Option<TempDir>,
    paths: Arg0DispatchPaths,
}

impl Arg0PathEntryGuard {
    pub fn new(temp_dir: TempDir, lock_file: File, paths: Arg0DispatchPaths) -> Self {
        Self {
            _temp_dir: Some(temp_dir),
            _lock_file: Some(lock_file),
            paths,
        }
    }

    pub fn noop(paths: Arg0DispatchPaths) -> Self {
        Self {
            _temp_dir: None,
            _lock_file: None,
            paths,
        }
    }

    pub fn paths(&self) -> &Arg0DispatchPaths {
        &self.paths
    }

    /// Injects the dispatch bin dir at the front of PATH for the current process.
    pub fn apply_to_current_env(&self) {
        if let Some(ref bin_dir) = self.paths.dispatch_bin_dir
            && let Ok(current_path) = env::var("PATH")
        {
            let new_path = format!("{}:{}", bin_dir.display(), current_path);
            unsafe { env::set_var("PATH", new_path) };
        }
    }
}

/// Detects whether current process was invoked as `apply_patch` or `applypatch`.
pub fn is_apply_patch_invocation() -> bool {
    if let Some(arg0) = env::args().next() {
        let path = Path::new(&arg0);
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            return stem == APPLY_PATCH_ARG0 || stem == MISSPELLED_APPLY_PATCH_ARG0;
        }
    }
    false
}

/// Detects whether current process was invoked as `m31a-linux-sandbox`.
pub fn is_linux_sandbox_invocation() -> bool {
    if let Some(arg0) = env::args().next() {
        let path = Path::new(&arg0);
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            return stem == LINUX_SANDBOX_ARG0;
        }
    }
    false
}

/// Sets up the dispatch directory with symlinks for `apply_patch` and `m31a-linux-sandbox`.
pub fn setup_arg0_dispatch() -> Result<Arg0PathEntryGuard> {
    let self_exe = env::current_exe()
        .ok()
        .and_then(|p| fs::canonicalize(&p).ok().or(Some(p)));

    let Some(ref exe_path) = self_exe else {
        return Ok(Arg0PathEntryGuard::noop(Arg0DispatchPaths::default()));
    };

    let temp_dir = tempfile::Builder::new()
        .prefix("m31a-arg0-")
        .tempdir()
        .context("Failed to create temporary arg0 directory")?;

    let lock_path = temp_dir.path().join(LOCK_FILENAME);
    let lock_file = File::create(&lock_path).context("Failed to create arg0 lock file")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let patch_link = temp_dir.path().join(APPLY_PATCH_ARG0);
        let _ = symlink(exe_path, &patch_link);

        let misspelled_link = temp_dir.path().join(MISSPELLED_APPLY_PATCH_ARG0);
        let _ = symlink(exe_path, &misspelled_link);

        let sandbox_link = temp_dir.path().join(LINUX_SANDBOX_ARG0);
        let _ = symlink(exe_path, &sandbox_link);
    }

    #[cfg(not(unix))]
    {
        // On non-unix, copy or create hardlink
        let patch_link = temp_dir.path().join(format!("{}.exe", APPLY_PATCH_ARG0));
        let _ = fs::copy(exe_path, &patch_link);

        let sandbox_link = temp_dir.path().join(format!("{}.exe", LINUX_SANDBOX_ARG0));
        let _ = fs::copy(exe_path, &sandbox_link);
    }

    let bin_dir = temp_dir.path().to_path_buf();
    debug!("Setup arg0 dispatch in {:?}", bin_dir);

    let paths = Arg0DispatchPaths {
        self_exe: Some(exe_path.clone()),
        dispatch_bin_dir: Some(bin_dir),
    };

    let guard = Arg0PathEntryGuard::new(temp_dir, lock_file, paths);
    guard.apply_to_current_env();
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_apply_patch_invocation_false() {
        assert!(!is_apply_patch_invocation());
        assert!(!is_linux_sandbox_invocation());
    }

    #[test]
    fn test_setup_arg0_dispatch() {
        let guard = setup_arg0_dispatch().unwrap();
        if let Some(ref dir) = guard.paths().dispatch_bin_dir {
            assert!(dir.exists());
            #[cfg(unix)]
            {
                assert!(dir.join(APPLY_PATCH_ARG0).exists());
                assert!(dir.join(LINUX_SANDBOX_ARG0).exists());
            }
        }
    }
}
