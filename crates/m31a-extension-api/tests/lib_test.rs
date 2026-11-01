//! Dedicated integration tests for `m31a-extension-api` (src/lib.rs).
//!
//! Validates:
//! - Cryptographic SHA-256 plugin binary integrity verification
//! - PluginManifest TOML parsing, min_m31a_version bounds, and serialization
//! - PluginMarketplaceClient catalog queries, text search, and publisher verification
//! - In-memory implementation of M31APlugin exercising tools, commands, approvals, and context providers
//! - Error handling for missing libraries and non-existent plugins

use m31a_core::types::{FunctionCall, ToolCall};
use m31a_extension_api::{
    CommandDef, ExtensionContext, M31APlugin, PluginCapability, PluginConfig, PluginLoader,
    PluginManifest, PluginMarketplaceClient, PluginToolDef, verify_plugin_integrity,
};
use serde_json::json;
use std::path::PathBuf;
use tempfile::tempdir;

struct TestEmbeddedPlugin {
    initialized: bool,
}

impl M31APlugin for TestEmbeddedPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            name: "test-embedded-plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "Embedded in-process test plugin".to_string(),
            author: Some("Tester".to_string()),
            repository: None,
            license: Some("MIT".to_string()),
            capabilities: vec![PluginCapability::Tools, PluginCapability::Commands],
            entry_point: "create_plugin".to_string(),
            min_m31a_version: "0.1.0".to_string(),
            config_schema: None,
            sha256: None,
            signature: None,
        }
    }

    fn initialize(&mut self, _ctx: ExtensionContext) -> anyhow::Result<()> {
        self.initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.initialized = false;
        Ok(())
    }

    fn tools(&self) -> Vec<PluginToolDef> {
        vec![PluginToolDef {
            name: "calc_sum".to_string(),
            description: "Calculates sum of two numbers".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "a": { "type": "number" },
                    "b": { "type": "number" }
                },
                "required": ["a", "b"]
            }),
            output_schema: None,
            requires_approval: false,
            sandbox_policy: Some("ReadOnly".to_string()),
        }]
    }

    fn execute_tool(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        if name == "calc_sum" {
            let a = args["a"].as_f64().unwrap_or(0.0);
            let b = args["b"].as_f64().unwrap_or(0.0);
            Ok(json!({ "sum": a + b }))
        } else {
            anyhow::bail!("Unknown tool: {}", name);
        }
    }

    fn commands(&self) -> Vec<CommandDef> {
        vec![CommandDef {
            name: "echo_ping".to_string(),
            description: "Replies with pong".to_string(),
            usage: "/echo_ping [text]".to_string(),
            aliases: vec!["ping".to_string()],
        }]
    }

    fn execute_command(&self, name: &str, args: Vec<String>) -> anyhow::Result<String> {
        if name == "echo_ping" || name == "ping" {
            Ok(format!("pong: {}", args.join(" ")))
        } else {
            anyhow::bail!("Unknown command: {}", name);
        }
    }

    fn check_approval(&self, _hook: &str, tool_call: &ToolCall) -> anyhow::Result<bool> {
        // Reject destructive rm command
        if tool_call.function.name == "exec_command"
            && tool_call.function.arguments.contains("rm -rf")
        {
            Ok(false)
        } else {
            Ok(true)
        }
    }
}

#[test]
fn test_verify_plugin_integrity_sha256() {
    let dummy_binary = b"ELF\x02\x01\x01\x00 M31A Test Binary Buffer for Integrity Check";
    // echo -n "ELF\x02\x01\x01\x00 M31A Test Binary Buffer for Integrity Check" | sha256sum
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(dummy_binary);
    let valid_sha256 = format!("{:x}", hasher.finalize());

    // Valid hash passes
    assert!(verify_plugin_integrity(dummy_binary, &valid_sha256).is_ok());

    // Invalid hash fails with diagnostic message
    let bad_sha256 = "0000000000000000000000000000000000000000000000000000000000000000";
    let res = verify_plugin_integrity(dummy_binary, bad_sha256);
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Plugin integrity verification failed"));
}

#[test]
fn test_plugin_manifest_toml_roundtrip() {
    let toml_content = r#"
name = "docker-runner"
version = "1.2.3"
description = "Containerized isolated test execution"
author = "DevOps Specialist"
capabilities = ["tools", "sandboxbackends"]
entry_point = "create_docker_plugin"
min_m31a_version = "0.1.0"
sha256 = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90"
"#;

    let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
    assert_eq!(manifest.name, "docker-runner");
    assert_eq!(manifest.version, "1.2.3");
    assert_eq!(manifest.capabilities.len(), 2);
    assert!(manifest.capabilities.contains(&PluginCapability::Tools));
    assert!(
        manifest
            .capabilities
            .contains(&PluginCapability::SandboxBackends)
    );
    assert_eq!(manifest.min_m31a_version, "0.1.0");

    let serialized = toml::to_string(&manifest).unwrap();
    assert!(serialized.contains("docker-runner"));
    assert!(serialized.contains("create_docker_plugin"));
}

#[test]
fn test_plugin_marketplace_search_and_retrieval() {
    let client = PluginMarketplaceClient::new();

    // Direct get by ID
    let supabase = client.find_by_id("supabase-m31a");
    assert!(supabase.is_some());
    let p = supabase.unwrap();
    assert_eq!(p.name, "Supabase Database & Auth Connector");
    assert!(p.verified_publisher);
    assert!(p.downloads >= 1000);

    // Search query
    let results = client.search("vercel");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "vercel-deployer");

    // Case-insensitive search on description
    let vector_results = client.search("vector");
    assert_eq!(vector_results.len(), 1);
    assert_eq!(vector_results[0].id, "supabase-m31a");

    // Non-matching query
    let empty_results = client.search("non_existent_search_term_xyz");
    assert!(empty_results.is_empty());
}

#[test]
fn test_in_process_plugin_execution_pipeline() {
    let mut plugin = TestEmbeddedPlugin { initialized: false };

    let ctx = ExtensionContext {
        config: PluginConfig::default(),
        workspace_root: PathBuf::from("/tmp"),
        m31a_version: "0.1.13".to_string(),
        api_version: 1,
    };

    plugin.initialize(ctx).unwrap();
    assert!(plugin.initialized);

    // Tool execution
    let tools = plugin.tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "calc_sum");

    let sum_res = plugin
        .execute_tool("calc_sum", serde_json::json!({ "a": 15.5, "b": 4.5 }))
        .unwrap();
    assert_eq!(sum_res["sum"], 20.0);

    // Command execution
    let cmd_res = plugin
        .execute_command("echo_ping", vec!["hello".to_string(), "world".to_string()])
        .unwrap();
    assert_eq!(cmd_res, "pong: hello world");

    // Approval gate: safe command approved, dangerous command rejected
    let safe_call = ToolCall {
        id: "call-1".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "exec_command".to_string(),
            arguments: "cargo test".to_string(),
        },
    };
    assert!(plugin.check_approval("gate-1", &safe_call).unwrap());

    let dangerous_call = ToolCall {
        id: "call-2".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "exec_command".to_string(),
            arguments: "rm -rf /".to_string(),
        },
    };
    assert!(!plugin.check_approval("gate-1", &dangerous_call).unwrap());

    plugin.shutdown().unwrap();
    assert!(!plugin.initialized);
}

#[test]
fn test_plugin_loader_missing_library_error() {
    let tmp = tempdir().unwrap();
    let mut loader = PluginLoader::new();

    // Directory without dynamic library
    let res = loader.load_plugin(tmp.path());
    assert!(
        res.is_err(),
        "Loading from directory without plugin shared library must fail"
    );
    let err_str = res.unwrap_err().to_string();
    assert!(err_str.contains("Plugin library not found"));
}
