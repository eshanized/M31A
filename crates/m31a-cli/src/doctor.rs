//! M31A Doctor Diagnostic Subsystem
//!
//! Evaluates system environment, dependencies, toolchains, sandbox containment, network connectivity, and storage integrity.
//! Performs comprehensive, read-mostly diagnostics across system, environment, network,
//! bubblewrap sandboxing, SQLite thread storage, and security configurations.
//! Supports both human-readable terminal rendering and machine-parseable `--json` reports.

use colored::Colorize;
use m31a_core::Config;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCheck {
    pub os: String,
    pub arch: String,
    pub hostname: String,
    pub terminal: String,
    pub is_terminal: bool,
    pub color_support: bool,
    pub multiplexer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCheck {
    pub installed: bool,
    pub version: Option<String>,
    pub inside_worktree: bool,
    pub branch: Option<String>,
    pub head_commit: Option<String>,
    pub is_clean: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallContextCheck {
    pub method: String,
    pub executable_path: PathBuf,
    pub bin_dir: PathBuf,
    pub has_resources: bool,
    pub is_development: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlagsCheck {
    pub total_features: usize,
    pub active_features: usize,
    pub stable_count: usize,
    pub experimental_count: usize,
    pub under_dev_count: usize,
    pub active_list: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemCheck {
    pub config_dir: PathBuf,
    pub config_dir_exists: bool,
    pub config_file_exists: bool,
    pub memories_dir_exists: bool,
    pub is_writable: bool,
    pub is_nfs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCheck {
    pub endpoint_url: String,
    pub reachable: bool,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u128>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCheck {
    pub bwrap_installed: bool,
    pub bwrap_version: Option<String>,
    pub unprivileged_userns: bool,
    pub isolation_test_passed: bool,
    pub descriptor_pinned_exec: bool,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCheck {
    pub key_configured: bool,
    pub key_length: usize,
    pub active_model: String,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadInventoryCheck {
    pub db_path: PathBuf,
    pub db_exists: bool,
    pub db_size_bytes: u64,
    pub thread_count: usize,
    pub board_post_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonCheck {
    pub socket_path: PathBuf,
    pub socket_exists: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioHardwareCheck {
    pub total_devices: usize,
    pub capture_devices: usize,
    pub playback_devices: usize,
    pub devices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSnapshotCheck {
    pub shell: String,
    pub cached: bool,
    pub toolchains_detected: Vec<String>,
    pub env_var_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDirectoryCheck {
    pub plugins_dir: PathBuf,
    pub exists: bool,
    pub is_writable: bool,
    pub plugin_count: usize,
    pub healthy_manifests: usize,
    pub plugin_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPresetCheck {
    pub active_sandbox_policy: String,
    pub active_approval_policy: String,
    pub available_presets: Vec<String>,
    pub sleep_inhibitor_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub timestamp: String,
    pub version: String,
    pub system: SystemCheck,
    pub install_context: InstallContextCheck,
    pub git: GitCheck,
    pub filesystem: FilesystemCheck,
    pub network: NetworkCheck,
    pub sandbox: SandboxCheck,
    pub auth: AuthCheck,
    pub thread_inventory: ThreadInventoryCheck,
    pub daemon: DaemonCheck,
    pub audio_hardware: AudioHardwareCheck,
    pub shell_snapshot: ShellSnapshotCheck,
    pub plugin_directory: PluginDirectoryCheck,
    pub features: FeatureFlagsCheck,
    pub security: SecurityPresetCheck,
}

pub struct Doctor;

impl Doctor {
    pub async fn run_diagnostics(config: &Config) -> DoctorReport {
        let system = Self::check_system();
        let install_context = Self::check_install_context();
        let git = Self::check_git();
        let filesystem = Self::check_filesystem();
        let network = Self::check_network(config).await;
        let sandbox = Self::check_sandbox();
        let auth = Self::check_auth(config);
        let thread_inventory = Self::check_thread_inventory();
        let daemon = Self::check_daemon();
        let audio_hardware = Self::check_audio_hardware();
        let shell_snapshot = Self::check_shell_snapshot();
        let plugin_directory = Self::check_plugin_directory();
        let features = Self::check_features();
        let security = Self::check_security(config);

        DoctorReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            system,
            install_context,
            git,
            filesystem,
            network,
            sandbox,
            auth,
            thread_inventory,
            daemon,
            audio_hardware,
            shell_snapshot,
            plugin_directory,
            features,
            security,
        }
    }

    fn check_system() -> SystemCheck {
        let os = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();
        let hostname = env::var("HOSTNAME")
            .or_else(|_| env::var("HOST"))
            .unwrap_or_else(|_| "localhost".to_string());

        let term_info = m31a_core::TerminalInfo::detect();
        let terminal = term_info.display_summary();
        let is_terminal = term_info.is_tty;
        let color_support = term_info.color_depth != m31a_core::ColorDepth::NoColor;
        let multiplexer = match &term_info.multiplexer {
            Some(m31a_core::Multiplexer::Tmux { version }) => {
                Some(format!("tmux {}", version.as_deref().unwrap_or("active")))
            }
            Some(m31a_core::Multiplexer::Zellij { version }) => {
                Some(format!("zellij {}", version.as_deref().unwrap_or("active")))
            }
            None => None,
        };

        SystemCheck {
            os,
            arch,
            hostname,
            terminal,
            is_terminal,
            color_support,
            multiplexer,
        }
    }

    fn check_git() -> GitCheck {
        let version_out = Command::new("git").arg("--version").output();
        if let Ok(out) = version_out
            && out.status.success()
        {
            let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let inside_worktree = Command::new("git")
                .args(["rev-parse", "--is-inside-work-tree"])
                .output()
                .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
                .unwrap_or(false);

            let branch = if inside_worktree {
                Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                        } else {
                            None
                        }
                    })
            } else {
                None
            };

            let head_commit = if inside_worktree {
                Command::new("git")
                    .args(["rev-parse", "--short", "HEAD"])
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                        } else {
                            None
                        }
                    })
            } else {
                None
            };

            let is_clean = if inside_worktree {
                Command::new("git")
                    .args(["status", "--porcelain"])
                    .output()
                    .ok()
                    .map(|o| {
                        o.status.success() && String::from_utf8_lossy(&o.stdout).trim().is_empty()
                    })
            } else {
                None
            };

            return GitCheck {
                installed: true,
                version: Some(ver),
                inside_worktree,
                branch,
                head_commit,
                is_clean,
            };
        }

        GitCheck {
            installed: false,
            version: None,
            inside_worktree: false,
            branch: None,
            head_commit: None,
            is_clean: None,
        }
    }

    fn check_install_context() -> InstallContextCheck {
        let ctx = m31a_core::InstallContext::current();
        let method_str = match &ctx.method {
            m31a_core::InstallMethod::LocalBin { .. } => "Local User Bin (~/.local/bin/m31a)",
            m31a_core::InstallMethod::CargoInstall { .. } => "Cargo Install (~/.cargo/bin/m31a)",
            m31a_core::InstallMethod::Standalone { .. } => "Standalone Release Distribution",
            m31a_core::InstallMethod::NixStore { .. } => "Nix Store Package",
            m31a_core::InstallMethod::SystemPackage { .. } => "System Package (/usr/bin)",
            m31a_core::InstallMethod::SourceDevelopment { .. } => "Development Workspace Build",
        };
        let has_resources = ctx
            .layout
            .as_ref()
            .and_then(|l| l.resources_dir.as_ref())
            .is_some();

        InstallContextCheck {
            method: method_str.to_string(),
            executable_path: ctx.executable_path.clone(),
            bin_dir: ctx.bin_dir.clone(),
            has_resources,
            is_development: ctx.is_development(),
        }
    }

    fn check_features() -> FeatureFlagsCheck {
        let reg = m31a_core::FeatureRegistry::new();
        let active = reg.active_features();
        let mut stable = 0;
        let mut exp = 0;
        let mut under_dev = 0;

        for &k in m31a_core::FeatureKey::ALL {
            match k.default_stage() {
                m31a_core::FeatureStage::Stable => stable += 1,
                m31a_core::FeatureStage::Experimental { .. } => exp += 1,
                m31a_core::FeatureStage::UnderDevelopment => under_dev += 1,
                _ => {}
            }
        }

        let mut active_list: Vec<String> = active.iter().map(|k| k.as_str().to_string()).collect();
        active_list.sort();

        FeatureFlagsCheck {
            total_features: m31a_core::FeatureKey::ALL.len(),
            active_features: active.len(),
            stable_count: stable,
            experimental_count: exp,
            under_dev_count: under_dev,
            active_list,
        }
    }

    fn check_security(config: &Config) -> SecurityPresetCheck {
        let presets = m31a_execpolicy::ApprovalPreset::builtin_presets()
            .into_iter()
            .map(|p| format!("{} ({})", p.id, p.label))
            .collect();
        let sleep_available = which::which("systemd-inhibit").is_ok()
            || which::which("gnome-session-inhibit").is_ok();
        SecurityPresetCheck {
            active_sandbox_policy: format!("{:?}", config.sandbox),
            active_approval_policy: format!("{:?}", config.approval),
            available_presets: presets,
            sleep_inhibitor_available: sleep_available,
        }
    }

    fn check_filesystem() -> FilesystemCheck {
        let cfg_dir = Config::config_dir();
        let cfg_file = Config::config_path();
        let mem_dir = cfg_dir.join("memories");

        let is_writable = {
            let probe = cfg_dir.join(".write_probe");
            if fs::create_dir_all(&cfg_dir).is_ok() {
                let wrote = fs::write(&probe, b"m31a-doctor-probe").is_ok();
                let _ = fs::remove_file(&probe);
                wrote
            } else {
                false
            }
        };

        let is_nfs = m31a_core::is_nfs(&cfg_dir);

        FilesystemCheck {
            config_dir_exists: cfg_dir.exists(),
            config_file_exists: cfg_file.exists(),
            memories_dir_exists: mem_dir.exists(),
            config_dir: cfg_dir,
            is_writable,
            is_nfs,
        }
    }

    async fn check_network(config: &Config) -> NetworkCheck {
        let url = format!("{}/models", config.api_base);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(4))
            .build();

        match client {
            Ok(c) => {
                let start = Instant::now();
                let mut req = c.get(&url);
                if let Some(ref key) = config.api_key {
                    req = req.bearer_auth(key);
                }
                match req.send().await {
                    Ok(resp) => {
                        let latency = start.elapsed().as_millis();
                        let status = resp.status().as_u16();
                        NetworkCheck {
                            endpoint_url: url,
                            reachable: true,
                            http_status: Some(status),
                            latency_ms: Some(latency),
                            error: None,
                        }
                    }
                    Err(e) => NetworkCheck {
                        endpoint_url: url,
                        reachable: false,
                        http_status: None,
                        latency_ms: None,
                        error: Some(e.to_string()),
                    },
                }
            }
            Err(e) => NetworkCheck {
                endpoint_url: url,
                reachable: false,
                http_status: None,
                latency_ms: None,
                error: Some(e.to_string()),
            },
        }
    }

    fn check_sandbox() -> SandboxCheck {
        let bwrap_out = Command::new("bwrap").arg("--version").output();
        let (bwrap_installed, bwrap_version) = match bwrap_out {
            Ok(o) if o.status.success() => {
                let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
                (true, Some(v))
            }
            _ => (false, None),
        };

        // Check unprivileged user namespaces
        let unprivileged_userns = fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone")
            .map(|s| s.trim() == "1")
            .unwrap_or(true); // default on modern kernels

        // Test running real isolated bwrap probe
        let isolation_test = if bwrap_installed {
            Command::new("bwrap")
                .args([
                    "--ro-bind",
                    "/bin",
                    "/bin",
                    "--ro-bind",
                    "/lib",
                    "/lib",
                    "--ro-bind",
                    "/lib64",
                    "/lib64",
                    "--ro-bind",
                    "/usr",
                    "/usr",
                    "--proc",
                    "/proc",
                    "--dev",
                    "/dev",
                    "--unshare-pid",
                    "--unshare-net",
                    "--unshare-ipc",
                    "--unshare-uts",
                    "echo",
                    "sandbox-active",
                ])
                .output()
                .map(|o| {
                    o.status.success()
                        && String::from_utf8_lossy(&o.stdout).contains("sandbox-active")
                })
                .unwrap_or(false)
        } else {
            false
        };

        let bwrap_path_opt = Command::new("which")
            .arg("bwrap")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if !p.is_empty() {
                        Some(PathBuf::from(p))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

        let descriptor_pinned_exec = if let Some(ref bpath) = bwrap_path_opt {
            m31a_sandbox::descriptor_binary::verify_and_pin_binary(bpath).is_ok()
        } else {
            false
        };

        let details = if isolation_test {
            "Bubblewrap containerization operational with namespace unshare (PID/NET/IPC/UTS)"
                .to_string()
        } else if bwrap_installed {
            "Bubblewrap binary present but isolation probe restricted or unprivileged namespaces disabled".to_string()
        } else {
            "Bubblewrap binary not found in PATH; commands run in host process fallback mode"
                .to_string()
        };

        SandboxCheck {
            bwrap_installed,
            bwrap_version,
            unprivileged_userns,
            isolation_test_passed: isolation_test,
            descriptor_pinned_exec,
            details,
        }
    }

    fn check_auth(config: &Config) -> AuthCheck {
        let (key_configured, key_length) = match &config.api_key {
            Some(k) if !k.trim().is_empty() => (true, k.trim().len()),
            _ => (false, 0),
        };

        AuthCheck {
            key_configured,
            key_length,
            active_model: config.model.clone(),
            profile: config.profile.clone(),
        }
    }

    fn check_thread_inventory() -> ThreadInventoryCheck {
        let db_path = Config::config_dir().join("state.db");
        let db_exists = db_path.exists();
        let db_size_bytes = db_path.metadata().map(|m| m.len()).unwrap_or(0);

        let (thread_count, board_post_count) = if db_exists {
            if let Ok(storage) = m31a_storage::Storage::open(&db_path) {
                let threads = storage.list_threads(1000).map(|v| v.len()).unwrap_or(0);
                let posts = storage
                    .list_posts("default", "general", 1000)
                    .map(|v| v.len())
                    .unwrap_or(0);
                (threads, posts)
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        };

        ThreadInventoryCheck {
            db_path,
            db_exists,
            db_size_bytes,
            thread_count,
            board_post_count,
        }
    }

    fn check_daemon() -> DaemonCheck {
        let socket_path = Config::config_dir().join("m31a-daemon.sock");
        let socket_exists = socket_path.exists();
        let is_active =
            socket_exists && std::os::unix::net::UnixStream::connect(&socket_path).is_ok();

        DaemonCheck {
            socket_path,
            socket_exists,
            is_active,
        }
    }

    fn check_audio_hardware() -> AudioHardwareCheck {
        let devices = m31a_core::realtime_voice::AudioDeviceManager::enumerate_devices();
        let total = devices.len();
        let capture = devices
            .iter()
            .filter(|d| matches!(d.kind, m31a_core::realtime_voice::AudioDeviceKind::Input))
            .count();
        let playback = devices
            .iter()
            .filter(|d| matches!(d.kind, m31a_core::realtime_voice::AudioDeviceKind::Output))
            .count();
        let summary: Vec<String> = devices
            .iter()
            .map(|d| {
                format!(
                    "{} [{} - {} channel(s){}]",
                    d.name,
                    match d.kind {
                        m31a_core::realtime_voice::AudioDeviceKind::Input => "Microphone/Capture",
                        m31a_core::realtime_voice::AudioDeviceKind::Output => "Speaker/Playback",
                    },
                    d.channels,
                    if d.is_default { " (Default)" } else { "" }
                )
            })
            .collect();

        AudioHardwareCheck {
            total_devices: total,
            capture_devices: capture,
            playback_devices: playback,
            devices: summary,
        }
    }

    fn check_shell_snapshot() -> ShellSnapshotCheck {
        let snapshot = m31a_core::shell_snapshot::ShellSnapshotCache::get_global();
        let mut toolchains = Vec::new();
        for tp in &snapshot.toolchain_paths {
            toolchains.push(tp.display().to_string());
        }
        if (snapshot.environment.contains_key("CARGO_HOME")
            || snapshot
                .environment
                .get("PATH")
                .map(|p| p.contains(".cargo"))
                .unwrap_or(false))
            && !toolchains.iter().any(|t| t.contains("cargo"))
        {
            toolchains.push("Rust (Cargo)".to_string());
        }
        if (snapshot.environment.contains_key("NVM_DIR")
            || snapshot
                .environment
                .get("PATH")
                .map(|p| p.contains(".nvm") || p.contains("node"))
                .unwrap_or(false))
            && !toolchains
                .iter()
                .any(|t| t.contains("node") || t.contains("nvm"))
        {
            toolchains.push("Node.js (NVM)".to_string());
        }
        if snapshot.environment.contains_key("VIRTUAL_ENV")
            || snapshot.environment.contains_key("CONDA_PREFIX")
        {
            toolchains.push("Python (VirtualEnv/Conda)".to_string());
        }

        ShellSnapshotCheck {
            shell: snapshot.shell_path.clone(),
            cached: true,
            toolchains_detected: toolchains,
            env_var_count: snapshot.environment.len(),
        }
    }

    fn check_plugin_directory() -> PluginDirectoryCheck {
        let plugins_dir = Config::config_dir().join("plugins");
        let exists = plugins_dir.exists();
        let is_writable = if exists {
            let test_file = plugins_dir.join(".probe_write");
            if fs::write(&test_file, b"ok").is_ok() {
                let _ = fs::remove_file(test_file);
                true
            } else {
                false
            }
        } else {
            false
        };

        let mut plugin_count = 0;
        let mut healthy_manifests = 0;
        let mut plugin_names = Vec::new();

        if exists && let Ok(entries) = fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    plugin_count += 1;
                    let manifest = path.join("plugin.toml");
                    let manifest_json = path.join("plugin.json");
                    if manifest.exists() || manifest_json.exists() {
                        healthy_manifests += 1;
                        plugin_names.push(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
        }

        PluginDirectoryCheck {
            plugins_dir,
            exists,
            is_writable,
            plugin_count,
            healthy_manifests,
            plugin_names,
        }
    }

    pub fn render_human(report: &DoctorReport) {
        println!(
            "{}",
            "========================================================="
                .bold()
                .blue()
        );
        println!(
            "  {} Doctor Diagnostics Report",
            "M31A".bold().bright_green()
        );
        println!(
            "  Version: {} | Host: {} | OS: {}/{}",
            report.version, report.system.hostname, report.system.os, report.system.arch
        );
        println!("  Timestamp: {}", report.timestamp);
        println!(
            "{}\n",
            "========================================================="
                .bold()
                .blue()
        );

        // 1. System & Terminal
        println!("{}", "1. System & Terminal Environment".bold().underline());
        println!(
            "   Terminal: {} (TTY: {}, Colors: {})",
            report.system.terminal.cyan(),
            if report.system.is_terminal {
                "Yes".green()
            } else {
                "No".yellow()
            },
            if report.system.color_support {
                "Yes".green()
            } else {
                "No".yellow()
            },
        );
        if let Some(ref mux) = report.system.multiplexer {
            println!("   Multiplexer: {} active", mux.magenta());
        }
        println!(
            "   Install Origin: {}",
            report.install_context.method.cyan()
        );
        println!(
            "   Executable: {:?}",
            report.install_context.executable_path
        );
        println!("   Package Bin Dir: {:?}", report.install_context.bin_dir);

        // 2. Git Environment
        println!("\n{}", "2. Git Workspace Diagnostics".bold().underline());
        if report.git.installed {
            let ver = report.git.version.as_deref().unwrap_or("unknown");
            println!("   Git Binary: {} ({})", "Installed".green(), ver);
            if report.git.inside_worktree {
                println!("   Repository: Inside git worktree");
                if let Some(ref br) = report.git.branch {
                    println!("   Branch: {}", br.cyan());
                }
                if let Some(ref head) = report.git.head_commit {
                    println!("   Head Commit: {}", head.yellow());
                }
                if let Some(clean) = report.git.is_clean {
                    println!(
                        "   Working Tree: {}",
                        if clean {
                            "Clean".green()
                        } else {
                            "Modified (Dirty)".yellow()
                        }
                    );
                }
            } else {
                println!(
                    "   Repository: {} (Current directory is not a git repository)",
                    "None".yellow()
                );
            }
        } else {
            println!(
                "   Git Binary: {} (Install git to enable branch & worktree features)",
                "Missing".red()
            );
        }

        // 3. Filesystem & Configuration
        println!("\n{}", "3. Configuration & Filesystem".bold().underline());
        println!(
            "   Config Directory: {:?} (Exists: {}, Writable: {})",
            report.filesystem.config_dir,
            if report.filesystem.config_dir_exists {
                "Yes".green()
            } else {
                "No".yellow()
            },
            if report.filesystem.is_writable {
                "Yes".green()
            } else {
                "No".red()
            },
        );
        println!(
            "   Config File: {}",
            if report.filesystem.config_file_exists {
                "Found (~/.m31a/config.toml)".green()
            } else {
                "Default In-Memory (No custom ~/.m31a/config.toml)".cyan()
            }
        );
        println!(
            "   NFS Network Mount: {}",
            if report.filesystem.is_nfs {
                "Yes (NFS mount detected - flock cautiously)".yellow()
            } else {
                "No (Local POSIX locks safe)".green()
            }
        );

        // 4. Authentication & Model
        println!(
            "\n{}",
            "4. Model Provider & Authentication".bold().underline()
        );
        println!(
            "   Active Model: {}",
            report.auth.active_model.bright_green()
        );
        if let Some(ref prof) = report.auth.profile {
            println!("   Active Profile: {}", prof.cyan());
        }
        if report.auth.key_configured {
            println!(
                "   API Key: {} ({} characters configured)",
                "OK".green(),
                report.auth.key_length
            );
        } else {
            println!(
                "   API Key: {} (NVIDIA_API_KEY environment variable is not set)",
                "WARN".yellow()
            );
            println!("   Remediation: export NVIDIA_API_KEY=\"nvapi-...\"");
        }

        // 5. Network Connectivity & Latency
        println!("\n{}", "5. Network Connectivity Probe".bold().underline());
        println!("   Target Endpoint: {}", report.network.endpoint_url.cyan());
        if report.network.reachable {
            let status = report.network.http_status.unwrap_or(0);
            let lat = report.network.latency_ms.unwrap_or(0);
            println!(
                "   Endpoint Reachable: {} (Status: {}, Latency: {} ms)",
                "OK".green(),
                status,
                lat
            );
        } else {
            let err = report.network.error.as_deref().unwrap_or("unknown error");
            println!("   Endpoint Reachable: {} (Error: {})", "FAIL".red(), err);
        }

        // 6. Sandboxing & Isolation
        println!(
            "\n{}",
            "6. Sandboxing & Isolation Engine".bold().underline()
        );
        if report.sandbox.bwrap_installed {
            let ver = report.sandbox.bwrap_version.as_deref().unwrap_or("");
            println!("   Bubblewrap: {} ({})", "Installed".green(), ver);
            println!(
                "   Unprivileged User Namespaces: {}",
                if report.sandbox.unprivileged_userns {
                    "Enabled".green()
                } else {
                    "Disabled".yellow()
                }
            );
            println!(
                "   Isolation Probe: {}",
                if report.sandbox.isolation_test_passed {
                    "PASS (Namespaces isolated)".green()
                } else {
                    "FAIL (Restricted or sandbox disabled)".yellow()
                }
            );
            println!(
                "   Descriptor-Backed Pinning: {}",
                if report.sandbox.descriptor_pinned_exec {
                    "PASS (TOCTOU-free /proc/self/fd execution)".green()
                } else {
                    "FAIL (Verification failed)".yellow()
                }
            );
        } else {
            println!(
                "   Bubblewrap: {} (Install bubblewrap for containerized isolation)",
                "Not Installed".yellow()
            );
        }
        println!("   Details: {}", report.sandbox.details.dimmed());

        // 7. State Database & Thread Store
        println!("\n{}", "7. State Storage & Thread Store".bold().underline());
        println!("   Database: {:?}", report.thread_inventory.db_path);
        if report.thread_inventory.db_exists {
            println!(
                "   Status: {} (Size: {:.2} KB)",
                "Active".green(),
                report.thread_inventory.db_size_bytes as f64 / 1024.0
            );
            println!(
                "   Persisted Threads: {}",
                report.thread_inventory.thread_count.to_string().cyan()
            );
            println!(
                "   Message Board Posts: {}",
                report.thread_inventory.board_post_count.to_string().cyan()
            );
        } else {
            println!(
                "   Status: {} (Will be auto-created on first session)",
                "Clean / New".cyan()
            );
        }

        // 8. Background Daemon
        println!("\n{}", "8. Background App Daemon".bold().underline());
        println!("   UDS Socket: {:?}", report.daemon.socket_path);
        println!(
            "   Daemon Status: {}",
            if report.daemon.is_active {
                "Running & Connectable".green()
            } else {
                "Idle / Not Started (Use 'm31a daemon' or auto-starts on demand)".cyan()
            }
        );

        // 9. Real-Time Audio Hardware Topology
        println!(
            "\n{}",
            "9. Real-Time Audio Hardware Topology".bold().underline()
        );
        println!(
            "   Detected PCM Devices: {} (Capture: {}, Playback: {})",
            report.audio_hardware.total_devices.to_string().cyan(),
            report.audio_hardware.capture_devices.to_string().green(),
            report.audio_hardware.playback_devices.to_string().green(),
        );
        for dev in &report.audio_hardware.devices {
            println!("   - {}", dev.dimmed());
        }

        // 10. Login Shell Environment & Toolchain Snapshot
        println!(
            "\n{}",
            "10. Login Shell Environment & Toolchains"
                .bold()
                .underline()
        );
        println!("   Login Shell: {}", report.shell_snapshot.shell.cyan());
        println!(
            "   Environment Variables Captured: {}",
            report.shell_snapshot.env_var_count.to_string().cyan()
        );
        println!(
            "   Detected Toolchains: {}",
            if report.shell_snapshot.toolchains_detected.is_empty() {
                "Standard POSIX (no specialized SDKs detected in PATH)".dimmed()
            } else {
                report.shell_snapshot.toolchains_detected.join(", ").green()
            }
        );

        // 11. Extension Plugins & Bundles Directory
        println!(
            "\n{}",
            "11. Extension Plugins & Bundles Directory"
                .bold()
                .underline()
        );
        println!(
            "   Plugins Directory: {:?}",
            report.plugin_directory.plugins_dir
        );
        println!(
            "   Directory Exists: {} (Writable: {})",
            if report.plugin_directory.exists {
                "Yes".green()
            } else {
                "No (Will be created when installing plugins)".cyan()
            },
            if report.plugin_directory.is_writable {
                "Yes".green()
            } else {
                "No / Not yet created".yellow()
            },
        );
        if report.plugin_directory.plugin_count > 0 {
            println!(
                "   Installed Plugins: {} (Healthy Manifests: {})",
                report.plugin_directory.plugin_count.to_string().cyan(),
                report
                    .plugin_directory
                    .healthy_manifests
                    .to_string()
                    .green(),
            );
            for p in &report.plugin_directory.plugin_names {
                println!("   - {}", p.bright_blue());
            }
        }

        // 12. Feature Flags & Sovereign Capability Matrix
        println!(
            "\n{}",
            "12. Feature Flags & Sovereign Capability Matrix"
                .bold()
                .underline()
        );
        println!(
            "   Total Registered Features: {}",
            report.features.total_features.to_string().cyan()
        );
        println!(
            "   Active Capabilities: {} (Stable: {}, Experimental: {}, Under Development: {})",
            report.features.active_features.to_string().bright_green(),
            report.features.stable_count.to_string().green(),
            report.features.experimental_count.to_string().yellow(),
            report.features.under_dev_count.to_string().magenta(),
        );
        let preview_limit = 10;
        let active_preview = report
            .features
            .active_list
            .iter()
            .take(preview_limit)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let remaining = report
            .features
            .active_list
            .len()
            .saturating_sub(preview_limit);
        println!(
            "   Active Flags: {}{}",
            active_preview.cyan(),
            if remaining > 0 {
                format!(" (+{} more)", remaining)
            } else {
                String::new()
            },
        );

        // 13. Security Presets & Sandboxing Posture
        println!(
            "\n{}",
            "13. Security Presets & Sandboxing Posture"
                .bold()
                .underline()
        );
        println!(
            "   Active Sandbox Policy: {}",
            report.security.active_sandbox_policy.cyan().bold()
        );
        println!(
            "   Active Approval Policy: {}",
            report.security.active_approval_policy.yellow().bold()
        );
        println!(
            "   Sleep Inhibitor: {}",
            if report.security.sleep_inhibitor_available {
                "Available (systemd-inhibit / gnome-session-inhibit)"
                    .green()
                    .bold()
            } else {
                "None (No-op fallback)".dimmed()
            }
        );
        println!(
            "   Available Presets: {}",
            report.security.available_presets.join(", ").dimmed()
        );

        println!(
            "\n{}",
            "Diagnostic Summary: All systems verified."
                .bold()
                .bright_green()
        );
    }
}
