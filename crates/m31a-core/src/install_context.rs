//! Installation method detection, bundle layout discovery, and compilation provenance.
//!
//! Identifies whether M31A was launched from Cargo, system package manager, standalone binary, or container.

use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Method by which the current M31A runtime executable was installed or launched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum InstallMethod {
    /// Cargo install (`~/.cargo/bin/m31a`).
    CargoInstall { bin_path: PathBuf },
    /// User local bin install (`~/.local/bin/m31a`).
    LocalBin { bin_path: PathBuf },
    /// Managed standalone release distribution bundle.
    Standalone { package_dir: PathBuf },
    /// Nix package manager store path (`/nix/store/...`).
    NixStore { store_path: PathBuf },
    /// System package manager (`/usr/bin` or `/usr/local/bin`).
    SystemPackage { bin_path: PathBuf },
    /// Local workspace development build (`target/debug` or `target/release`).
    SourceDevelopment { target_dir: PathBuf },
}

/// Discovered package layout for bundled M31A installations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageLayout {
    pub package_dir: PathBuf,
    pub bin_dir: PathBuf,
    pub resources_dir: Option<PathBuf>,
    pub path_dir: Option<PathBuf>,
}

/// Discovered package layout and installation directories.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallContext {
    pub method: InstallMethod,
    pub executable_path: PathBuf,
    pub bin_dir: PathBuf,
    pub layout: Option<PackageLayout>,
}

static INSTALL_CONTEXT: OnceLock<InstallContext> = OnceLock::new();

impl InstallContext {
    /// Resolves the current installation context by inspecting the running executable.
    pub fn current() -> &'static Self {
        INSTALL_CONTEXT.get_or_init(|| {
            let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("m31a"));
            Self::detect_from_path(&exe)
        })
    }

    /// Detects installation context from an explicit executable path.
    pub fn detect_from_path(exe: &Path) -> Self {
        let bin_dir = exe
            .parent()
            .unwrap_or_else(|| Path::new("/usr/local/bin"))
            .to_path_buf();
        let exe_str = exe.to_string_lossy();

        let method = if exe_str.contains("/target/debug/") || exe_str.contains("/target/release/") {
            let target_dir = bin_dir.parent().unwrap_or(&bin_dir).to_path_buf();
            InstallMethod::SourceDevelopment { target_dir }
        } else if exe_str.contains("/.local/bin/") {
            InstallMethod::LocalBin {
                bin_path: exe.to_path_buf(),
            }
        } else if exe_str.contains("/.cargo/bin/") {
            InstallMethod::CargoInstall {
                bin_path: exe.to_path_buf(),
            }
        } else if exe_str.starts_with("/nix/store/") {
            InstallMethod::NixStore {
                store_path: bin_dir.clone(),
            }
        } else if exe_str.starts_with("/usr/bin/") || exe_str.starts_with("/usr/local/bin/") {
            InstallMethod::SystemPackage {
                bin_path: exe.to_path_buf(),
            }
        } else {
            InstallMethod::Standalone {
                package_dir: bin_dir.clone(),
            }
        };

        let resources_dir = bin_dir.join("resources");
        let resources_opt = if resources_dir.exists() && resources_dir.is_dir() {
            Some(resources_dir.clone())
        } else {
            None
        };

        let path_dir = bin_dir.join("m31a-path");
        let path_dir_opt = if path_dir.exists() && path_dir.is_dir() {
            Some(path_dir)
        } else {
            None
        };

        let package_dir = bin_dir.parent().unwrap_or(&bin_dir).to_path_buf();
        let layout = Some(PackageLayout {
            package_dir,
            bin_dir: bin_dir.clone(),
            resources_dir: resources_opt,
            path_dir: path_dir_opt,
        });

        Self {
            method,
            executable_path: exe.to_path_buf(),
            bin_dir,
            layout,
        }
    }

    /// Finds a managed helper executable in `resources/` or falls back to system `$PATH`.
    pub fn find_resource_tool(&self, tool_name: &str) -> Option<PathBuf> {
        if let Some(ref layout) = self.layout
            && let Some(ref res_dir) = layout.resources_dir
        {
            let candidate = res_dir.join(tool_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        which::which(tool_name).ok()
    }

    pub fn is_development(&self) -> bool {
        matches!(self.method, InstallMethod::SourceDevelopment { .. })
    }
}

/// Build provenance and target architecture stamped at compilation time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildInfo {
    pub version: &'static str,
    pub build_commit: &'static str,
    pub target: &'static str,
    pub rustc_version: &'static str,
}

pub static CURRENT_BUILD: BuildInfo = BuildInfo {
    version: env!("CARGO_PKG_VERSION"),
    build_commit: match option_env!("M31A_BUILD_COMMIT") {
        Some(c) => c,
        None => "cc597e4",
    },
    target: match option_env!("TARGET") {
        Some(t) => t,
        None => "x86_64-unknown-linux-gnu",
    },
    rustc_version: match option_env!("M31A_RUSTC_VERSION") {
        Some(r) => r,
        None => "rustc 1.86.0",
    },
};

impl BuildInfo {
    pub fn get() -> &'static BuildInfo {
        &CURRENT_BUILD
    }

    pub fn format_banner(&self) -> String {
        format!(
            "M31A v{} (commit: {}, target: {})",
            self.version, self.build_commit, self.target
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_context_detection() {
        let ctx = InstallContext::current();
        assert!(!ctx.executable_path.as_os_str().is_empty());
        assert!(!ctx.bin_dir.as_os_str().is_empty());
    }

    #[test]
    fn test_install_method_variants() {
        let local_bin = InstallContext::detect_from_path(Path::new("/home/user/.local/bin/m31a"));
        assert!(matches!(local_bin.method, InstallMethod::LocalBin { .. }));

        let cargo_bin = InstallContext::detect_from_path(Path::new("/home/user/.cargo/bin/m31a"));
        assert!(matches!(
            cargo_bin.method,
            InstallMethod::CargoInstall { .. }
        ));

        let dev_bin =
            InstallContext::detect_from_path(Path::new("/home/user/repo/target/debug/m31a"));
        assert!(dev_bin.is_development());

        let sys_bin = InstallContext::detect_from_path(Path::new("/usr/bin/m31a"));
        assert!(matches!(
            sys_bin.method,
            InstallMethod::SystemPackage { .. }
        ));
    }

    #[test]
    fn test_find_resource_tool_fallback() {
        let ctx = InstallContext::current();
        // sh or bash exists on linux
        let sh_tool = ctx.find_resource_tool("sh");
        assert!(sh_tool.is_some());
    }

    #[test]
    fn test_build_info_banner() {
        let build = BuildInfo::get();
        assert_eq!(build.version, "0.1.15");
        let banner = build.format_banner();
        assert!(banner.contains("M31A v0.1.15"));
        assert!(banner.contains("commit:"));
    }
}
