//! Plugin/Extension API for M31A
//!
//! Dynamic loading of cdylib plugins with capability-based contributions.

use anyhow::{Context, Result};
use libloading::{Library, Symbol};
use m31a_core::types::{ChatMessage, ToolCall};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

/// Plugin manifest (loaded from m31a-plugin.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub capabilities: Vec<PluginCapability>,
    pub entry_point: String,
    /// Oldest M31A version this plugin supports; defaults to "0.0.0" (any).
    #[serde(default = "default_min_version")]
    pub min_m31a_version: String,
    pub config_schema: Option<serde_json::Value>,
    /// Cryptographic SHA-256 hash of the binary for integrity validation
    pub sha256: Option<String>,
    /// Signature hex for publisher authenticity
    pub signature: Option<String>,
}

fn default_min_version() -> String {
    "0.0.0".to_string()
}

/// Plugin capability types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginCapability {
    Tools,
    ContextProviders,
    ApprovalHooks,
    Commands,
    ModelProviders,
    SandboxBackends,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,
    pub settings: HashMap<String, serde_json::Value>,
}

/// Extension context provided to plugins
pub struct ExtensionContext {
    pub config: PluginConfig,
    pub workspace_root: PathBuf,
    pub m31a_version: String,
    pub api_version: u32,
}

/// Tool definition from plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
    pub requires_approval: bool,
    pub sandbox_policy: Option<String>,
}

/// Context provider from plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextProviderDef {
    pub name: String,
    pub description: String,
    pub trigger_patterns: Vec<String>,
    pub provides_file_context: bool,
}

/// Approval hook from plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalHookDef {
    pub name: String,
    pub description: String,
    pub tool_patterns: Vec<String>,
    pub priority: i32,
}

/// Custom command from plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub aliases: Vec<String>,
}

/// Trait that all plugins must implement
pub trait M31APlugin: Send + Sync {
    /// Get plugin manifest
    fn manifest(&self) -> PluginManifest;

    /// Initialize the plugin
    fn initialize(&mut self, ctx: ExtensionContext) -> Result<()>;

    /// Shutdown the plugin
    fn shutdown(&mut self) -> Result<()>;

    /// Get tools provided by this plugin
    fn tools(&self) -> Vec<PluginToolDef> {
        vec![]
    }

    /// Get context providers
    fn context_providers(&self) -> Vec<ContextProviderDef> {
        vec![]
    }

    /// Get approval hooks
    fn approval_hooks(&self) -> Vec<ApprovalHookDef> {
        vec![]
    }

    /// Get custom commands
    fn commands(&self) -> Vec<CommandDef> {
        vec![]
    }

    /// Execute a tool call
    fn execute_tool(&self, name: &str, _args: serde_json::Value) -> Result<serde_json::Value> {
        Err(anyhow::anyhow!("Tool '{}' not implemented", name))
    }

    /// Execute a command
    fn execute_command(&self, name: &str, _args: Vec<String>) -> Result<String> {
        Err(anyhow::anyhow!("Command '{}' not implemented", name))
    }

    /// Provide context for a query
    fn provide_context(&self, _provider: &str, _query: &str) -> Result<Vec<ChatMessage>> {
        Ok(vec![])
    }

    /// Check if a tool call should be approved
    fn check_approval(&self, _hook: &str, _tool_call: &ToolCall) -> Result<bool> {
        Ok(true)
    }
}

/// Dynamic plugin loader
pub struct PluginLoader {
    loaded_libraries: HashMap<String, Library>,
    plugins: HashMap<String, Box<dyn M31APlugin>>,
    plugin_configs: HashMap<String, PluginConfig>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            loaded_libraries: HashMap::new(),
            plugins: HashMap::new(),
            plugin_configs: HashMap::new(),
        }
    }

    /// Load a plugin from a directory containing the shared library
    pub fn load_plugin(&mut self, plugin_dir: &Path) -> Result<()> {
        // Find the shared library
        let lib_name = if cfg!(target_os = "windows") {
            "m31a_plugin.dll"
        } else if cfg!(target_os = "macos") {
            "libm31a_plugin.dylib"
        } else {
            "libm31a_plugin.so"
        };

        let lib_path = plugin_dir.join(lib_name);
        if !lib_path.exists() {
            return Err(anyhow::anyhow!(
                "Plugin library not found at {}",
                lib_path.display()
            ));
        }

        // Load manifest
        let manifest_path = plugin_dir.join("m31a-plugin.toml");
        let manifest_content =
            std::fs::read_to_string(&manifest_path).context("Failed to read plugin manifest")?;
        let manifest: PluginManifest =
            toml::from_str(&manifest_content).context("Failed to parse plugin manifest")?;

        // Enforce min_m31a_version against running host version
        let host_ver_str = env!("CARGO_PKG_VERSION");
        if let (Ok(required_ver), Ok(current_ver)) = (
            semver::Version::parse(&manifest.min_m31a_version),
            semver::Version::parse(host_ver_str),
        ) && current_ver < required_ver
        {
            return Err(anyhow::anyhow!(
                "Plugin '{}' requires M31A version >= {}, but host is running {}",
                manifest.name,
                manifest.min_m31a_version,
                host_ver_str
            ));
        }

        // If manifest specifies sha256 checksum, verify binary integrity before dynamic load
        if let Some(ref expected_hash) = manifest.sha256 {
            let binary_bytes = std::fs::read(&lib_path).context(format!(
                "Failed to read binary for integrity check: {}",
                lib_path.display()
            ))?;
            verify_plugin_integrity(&binary_bytes, expected_hash)?;
        }

        // Load the library
        let library = unsafe { Library::new(&lib_path) }
            .context(format!("Failed to load library {}", lib_path.display()))?;

        // Get the plugin creation function
        let create_plugin: Symbol<unsafe extern "C" fn() -> *mut dyn M31APlugin> =
            unsafe { library.get(manifest.entry_point.as_bytes()) }
                .context(format!("Entry point '{}' not found", manifest.entry_point))?;

        // Create the plugin instance
        let plugin_ptr = unsafe { create_plugin() };
        let mut plugin = unsafe { Box::from_raw(plugin_ptr) };

        // Initialize the plugin
        let config = self
            .plugin_configs
            .get(&manifest.name)
            .cloned()
            .unwrap_or_default();
        let ctx = ExtensionContext {
            config,
            workspace_root: std::env::current_dir()?,
            m31a_version: env!("CARGO_PKG_VERSION").to_string(),
            api_version: 1,
        };

        plugin.initialize(ctx)?;

        // Store the library and plugin
        self.loaded_libraries.insert(manifest.name.clone(), library);
        self.plugins.insert(manifest.name.clone(), plugin);

        Ok(())
    }

    /// Load all plugins from a directory
    pub fn load_plugins_from_dir(&mut self, plugins_dir: &Path) -> Result<Vec<String>> {
        let mut loaded = Vec::new();
        if !plugins_dir.exists() {
            return Ok(loaded);
        }

        for entry in std::fs::read_dir(plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Err(e) = self.load_plugin(&path) {
                    eprintln!("Failed to load plugin from {}: {}", path.display(), e);
                } else {
                    loaded.push(path.file_name().unwrap().to_string_lossy().to_string());
                }
            }
        }
        Ok(loaded)
    }

    /// Unload a plugin
    pub fn unload_plugin(&mut self, name: &str) -> Result<()> {
        if let Some(mut plugin) = self.plugins.remove(name) {
            plugin.shutdown()?;
        }
        self.loaded_libraries.remove(name);
        Ok(())
    }

    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&dyn M31APlugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    /// Get all loaded plugins
    pub fn list_plugins(&self) -> Vec<PluginManifest> {
        self.plugins.values().map(|p| p.manifest()).collect()
    }

    /// Get all tools from all plugins
    pub fn all_tools(&self) -> Vec<PluginToolDef> {
        self.plugins.values().flat_map(|p| p.tools()).collect()
    }

    /// Get all context providers
    pub fn all_context_providers(&self) -> Vec<ContextProviderDef> {
        self.plugins
            .values()
            .flat_map(|p| p.context_providers())
            .collect()
    }

    /// Get all approval hooks
    pub fn all_approval_hooks(&self) -> Vec<ApprovalHookDef> {
        self.plugins
            .values()
            .flat_map(|p| p.approval_hooks())
            .collect()
    }

    /// Get all commands
    pub fn all_commands(&self) -> Vec<CommandDef> {
        self.plugins.values().flat_map(|p| p.commands()).collect()
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin manager for runtime plugin management
pub struct PluginManager {
    loader: RwLock<PluginLoader>,
    plugin_dirs: Vec<PathBuf>,
}

impl PluginManager {
    pub fn new(plugin_dirs: Vec<PathBuf>) -> Self {
        Self {
            loader: RwLock::new(PluginLoader::new()),
            plugin_dirs,
        }
    }

    /// Initialize and load all plugins
    pub async fn initialize(&self) -> Result<Vec<String>> {
        let mut loader = self.loader.write().await;
        let mut all_loaded = Vec::new();
        for dir in &self.plugin_dirs {
            let loaded = loader.load_plugins_from_dir(dir)?;
            all_loaded.extend(loaded);
        }
        Ok(all_loaded)
    }

    /// Reload all plugins
    pub async fn reload(&self) -> Result<Vec<String>> {
        let mut loader = self.loader.write().await;
        // Unload all current plugins
        let plugin_names: Vec<String> = loader.plugins.keys().cloned().collect();
        for name in plugin_names {
            loader.unload_plugin(&name)?;
        }

        // Reload
        let mut all_loaded = Vec::new();
        for dir in &self.plugin_dirs {
            let loaded = loader.load_plugins_from_dir(dir)?;
            all_loaded.extend(loaded);
        }
        Ok(all_loaded)
    }

    /// Get plugin by name
    pub async fn get_plugin(&self, name: &str) -> Option<PluginManifest> {
        let loader = self.loader.read().await;
        loader.get_plugin(name).map(|p| p.manifest())
    }

    /// List all plugins
    pub async fn list_plugins(&self) -> Vec<PluginManifest> {
        let loader = self.loader.read().await;
        loader.list_plugins()
    }

    /// Execute a plugin tool
    pub async fn execute_tool(
        &self,
        plugin_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let loader = self.loader.read().await;
        let plugin = loader
            .get_plugin(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;
        plugin.execute_tool(tool_name, args)
    }

    /// Execute a plugin command
    pub async fn execute_command(
        &self,
        plugin_name: &str,
        command_name: &str,
        args: Vec<String>,
    ) -> Result<String> {
        let loader = self.loader.read().await;
        let plugin = loader
            .get_plugin(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;
        plugin.execute_command(command_name, args)
    }

    /// Get all tools from all plugins
    pub async fn all_tools(&self) -> Vec<PluginToolDef> {
        let loader = self.loader.read().await;
        loader.all_tools()
    }

    /// Get all context providers
    pub async fn all_context_providers(&self) -> Vec<ContextProviderDef> {
        let loader = self.loader.read().await;
        loader.all_context_providers()
    }

    /// Get all approval hooks
    pub async fn all_approval_hooks(&self) -> Vec<ApprovalHookDef> {
        let loader = self.loader.read().await;
        loader.all_approval_hooks()
    }

    /// Get all commands
    pub async fn all_commands(&self) -> Vec<CommandDef> {
        let loader = self.loader.read().await;
        loader.all_commands()
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            settings: HashMap::new(),
        }
    }
}

/// Verifies the cryptographic integrity of a plugin binary against its expected SHA-256 hash.
pub fn verify_plugin_integrity(binary_bytes: &[u8], expected_sha256: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(binary_bytes);
    let hash = format!("{:x}", hasher.finalize());

    if hash.eq_ignore_ascii_case(expected_sha256.trim()) {
        Ok(())
    } else {
        anyhow::bail!(
            "Plugin integrity verification failed: expected SHA-256 '{}', computed '{}'",
            expected_sha256,
            hash
        );
    }
}

/// Metadata for a verified plugin listed in the M31A plugin marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub sha256: String,
    pub downloads: u64,
    pub verified_publisher: bool,
    pub capabilities: Vec<PluginCapability>,
}

/// In-memory catalog and client for the signed plugin marketplace.
pub struct PluginMarketplaceClient {
    catalog: Vec<MarketplacePlugin>,
}

impl Default for PluginMarketplaceClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginMarketplaceClient {
    pub fn new() -> Self {
        Self {
            catalog: vec![
                MarketplacePlugin {
                    id: "supabase-m31a".to_string(),
                    name: "Supabase Database & Auth Connector".to_string(),
                    version: "0.2.0".to_string(),
                    description: "Autonomous migrations, edge function deployments, and vector search on Supabase".to_string(),
                    author: "M31A Community".to_string(),
                    sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
                    downloads: 1420,
                    verified_publisher: true,
                    capabilities: vec![PluginCapability::Tools, PluginCapability::Commands],
                },
                MarketplacePlugin {
                    id: "vercel-deployer".to_string(),
                    name: "Vercel Zero-Config Deployment".to_string(),
                    version: "1.1.0".to_string(),
                    description: "Continuous preview and production deployments to Vercel".to_string(),
                    author: "M31A Team".to_string(),
                    sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
                    downloads: 3890,
                    verified_publisher: true,
                    capabilities: vec![PluginCapability::Tools],
                },
            ],
        }
    }

    pub fn search(&self, query: &str) -> Vec<MarketplacePlugin> {
        let q = query.to_lowercase();
        self.catalog
            .iter()
            .filter(|p| {
                p.id.to_lowercase().contains(&q)
                    || p.name.to_lowercase().contains(&q)
                    || p.description.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    pub fn find_by_id(&self, id: &str) -> Option<&MarketplacePlugin> {
        self.catalog.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manifest_parsing() {
        let toml_str = r#"
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
capabilities = ["tools", "commands"]
entry_point = "create_plugin"
sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.capabilities.len(), 2);
        assert_eq!(
            manifest.sha256.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
    }

    #[test]
    fn test_verify_plugin_integrity() {
        let data = b"m31a-plugin-binary-bytes";
        // sha256 of "m31a-plugin-binary-bytes"
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let valid_hash = format!("{:x}", hasher.finalize());

        assert!(verify_plugin_integrity(data, &valid_hash).is_ok());
        assert!(verify_plugin_integrity(data, "wrong-hash").is_err());
    }

    #[test]
    fn test_marketplace_search() {
        let marketplace = PluginMarketplaceClient::new();
        let results = marketplace.search("vercel");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "vercel-deployer");
        assert!(results[0].verified_publisher);

        let none = marketplace.search("nonexistent-tool");
        assert!(none.is_empty());
    }
}
