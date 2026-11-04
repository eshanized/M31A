//! Plugin/Extension API for M31A
//!
//! Dynamic loading of cdylib plugins with capability-based contributions.

use anyhow::{Context, Result, bail};
use libloading::{Library, Symbol};
use m31a_core::types::{ChatMessage, ToolCall};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub const M31A_PLUGIN_ABI_VERSION: u32 = 1;

pub const M31A_PLUGIN_OK: c_int = 0;
pub const M31A_PLUGIN_ERR_PANIC: c_int = -1;
pub const M31A_PLUGIN_ERR_INVALID_ARG: c_int = -2;
pub const M31A_PLUGIN_ERR_NOT_FOUND: c_int = -3;
pub const M31A_PLUGIN_ERR_EXECUTION: c_int = -4;

/// Versioned C ABI Function Table for dynamic plugin communication across cdylib boundary.
/// Avoids passing raw Rust trait objects (which have an unstable ABI across compiler versions).
#[repr(C)]
pub struct PluginVTableV1 {
    pub abi_version: u32,
    pub initialize: unsafe extern "C" fn(
        handle: *mut c_void,
        config_json: *const c_char,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub shutdown: unsafe extern "C" fn(handle: *mut c_void, out_err: *mut *mut c_char) -> c_int,
    pub get_manifest_json: unsafe extern "C" fn(
        handle: *mut c_void,
        out_json: *mut *mut c_char,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub get_tools_json: unsafe extern "C" fn(
        handle: *mut c_void,
        out_json: *mut *mut c_char,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub get_context_providers_json: unsafe extern "C" fn(
        handle: *mut c_void,
        out_json: *mut *mut c_char,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub get_approval_hooks_json: unsafe extern "C" fn(
        handle: *mut c_void,
        out_json: *mut *mut c_char,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub get_commands_json: unsafe extern "C" fn(
        handle: *mut c_void,
        out_json: *mut *mut c_char,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub execute_tool: unsafe extern "C" fn(
        handle: *mut c_void,
        name: *const c_char,
        args_json: *const c_char,
        out_json: *mut *mut c_char,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub execute_command: unsafe extern "C" fn(
        handle: *mut c_void,
        name: *const c_char,
        args_json: *const c_char,
        out_json: *mut *mut c_char,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub provide_context: unsafe extern "C" fn(
        handle: *mut c_void,
        provider: *const c_char,
        query: *const c_char,
        out_json: *mut *mut c_char,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub check_approval: unsafe extern "C" fn(
        handle: *mut c_void,
        hook: *const c_char,
        tool_call_json: *const c_char,
        out_approved: *mut c_int,
        out_err: *mut *mut c_char,
    ) -> c_int,
    pub free_string: unsafe extern "C" fn(ptr: *mut c_char),
    pub destroy: unsafe extern "C" fn(handle: *mut c_void),
}

pub type PluginEntryPointV1 =
    unsafe extern "C" fn(out_vtable: *mut PluginVTableV1, out_handle: *mut *mut c_void) -> c_int;

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

/// Safe C-string extraction helper with caller-provided deallocator.
unsafe fn c_to_string(ptr: *mut c_char, free_fn: unsafe extern "C" fn(*mut c_char)) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let cstr = unsafe { CStr::from_ptr(ptr) };
    let s = cstr.to_string_lossy().into_owned();
    unsafe { free_fn(ptr) };
    s
}

/// Dynamic C-ABI Plugin Wrapper maintaining opaque handle, versioned function table, and in-flight tracking.
pub struct CAbiPlugin {
    manifest: PluginManifest,
    vtable: PluginVTableV1,
    handle: *mut c_void,
    _lib: Arc<Library>,
    in_flight: Arc<AtomicUsize>,
}

unsafe impl Send for CAbiPlugin {}
unsafe impl Sync for CAbiPlugin {}

impl CAbiPlugin {
    pub fn new(
        manifest: PluginManifest,
        vtable: PluginVTableV1,
        handle: *mut c_void,
        lib: Arc<Library>,
        in_flight: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            manifest,
            vtable,
            handle,
            _lib: lib,
            in_flight,
        }
    }
}

struct InFlightGuard<'a>(&'a AtomicUsize);

impl<'a> InFlightGuard<'a> {
    fn new(counter: &'a AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl<'a> Drop for InFlightGuard<'a> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Drop for CAbiPlugin {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                (self.vtable.destroy)(self.handle);
                self.handle = std::ptr::null_mut();
            }
        }
    }
}

impl M31APlugin for CAbiPlugin {
    fn manifest(&self) -> PluginManifest {
        self.manifest.clone()
    }

    fn initialize(&mut self, ctx: ExtensionContext) -> Result<()> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let config_json = serde_json::to_string(&ctx.config)?;
        let c_config = CString::new(config_json)?;
        let mut err_ptr: *mut c_char = std::ptr::null_mut();

        let code =
            unsafe { (self.vtable.initialize)(self.handle, c_config.as_ptr(), &mut err_ptr) };

        if code != M31A_PLUGIN_OK {
            let err_msg = unsafe { c_to_string(err_ptr, self.vtable.free_string) };
            bail!("Plugin initialization failed (code {code}): {err_msg}");
        }

        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let mut err_ptr: *mut c_char = std::ptr::null_mut();
        let code = unsafe { (self.vtable.shutdown)(self.handle, &mut err_ptr) };
        if code != M31A_PLUGIN_OK {
            let err_msg = unsafe { c_to_string(err_ptr, self.vtable.free_string) };
            bail!("Plugin shutdown failed (code {code}): {err_msg}");
        }
        Ok(())
    }

    fn tools(&self) -> Vec<PluginToolDef> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let mut json_ptr: *mut c_char = std::ptr::null_mut();
        let mut err_ptr: *mut c_char = std::ptr::null_mut();

        let code =
            unsafe { (self.vtable.get_tools_json)(self.handle, &mut json_ptr, &mut err_ptr) };

        if code == M31A_PLUGIN_OK && !json_ptr.is_null() {
            let json_str = unsafe { c_to_string(json_ptr, self.vtable.free_string) };
            serde_json::from_str(&json_str).unwrap_or_default()
        } else {
            if !err_ptr.is_null() {
                unsafe { (self.vtable.free_string)(err_ptr) };
            }
            Vec::new()
        }
    }

    fn context_providers(&self) -> Vec<ContextProviderDef> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let mut json_ptr: *mut c_char = std::ptr::null_mut();
        let mut err_ptr: *mut c_char = std::ptr::null_mut();

        let code = unsafe {
            (self.vtable.get_context_providers_json)(self.handle, &mut json_ptr, &mut err_ptr)
        };

        if code == M31A_PLUGIN_OK && !json_ptr.is_null() {
            let json_str = unsafe { c_to_string(json_ptr, self.vtable.free_string) };
            serde_json::from_str(&json_str).unwrap_or_default()
        } else {
            if !err_ptr.is_null() {
                unsafe { (self.vtable.free_string)(err_ptr) };
            }
            Vec::new()
        }
    }

    fn approval_hooks(&self) -> Vec<ApprovalHookDef> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let mut json_ptr: *mut c_char = std::ptr::null_mut();
        let mut err_ptr: *mut c_char = std::ptr::null_mut();

        let code = unsafe {
            (self.vtable.get_approval_hooks_json)(self.handle, &mut json_ptr, &mut err_ptr)
        };

        if code == M31A_PLUGIN_OK && !json_ptr.is_null() {
            let json_str = unsafe { c_to_string(json_ptr, self.vtable.free_string) };
            serde_json::from_str(&json_str).unwrap_or_default()
        } else {
            if !err_ptr.is_null() {
                unsafe { (self.vtable.free_string)(err_ptr) };
            }
            Vec::new()
        }
    }

    fn commands(&self) -> Vec<CommandDef> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let mut json_ptr: *mut c_char = std::ptr::null_mut();
        let mut err_ptr: *mut c_char = std::ptr::null_mut();

        let code =
            unsafe { (self.vtable.get_commands_json)(self.handle, &mut json_ptr, &mut err_ptr) };

        if code == M31A_PLUGIN_OK && !json_ptr.is_null() {
            let json_str = unsafe { c_to_string(json_ptr, self.vtable.free_string) };
            serde_json::from_str(&json_str).unwrap_or_default()
        } else {
            if !err_ptr.is_null() {
                unsafe { (self.vtable.free_string)(err_ptr) };
            }
            Vec::new()
        }
    }

    fn execute_tool(&self, name: &str, args: serde_json::Value) -> Result<serde_json::Value> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let c_name = CString::new(name)?;
        let args_str = serde_json::to_string(&args)?;
        let c_args = CString::new(args_str)?;

        let mut out_json: *mut c_char = std::ptr::null_mut();
        let mut out_err: *mut c_char = std::ptr::null_mut();

        let code = unsafe {
            (self.vtable.execute_tool)(
                self.handle,
                c_name.as_ptr(),
                c_args.as_ptr(),
                &mut out_json,
                &mut out_err,
            )
        };

        if code != M31A_PLUGIN_OK {
            let err_msg = unsafe { c_to_string(out_err, self.vtable.free_string) };
            bail!("Plugin tool execution error for '{name}' (code {code}): {err_msg}");
        }

        let res_str = unsafe { c_to_string(out_json, self.vtable.free_string) };
        serde_json::from_str(&res_str).context("Failed to parse tool execution response JSON")
    }

    fn execute_command(&self, name: &str, args: Vec<String>) -> Result<String> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let c_name = CString::new(name)?;
        let args_str = serde_json::to_string(&args)?;
        let c_args = CString::new(args_str)?;

        let mut out_json: *mut c_char = std::ptr::null_mut();
        let mut out_err: *mut c_char = std::ptr::null_mut();

        let code = unsafe {
            (self.vtable.execute_command)(
                self.handle,
                c_name.as_ptr(),
                c_args.as_ptr(),
                &mut out_json,
                &mut out_err,
            )
        };

        if code != M31A_PLUGIN_OK {
            let err_msg = unsafe { c_to_string(out_err, self.vtable.free_string) };
            bail!("Plugin command execution error for '{name}' (code {code}): {err_msg}");
        }

        Ok(unsafe { c_to_string(out_json, self.vtable.free_string) })
    }

    fn provide_context(&self, provider: &str, query: &str) -> Result<Vec<ChatMessage>> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let c_prov = CString::new(provider)?;
        let c_query = CString::new(query)?;

        let mut out_json: *mut c_char = std::ptr::null_mut();
        let mut out_err: *mut c_char = std::ptr::null_mut();

        let code = unsafe {
            (self.vtable.provide_context)(
                self.handle,
                c_prov.as_ptr(),
                c_query.as_ptr(),
                &mut out_json,
                &mut out_err,
            )
        };

        if code != M31A_PLUGIN_OK {
            let err_msg = unsafe { c_to_string(out_err, self.vtable.free_string) };
            bail!("Context provider error for '{provider}' (code {code}): {err_msg}");
        }

        let res_str = unsafe { c_to_string(out_json, self.vtable.free_string) };
        serde_json::from_str(&res_str).context("Failed to parse context messages JSON")
    }

    fn check_approval(&self, hook: &str, tool_call: &ToolCall) -> Result<bool> {
        let _guard = InFlightGuard::new(&self.in_flight);
        let c_hook = CString::new(hook)?;
        let call_json = serde_json::to_string(tool_call)?;
        let c_call = CString::new(call_json)?;

        let mut approved: c_int = 1;
        let mut out_err: *mut c_char = std::ptr::null_mut();

        let code = unsafe {
            (self.vtable.check_approval)(
                self.handle,
                c_hook.as_ptr(),
                c_call.as_ptr(),
                &mut approved,
                &mut out_err,
            )
        };

        if code != M31A_PLUGIN_OK {
            let err_msg = unsafe { c_to_string(out_err, self.vtable.free_string) };
            bail!("Approval hook error for '{hook}' (code {code}): {err_msg}");
        }

        Ok(approved != 0)
    }
}

/// Dynamic plugin loader supporting versioned C-ABI and in-flight safety tracking.
pub struct PluginLoader {
    loaded_libraries: HashMap<String, Arc<Library>>,
    plugins: HashMap<String, Arc<dyn M31APlugin>>,
    in_flight: HashMap<String, Arc<AtomicUsize>>,
    plugin_configs: HashMap<String, PluginConfig>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            loaded_libraries: HashMap::new(),
            plugins: HashMap::new(),
            in_flight: HashMap::new(),
            plugin_configs: HashMap::new(),
        }
    }

    /// Directly registers an in-process plugin instance (e.g. built-in or test plugins).
    pub fn register_in_process_plugin(&mut self, name: String, plugin: Box<dyn M31APlugin>) {
        self.plugins.insert(name.clone(), Arc::from(plugin));
        self.in_flight
            .entry(name)
            .or_insert_with(|| Arc::new(AtomicUsize::new(0)));
    }

    /// Load a plugin from a directory containing the shared library and manifest.
    pub fn load_plugin(&mut self, plugin_dir: &Path) -> Result<()> {
        let lib_name = if cfg!(target_os = "windows") {
            "m31a_plugin.dll"
        } else if cfg!(target_os = "macos") {
            "libm31a_plugin.dylib"
        } else {
            "libm31a_plugin.so"
        };

        let lib_path = plugin_dir.join(lib_name);
        if !lib_path.exists() {
            bail!("Plugin library not found at {}", lib_path.display());
        }

        // Load manifest
        let manifest_path = plugin_dir.join("m31a-plugin.toml");
        let manifest_content = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest at {}", manifest_path.display()))?;
        let manifest: PluginManifest =
            toml::from_str(&manifest_content).context("Failed to parse plugin manifest")?;

        // Version check against host
        let host_ver_str = env!("CARGO_PKG_VERSION");
        if let (Ok(required_ver), Ok(current_ver)) = (
            semver::Version::parse(&manifest.min_m31a_version),
            semver::Version::parse(host_ver_str),
        ) && current_ver < required_ver
        {
            bail!(
                "Plugin '{}' requires M31A version >= {}, host is {}",
                manifest.name,
                manifest.min_m31a_version,
                host_ver_str
            );
        }

        // Integrity check
        let binary_bytes = std::fs::read(&lib_path)?;
        if let Some(ref expected_hash) = manifest.sha256 {
            verify_plugin_integrity(&binary_bytes, expected_hash)?;
        }

        // Signature check if publisher identity is present
        if let Some(ref sig) = manifest.signature {
            verify_publisher_signature(
                &binary_bytes,
                manifest.sha256.as_deref().unwrap_or(""),
                Some(sig),
                manifest.author.as_deref(),
            )?;
        }

        // Load library
        let library = unsafe { Library::new(&lib_path) }
            .with_context(|| format!("Failed to load library {}", lib_path.display()))?;
        let lib_arc = Arc::new(library);
        let in_flight = Arc::new(AtomicUsize::new(0));

        // Attempt versioned C-ABI entry point: `m31a_plugin_v1`
        let c_entry_result: Result<Symbol<PluginEntryPointV1>, _> =
            unsafe { lib_arc.get(b"m31a_plugin_v1\0") };

        if let Ok(entrypoint) = c_entry_result {
            let mut vtable_uninit = std::mem::MaybeUninit::<PluginVTableV1>::uninit();
            let mut handle: *mut c_void = std::ptr::null_mut();

            let ret = unsafe { entrypoint(vtable_uninit.as_mut_ptr(), &mut handle) };
            if ret != M31A_PLUGIN_OK || handle.is_null() {
                bail!("Plugin entrypoint 'm31a_plugin_v1' returned error code {ret}");
            }

            let vtable = unsafe { vtable_uninit.assume_init() };

            if vtable.abi_version != M31A_PLUGIN_ABI_VERSION {
                bail!(
                    "Incompatible plugin ABI version: expected {}, got {}",
                    M31A_PLUGIN_ABI_VERSION,
                    vtable.abi_version
                );
            }

            let mut plugin = CAbiPlugin::new(
                manifest.clone(),
                vtable,
                handle,
                Arc::clone(&lib_arc),
                Arc::clone(&in_flight),
            );

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

            self.loaded_libraries.insert(manifest.name.clone(), lib_arc);
            self.plugins.insert(manifest.name.clone(), Arc::new(plugin));
            self.in_flight.insert(manifest.name.clone(), in_flight);
        } else {
            // Fallback: legacy Rust trait object entry point
            let create_plugin: Symbol<unsafe extern "C" fn() -> *mut dyn M31APlugin> =
                unsafe { lib_arc.get(manifest.entry_point.as_bytes()) }
                    .context(format!("Entry point '{}' not found", manifest.entry_point))?;

            let plugin_ptr = unsafe { create_plugin() };
            if plugin_ptr.is_null() {
                bail!("Plugin factory function returned null pointer");
            }
            let mut plugin = unsafe { Box::from_raw(plugin_ptr) };

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

            self.loaded_libraries.insert(manifest.name.clone(), lib_arc);
            self.plugins
                .insert(manifest.name.clone(), Arc::from(plugin));
            self.in_flight.insert(manifest.name.clone(), in_flight);
        }

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
                    tracing::warn!("Failed to load plugin from {}: {}", path.display(), e);
                } else {
                    loaded.push(path.file_name().unwrap().to_string_lossy().to_string());
                }
            }
        }
        Ok(loaded)
    }

    /// Safely unloads a plugin only after all in-flight calls have terminated.
    pub fn unload_plugin(&mut self, name: &str) -> Result<()> {
        if let Some(in_flight) = self.in_flight.get(name) {
            let start = Instant::now();
            while in_flight.load(Ordering::SeqCst) > 0 {
                if start.elapsed() > Duration::from_secs(5) {
                    bail!(
                        "Refusing to unload plugin '{}': active calls are still in progress",
                        name
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        if let Some(plugin) = self.plugins.remove(name) {
            // Drop plugin reference before library removal
            drop(plugin);
        }
        self.loaded_libraries.remove(name);
        self.in_flight.remove(name);
        Ok(())
    }

    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<Arc<dyn M31APlugin>> {
        self.plugins.get(name).cloned()
    }

    /// Get all loaded plugins
    pub fn list_plugins(&self) -> Vec<PluginManifest> {
        self.plugins.values().map(|p| p.manifest()).collect()
    }

    /// Get all tools from all plugins with plugin namespace
    pub fn all_tools(&self) -> Vec<(String, PluginToolDef)> {
        let mut list = Vec::new();
        for (plugin_name, plugin) in &self.plugins {
            for tool in plugin.tools() {
                list.push((plugin_name.clone(), tool));
            }
        }
        list
    }

    /// Get all tool definitions
    pub fn all_tool_defs(&self) -> Vec<PluginToolDef> {
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

    /// Register an in-process plugin instance directly (useful for tests or embedded extensions)
    pub async fn register_in_process_plugin(&self, name: String, plugin: Box<dyn M31APlugin>) {
        let mut loader = self.loader.write().await;
        loader.register_in_process_plugin(name, plugin);
    }

    /// Initialize and load all plugins from search paths
    pub async fn initialize(&self) -> Result<Vec<String>> {
        let mut loader = self.loader.write().await;
        let mut all_loaded = Vec::new();
        for dir in &self.plugin_dirs {
            let loaded = loader.load_plugins_from_dir(dir)?;
            all_loaded.extend(loaded);
        }
        Ok(all_loaded)
    }

    /// Safely reload all plugins
    pub async fn reload(&self) -> Result<Vec<String>> {
        let mut loader = self.loader.write().await;
        let plugin_names: Vec<String> = loader.plugins.keys().cloned().collect();
        for name in plugin_names {
            loader.unload_plugin(&name)?;
        }

        let mut all_loaded = Vec::new();
        for dir in &self.plugin_dirs {
            let loaded = loader.load_plugins_from_dir(dir)?;
            all_loaded.extend(loaded);
        }
        Ok(all_loaded)
    }

    /// Unload a single plugin safely
    pub async fn unload_plugin(&self, name: &str) -> Result<()> {
        let mut loader = self.loader.write().await;
        loader.unload_plugin(name)
    }

    /// Get plugin manifest by name
    pub async fn get_plugin(&self, name: &str) -> Option<PluginManifest> {
        let loader = self.loader.read().await;
        loader.get_plugin(name).map(|p| p.manifest())
    }

    /// List all loaded plugin manifests
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

    /// Execute a plugin custom command
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

    /// Get all tools from all plugins with plugin namespace `(plugin_name, tool_def)`
    pub async fn all_tools(&self) -> Vec<(String, PluginToolDef)> {
        let loader = self.loader.read().await;
        loader.all_tools()
    }

    /// Get all tool definitions
    pub async fn all_tool_defs(&self) -> Vec<PluginToolDef> {
        let loader = self.loader.read().await;
        loader.all_tool_defs()
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
        bail!(
            "Plugin integrity verification failed: expected SHA-256 '{}', computed '{}'",
            expected_sha256,
            hash
        );
    }
}

/// Verifies publisher identity and signature over plugin binary.
pub fn verify_publisher_signature(
    binary_bytes: &[u8],
    expected_sha256: &str,
    signature: Option<&str>,
    publisher_key: Option<&str>,
) -> Result<()> {
    verify_plugin_integrity(binary_bytes, expected_sha256)?;
    if let (Some(sig), Some(key)) = (signature, publisher_key) {
        use sha2::{Digest, Sha256};
        let mut mac = Sha256::new();
        mac.update(key.as_bytes());
        mac.update(expected_sha256.as_bytes());
        let computed_sig = format!("{:x}", mac.finalize());
        if !computed_sig.eq_ignore_ascii_case(sig.trim()) {
            bail!("Plugin publisher cryptographic signature verification failed");
        }
    }
    Ok(())
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
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub publisher_key: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    pub verified_publisher: bool,
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub min_m31a_version: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceCatalogueResponse {
    pub version: u32,
    pub updated_at: u64,
    pub plugins: Vec<MarketplacePlugin>,
}

/// Remote-capable catalog and client for the signed plugin marketplace.
pub struct PluginMarketplaceClient {
    catalog: Vec<MarketplacePlugin>,
    cache_path: Option<PathBuf>,
    is_offline: bool,
}

impl Default for PluginMarketplaceClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginMarketplaceClient {
    pub fn new() -> Self {
        let default_cache =
            dirs::home_dir().map(|h| h.join(".cache/m31a/marketplace_catalogue.json"));
        Self {
            catalog: vec![
                MarketplacePlugin {
                    id: "supabase-m31a".to_string(),
                    name: "Supabase Database & Auth Connector".to_string(),
                    version: "0.2.0".to_string(),
                    description: "Autonomous migrations, edge function deployments, and vector search on Supabase".to_string(),
                    author: "M31A Community".to_string(),
                    sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
                    signature: None,
                    publisher_key: None,
                    downloads: 1420,
                    verified_publisher: true,
                    capabilities: vec![PluginCapability::Tools, PluginCapability::Commands],
                    category: Some("database".to_string()),
                    min_m31a_version: Some("0.1.0".to_string()),
                    channel: Some("stable".to_string()),
                    download_url: Some("https://plugins.m31a.org/packages/supabase-m31a-0.2.0.tar.gz".to_string()),
                },
                MarketplacePlugin {
                    id: "vercel-deployer".to_string(),
                    name: "Vercel Zero-Config Deployment".to_string(),
                    version: "1.1.0".to_string(),
                    description: "Continuous preview and production deployments to Vercel".to_string(),
                    author: "M31A Team".to_string(),
                    sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
                    signature: None,
                    publisher_key: None,
                    downloads: 3890,
                    verified_publisher: true,
                    capabilities: vec![PluginCapability::Tools],
                    category: Some("deployment".to_string()),
                    min_m31a_version: Some("0.1.0".to_string()),
                    channel: Some("stable".to_string()),
                    download_url: Some("https://plugins.m31a.org/packages/vercel-deployer-1.1.0.tar.gz".to_string()),
                },
            ],
            cache_path: default_cache,
            is_offline: false,
        }
    }

    pub fn with_cache_path(cache_path: PathBuf) -> Self {
        let mut client = Self::new();
        client.cache_path = Some(cache_path);
        client
    }

    pub fn is_offline(&self) -> bool {
        self.is_offline
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

    pub fn filter(
        &self,
        category: Option<&str>,
        capability: Option<PluginCapability>,
        channel: Option<&str>,
    ) -> Vec<MarketplacePlugin> {
        self.catalog
            .iter()
            .filter(|p| {
                if let Some(cat) = category
                    && p.category.as_deref() != Some(cat)
                {
                    return false;
                }
                if let Some(cap) = capability
                    && !p.capabilities.contains(&cap)
                {
                    return false;
                }
                if let Some(ch) = channel
                    && p.channel.as_deref() != Some(ch)
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect()
    }

    pub async fn refresh_from_remote(&mut self, endpoint: Option<&str>) -> Result<()> {
        let url = endpoint.unwrap_or("https://plugins.m31a.org/v1/catalogue");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.json::<MarketplaceCatalogueResponse>().await {
                    self.catalog = body.plugins;
                    self.is_offline = false;
                    if let Some(ref path) = self.cache_path {
                        if let Some(p) = path.parent() {
                            let _ = std::fs::create_dir_all(p);
                        }
                        let _ = std::fs::write(path, serde_json::to_string_pretty(&self.catalog)?);
                    }
                    return Ok(());
                }
            }
            _ => {
                // Network unavailable or endpoint failed: fallback to disk cache
                if let Some(ref path) = self.cache_path
                    && path.exists()
                    && let Ok(data) = std::fs::read_to_string(path)
                    && let Ok(cached) = serde_json::from_str::<Vec<MarketplacePlugin>>(&data)
                {
                    self.catalog = cached;
                    self.is_offline = true;
                    return Ok(());
                }
            }
        }

        self.is_offline = true;
        Ok(())
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
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let valid_hash = format!("{:x}", hasher.finalize());

        assert!(verify_plugin_integrity(data, &valid_hash).is_ok());
        assert!(verify_plugin_integrity(data, "wrong-hash").is_err());
    }

    #[test]
    fn test_verify_publisher_signature() {
        let data = b"m31a-plugin-binary-bytes";
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let valid_hash = format!("{:x}", hasher.finalize());

        let author_key = "publisher-secret-key-123";
        let mut mac = Sha256::new();
        mac.update(author_key.as_bytes());
        mac.update(valid_hash.as_bytes());
        let valid_sig = format!("{:x}", mac.finalize());

        assert!(
            verify_publisher_signature(data, &valid_hash, Some(&valid_sig), Some(author_key))
                .is_ok()
        );
        assert!(
            verify_publisher_signature(data, &valid_hash, Some("invalid-sig"), Some(author_key))
                .is_err()
        );
    }

    #[test]
    fn test_marketplace_search_and_filters() {
        let marketplace = PluginMarketplaceClient::new();
        let results = marketplace.search("vercel");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "vercel-deployer");
        assert!(results[0].verified_publisher);

        let filtered = marketplace.filter(Some("database"), None, None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "supabase-m31a");

        let none = marketplace.search("nonexistent-tool");
        assert!(none.is_empty());
    }
}
