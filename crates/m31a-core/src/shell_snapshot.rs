//! Shell environment snapshot capture and caching.
//!
//! Captures interactive user shell environment variables and toolchain paths:
//! - Probes the user's interactive login shell to capture real developer toolchain paths.
//! - Captures PATH, CARGO_HOME, RUSTUP_HOME, NVM_DIR, VIRTUAL_ENV, GOPATH, etc.
//! - Filters dangerous variables (LD_PRELOAD, DYLD_*, PROMPT_COMMAND) for security.
//! - Caches captured snapshots in memory with TTL to minimize subprocess latency.
//! - Injects captured toolchain environment into sandboxed and local executions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SNAPSHOT_TTL: Duration = Duration::from_secs(300); // 5 minutes

/// Key variables defining modern developer toolchains.
pub const ESSENTIAL_TOOLCHAIN_VARS: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "SHELL",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "NVM_DIR",
    "VOLTA_HOME",
    "PNPM_HOME",
    "BUN_INSTALL",
    "VIRTUAL_ENV",
    "CONDA_PREFIX",
    "PYENV_ROOT",
    "GOPATH",
    "GOROOT",
    "JAVA_HOME",
    "PKG_CONFIG_PATH",
    "LANG",
    "LC_ALL",
];

/// Captured snapshot of developer shell environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSnapshot {
    pub shell_path: String,
    pub captured_at_epoch_secs: u64,
    pub environment: HashMap<String, String>,
    pub toolchain_paths: Vec<PathBuf>,
    /// Captured shell aliases from interactive profile (§7.10).
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    /// Fail-closed SHA-256 integrity digest (§7.10).
    #[serde(default)]
    pub integrity_digest: String,
}

impl ShellSnapshot {
    /// Captures a fresh snapshot by probing the login shell and interactive startup state.
    pub fn capture_fresh() -> Self {
        let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut environment = HashMap::new();
        let mut aliases = HashMap::new();

        // 1. Start from current process environment (excluding unvirtualizable credentials)
        for (k, v) in std::env::vars() {
            if !is_dangerous_var(&k) && !is_unvirtualizable_credential(&k, &v) {
                environment.insert(k, v);
            }
        }

        // 2. Attempt login shell probe to harvest developer profile paths
        #[cfg(unix)]
        {
            if let Ok(output) = std::process::Command::new(&shell_path)
                .args(["-l", "-c", "env"])
                .output()
                && output.status.success()
            {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        let key = k.trim();
                        if !key.is_empty()
                            && !is_dangerous_var(key)
                            && !is_unvirtualizable_credential(key, v)
                        {
                            environment.insert(key.to_string(), v.to_string());
                        }
                    }
                }
            }

            // §7.10: Capture interactive startup aliases (e.g. `alias` command output)
            if let Ok(alias_output) = std::process::Command::new(&shell_path)
                .args(["-i", "-c", "alias"])
                .output()
                && alias_output.status.success()
            {
                let text = String::from_utf8_lossy(&alias_output.stdout);
                for line in text.lines() {
                    // formats: `alias name='command'` or `name=command`
                    let trimmed = line.trim();
                    let target = trimmed.strip_prefix("alias ").unwrap_or(trimmed);
                    if let Some((k, v)) = target.split_once('=') {
                        let name = k.trim().to_string();
                        let cmd = v.trim().trim_matches('\'').trim_matches('"').to_string();
                        if !name.is_empty() && !cmd.is_empty() {
                            aliases.insert(name, cmd);
                        }
                    }
                }
            }
        }

        // 3. Fallback discovery of common developer directories if not in PATH
        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() {
            let home_path = Path::new(&home);
            let common_bins = [
                home_path.join(".cargo").join("bin"),
                home_path.join(".local").join("bin"),
                home_path.join(".nvm").join("versions").join("node"),
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/home/linuxbrew/.linuxbrew/bin"),
            ];

            let mut current_path = environment.get("PATH").cloned().unwrap_or_default();
            for bin in &common_bins {
                if bin.is_dir() {
                    let bin_str = bin.to_string_lossy();
                    if !current_path.contains(bin_str.as_ref()) {
                        current_path = format!("{}:{}", bin_str, current_path);
                    }
                }
            }
            environment.insert("PATH".to_string(), current_path);
        }

        // 4. Extract active toolchain paths
        let toolchain_paths = environment
            .get("PATH")
            .map(|p| std::env::split_paths(p).collect())
            .unwrap_or_default();

        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut snapshot = Self {
            shell_path,
            captured_at_epoch_secs: epoch,
            environment,
            toolchain_paths,
            aliases,
            integrity_digest: String::new(),
        };
        snapshot.integrity_digest = snapshot.compute_digest();
        snapshot
    }

    /// Computes stable SHA-256 integrity digest over the environment and aliases (§7.10).
    pub fn compute_digest(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.shell_path.as_bytes());
        hasher.update(self.captured_at_epoch_secs.to_le_bytes());

        // Sort keys for deterministic hashing
        let mut env_keys: Vec<_> = self.environment.keys().collect();
        env_keys.sort();
        for k in env_keys {
            hasher.update(k.as_bytes());
            if let Some(v) = self.environment.get(k) {
                hasher.update(v.as_bytes());
            }
        }

        let mut alias_keys: Vec<_> = self.aliases.keys().collect();
        alias_keys.sort();
        for k in alias_keys {
            hasher.update(k.as_bytes());
            if let Some(v) = self.aliases.get(k) {
                hasher.update(v.as_bytes());
            }
        }

        format!("{:x}", hasher.finalize())
    }

    /// Verifies snapshot integrity against tampering (fail-closed check, §7.10).
    pub fn verify_integrity(&self) -> bool {
        if self.integrity_digest.is_empty() {
            return false;
        }
        self.compute_digest() == self.integrity_digest
    }

    /// Restores environment with brokered-credential re-injection under fail-closed tamper check (§7.10).
    pub fn replay_environment(
        &self,
        brokered_credentials: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, &'static str> {
        if !self.verify_integrity() {
            return Err(
                "Fail-closed tamper check failed: shell snapshot has been modified or corrupted",
            );
        }

        let mut replayed = self.environment.clone();

        // Re-inject brokered credentials safely
        for (k, v) in brokered_credentials {
            replayed.insert(k.clone(), v.clone());
        }

        Ok(replayed)
    }

    /// Retrieve the resolved PATH variable.
    pub fn path_var(&self) -> &str {
        self.environment
            .get("PATH")
            .map(|s| s.as_str())
            .unwrap_or("/usr/bin:/bin")
    }

    /// Check if a binary name exists in the snapshot's PATH.
    pub fn has_binary(&self, bin_name: &str) -> bool {
        for dir in &self.toolchain_paths {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return true;
            }
        }
        false
    }
}

fn is_dangerous_var(key: &str) -> bool {
    let k = key.to_uppercase();
    k.starts_with("LD_")
        || k.starts_with("DYLD_")
        || k == "PROMPT_COMMAND"
        || k == "BASH_ENV"
        || k == "ENV"
        || k == "CDPATH"
}

/// Detects un-virtualizable credential-bearing values that must be rejected from shell snapshot capture (§7.10).
pub fn is_unvirtualizable_credential(key: &str, val: &str) -> bool {
    let k = key.to_uppercase();
    if k.contains("SECRET") || k.contains("PASSWORD") || k.contains("PRIVATE_KEY") {
        return true;
    }
    let v = val.trim();
    v.starts_with("sk-")
        || v.starts_with("AKIA")
        || v.starts_with("ghp_")
        || v.starts_with("glpat-")
        || v.starts_with("Bearer ")
        || (v.len() >= 32 && (k.contains("TOKEN") || k.contains("KEY")))
}

struct CacheEntry {
    snapshot: ShellSnapshot,
    expires_at: Instant,
}

/// Thread-safe in-memory cache for shell environment snapshots.
pub struct ShellSnapshotCache {
    cached: Arc<Mutex<Option<CacheEntry>>>,
}

impl Default for ShellSnapshotCache {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_CACHE: std::sync::LazyLock<ShellSnapshotCache> =
    std::sync::LazyLock::new(ShellSnapshotCache::new);

impl ShellSnapshotCache {
    pub fn new() -> Self {
        Self {
            cached: Arc::new(Mutex::new(None)),
        }
    }

    /// Access the global shared shell snapshot cache.
    pub fn global() -> &'static ShellSnapshotCache {
        &GLOBAL_CACHE
    }

    /// Convenience helper to retrieve cached snapshot from global cache.
    pub fn get_global() -> ShellSnapshot {
        Self::global().get_or_capture()
    }

    /// Retrieve cached snapshot or capture a fresh one if expired.
    pub fn get_or_capture(&self) -> ShellSnapshot {
        let mut lock = self.cached.lock().unwrap();
        let now = Instant::now();

        if let Some(ref entry) = *lock
            && now < entry.expires_at
        {
            return entry.snapshot.clone();
        }

        let fresh = ShellSnapshot::capture_fresh();
        *lock = Some(CacheEntry {
            snapshot: fresh.clone(),
            expires_at: now + SNAPSHOT_TTL,
        });

        fresh
    }

    /// Invalidate cached snapshot.
    pub fn invalidate(&self) {
        let mut lock = self.cached.lock().unwrap();
        *lock = None;
    }
}

/// Exporter creating hermetic, reproducible execution artifacts (Dockerfile, devcontainer.json, flake.nix).
pub struct HermeticEnvironmentExporter;

impl HermeticEnvironmentExporter {
    /// Generates a reproducible Dockerfile from the captured environment snapshot and workspace characteristics.
    pub fn generate_dockerfile(snapshot: &ShellSnapshot, base_image: Option<&str>) -> String {
        let base = base_image.unwrap_or("ubuntu:24.04");
        let mut dockerfile = format!("FROM {}\n\n", base);
        dockerfile.push_str("ENV DEBIAN_FRONTEND=noninteractive\n");
        dockerfile
            .push_str("RUN apt-get update && apt-get install -y --no-install-recommends \\\n");
        dockerfile.push_str("    ca-certificates curl git build-essential pkg-config && \\\n");
        dockerfile.push_str("    rm -rf /var/lib/apt/lists/*\n\n");

        for var in ESSENTIAL_TOOLCHAIN_VARS {
            if let Some(val) = snapshot.environment.get(*var)
                && *var != "PATH"
                && *var != "USER"
                && *var != "HOME"
                && *var != "SHELL"
            {
                dockerfile.push_str(&format!("ENV {}={}\n", var, val));
            }
        }

        dockerfile.push_str("\nWORKDIR /workspace\n");
        dockerfile.push_str("CMD [\"/bin/bash\"]\n");
        dockerfile
    }

    /// Generates devcontainer.json configuration for VS Code / Codespaces reproduction.
    pub fn generate_devcontainer(snapshot: &ShellSnapshot, project_name: &str) -> String {
        let mut env_map = serde_json::Map::new();
        for var in ["CARGO_HOME", "RUSTUP_HOME", "GOPATH", "NODE_ENV"] {
            if let Some(val) = snapshot.environment.get(var) {
                env_map.insert(var.to_string(), serde_json::Value::String(val.clone()));
            }
        }

        let devcontainer = serde_json::json!({
            "name": format!("M31A Hermetic - {}", project_name),
            "dockerFile": "Dockerfile",
            "containerEnv": env_map,
            "customizations": {
                "vscode": {
                    "extensions": [
                        "rust-lang.rust-analyzer",
                        "tamasfe.even-better-toml"
                    ]
                }
            },
            "postCreateCommand": "echo 'M31A Hermetic Container initialized'"
        });

        serde_json::to_string_pretty(&devcontainer).unwrap_or_default()
    }

    /// Generates Nix flake.nix specification for bit-for-bit reproducible developer environment.
    pub fn generate_flake_nix(packages: &[&str]) -> String {
        let pkgs_list = if packages.is_empty() {
            vec!["rustc", "cargo", "pkg-config", "openssl"]
        } else {
            packages.to_vec()
        };

        format!(
            r#"{{
  description = "M31A Hermetic Reproducible Development Environment";

  inputs = {{
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  }};

  outputs = {{ self, nixpkgs, flake-utils }}:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${{system}};
      in
      {{
        devShells.default = pkgs.mkShell {{
          buildInputs = with pkgs; [
            {pkgs}
          ];
          shellHook = ''
            echo "Entered M31A hermetic reproducible development shell"
          '';
        }};
      }});
}}
"#,
            pkgs = pkgs_list.join("\n            ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_fresh_snapshot() {
        let snapshot = ShellSnapshot::capture_fresh();
        assert!(!snapshot.environment.is_empty());
        assert!(snapshot.environment.contains_key("PATH"));
        assert!(!snapshot.toolchain_paths.is_empty());

        // Dangerous variables must not be present
        assert!(!snapshot.environment.contains_key("LD_PRELOAD"));
        assert!(!snapshot.environment.contains_key("DYLD_INSERT_LIBRARIES"));
    }

    #[test]
    fn test_has_binary_standard_lookup() {
        let snapshot = ShellSnapshot::capture_fresh();
        // Common unix utilities should exist on linux
        #[cfg(unix)]
        {
            assert!(snapshot.has_binary("sh") || snapshot.has_binary("ls"));
        }
    }

    #[test]
    fn test_snapshot_cache_lifecycle() {
        let cache = ShellSnapshotCache::new();
        let snap1 = cache.get_or_capture();
        let snap2 = cache.get_or_capture();

        assert_eq!(snap1.captured_at_epoch_secs, snap2.captured_at_epoch_secs);

        cache.invalidate();
        let snap3 = cache.get_or_capture();
        assert!(snap3.environment.contains_key("PATH"));
    }

    #[test]
    fn test_integrity_tamper_check_and_credential_replay() {
        let mut snapshot = ShellSnapshot::capture_fresh();
        assert!(snapshot.verify_integrity());

        let mut creds = HashMap::new();
        creds.insert("M31A_TOKEN".to_string(), "brokered_secret".to_string());

        let replayed = snapshot
            .replay_environment(&creds)
            .expect("replay succeeds");
        assert_eq!(
            replayed.get("M31A_TOKEN").map(|s| s.as_str()),
            Some("brokered_secret")
        );

        // Tamper with snapshot
        snapshot
            .environment
            .insert("MALICIOUS_INJECT".to_string(), "hacked".to_string());
        assert!(!snapshot.verify_integrity());
        assert!(snapshot.replay_environment(&creds).is_err());
    }

    #[test]
    fn test_unvirtualizable_credential_detection() {
        assert!(is_unvirtualizable_credential(
            "OPENAI_API_KEY",
            "sk-proj-12345678901234567890"
        ));
        assert!(is_unvirtualizable_credential(
            "AWS_ACCESS_KEY_ID",
            "AKIAIOSFODNN7EXAMPLE"
        ));
        assert!(is_unvirtualizable_credential(
            "DATABASE_PASSWORD",
            "supersecret"
        ));
        assert!(!is_unvirtualizable_credential("USER", "developer"));
        assert!(!is_unvirtualizable_credential("LANG", "en_US.UTF-8"));
    }

    #[test]
    fn test_hermetic_environment_exporter() {
        let snapshot = ShellSnapshot::capture_fresh();
        let dockerfile = HermeticEnvironmentExporter::generate_dockerfile(&snapshot, None);
        assert!(dockerfile.starts_with("FROM ubuntu:24.04"));
        assert!(dockerfile.contains("WORKDIR /workspace"));

        let devcontainer =
            HermeticEnvironmentExporter::generate_devcontainer(&snapshot, "M31A-Test");
        assert!(devcontainer.contains("M31A Hermetic - M31A-Test"));

        let flake = HermeticEnvironmentExporter::generate_flake_nix(&["rustc", "cargo"]);
        assert!(flake.contains("rustc"));
        assert!(flake.contains("cargo"));
        assert!(flake.contains("devShells.default"));
    }
}
