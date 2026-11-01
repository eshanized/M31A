//! M31A Plugin and Marketplace Subsystem
//!
//! Plugin discovery, manifest validation, dependency resolution, and runtime lifecycle management.
//! Discovers, loads, and manages M31A plugins from user (`~/.m31a/plugins`) and project
//! (`.m31a/plugins`) directories. Plugins bundle skills, MCP server definitions, hooks,
//! and interface customizations.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::{Path, PathBuf};

pub const PLUGIN_MANIFEST_FILENAME: &str = "plugin.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMcpServer {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub author: Option<String>,
    pub skills: Option<String>,
    #[serde(default)]
    pub mcp_servers: std::collections::HashMap<String, PluginMcpServer>,
    pub hooks: Option<String>,
    #[serde(default)]
    pub extensions: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub description: String,
    pub path: PathBuf,
    pub manifest: PluginManifest,
    pub enabled: bool,
}

pub struct PluginManager {
    search_paths: Vec<PathBuf>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        let mut paths = Vec::new();

        // Project plugins
        paths.push(PathBuf::from(".m31a").join("plugins"));
        paths.push(PathBuf::from(".codex").join("plugins"));

        // User global plugins
        let cfg_dir = crate::Config::config_dir();
        paths.push(cfg_dir.join("plugins"));

        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(home).join(".codex").join("plugins"));
        }

        Self {
            search_paths: paths,
        }
    }

    pub fn with_search_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            search_paths: paths,
        }
    }

    /// Discovers all plugins across configured search paths.
    pub fn discover_plugins(&self) -> Vec<InstalledPlugin> {
        let mut plugins = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for root in &self.search_paths {
            if !root.is_dir() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let manifest_file = path.join(PLUGIN_MANIFEST_FILENAME);
                        if manifest_file.is_file()
                            && let Ok(content) = fs::read_to_string(&manifest_file)
                            && let Ok(manifest) = serde_json::from_str::<PluginManifest>(&content)
                            && seen_names.insert(manifest.name.clone())
                        {
                            plugins.push(InstalledPlugin {
                                name: manifest.name.clone(),
                                version: manifest
                                    .version
                                    .clone()
                                    .unwrap_or_else(|| "0.1.0".to_string()),
                                description: manifest.description.clone().unwrap_or_default(),
                                path: path.clone(),
                                manifest,
                                enabled: true,
                            });
                        }
                    }
                }
            }
        }

        plugins
    }

    /// Installs a plugin into the user plugins directory by copying a directory.
    pub fn install_from_dir(&self, source_dir: &Path) -> Result<InstalledPlugin> {
        let manifest_file = source_dir.join(PLUGIN_MANIFEST_FILENAME);
        if !manifest_file.is_file() {
            return Err(anyhow!(
                "Source directory does not contain {}",
                PLUGIN_MANIFEST_FILENAME
            ));
        }

        let content = fs::read_to_string(&manifest_file)
            .with_context(|| format!("Failed to read manifest from {:?}", manifest_file))?;
        let manifest: PluginManifest = serde_json::from_str(&content)
            .with_context(|| "Failed to parse plugin manifest JSON")?;

        let user_plugins_dir = crate::Config::config_dir().join("plugins");
        fs::create_dir_all(&user_plugins_dir)?;

        let dest_dir = user_plugins_dir.join(&manifest.name);
        if dest_dir.exists() {
            fs::remove_dir_all(&dest_dir)?;
        }
        copy_dir_all(source_dir, &dest_dir)?;

        Ok(InstalledPlugin {
            name: manifest.name.clone(),
            version: manifest
                .version
                .clone()
                .unwrap_or_else(|| "0.1.0".to_string()),
            description: manifest.description.clone().unwrap_or_default(),
            path: dest_dir,
            manifest,
            enabled: true,
        })
    }

    /// Uninstalls a plugin by removing its directory.
    pub fn uninstall(&self, name: &str) -> Result<bool> {
        let user_plugins_dir = crate::Config::config_dir().join("plugins");
        let target = user_plugins_dir.join(name);
        if target.exists() {
            fs::remove_dir_all(&target)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_plugin_manifest_parsing_and_discovery() {
        let tmp = tempdir().unwrap();
        let plugin_dir = tmp.path().join("code-analyzer");
        fs::create_dir_all(&plugin_dir).unwrap();

        let manifest_json = r#"{
            "name": "code-analyzer",
            "version": "1.2.0",
            "description": "Static code analysis plugin for Rust and Python",
            "keywords": ["rust", "python", "linter"],
            "mcp_servers": {
                "analyzer-mcp": {
                    "command": "cargo",
                    "args": ["clippy"]
                }
            }
        }"#;

        fs::write(plugin_dir.join(PLUGIN_MANIFEST_FILENAME), manifest_json).unwrap();

        let mgr = PluginManager::with_search_paths(vec![tmp.path().to_path_buf()]);
        let discovered = mgr.discover_plugins();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].name, "code-analyzer");
        assert_eq!(discovered[0].version, "1.2.0");
        assert_eq!(discovered[0].manifest.keywords.len(), 3);
        assert!(
            discovered[0]
                .manifest
                .mcp_servers
                .contains_key("analyzer-mcp")
        );
    }
}
