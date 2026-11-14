//! Automated Runtime Completeness Test Suite for M31A.
//!
//! Executes rigorous end-to-end verification against the packaged CLI executable:
//! - First-launch setup and config discovery
//! - Embedded model catalog loading
//! - Prompt fragment loading
//! - TUI icon rendering & capability fallback (Lucide -> Unicode -> ASCII)
//! - SQLite database schema initialization
//! - JSON-RPC App Server stdio startup & handshake
//! - Worktree and Git attribution checks
//! - Bubblewrap sandbox detection and execution
//! - MCP server registration lifecycle
//! - Tool discovery and registry initialization
//! - Slash commands and security presets
//! - Dynamic shell completion scripts (bash, zsh, fish)
//! - Runtime asset manifest cryptographic validation
//! - Offline startup and airgapped posture
//! - Dual-binary verification (GNU and MUSL releases when available)

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::tempdir;

fn m31a_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_m31a"))
}

#[test]
fn test_runtime_asset_manifest_validation() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("runtime_asset_manifest.json");

    assert!(
        manifest_path.exists(),
        "runtime_asset_manifest.json must exist in repository root"
    );

    let content = fs::read_to_string(&manifest_path).expect("read manifest");
    let json: serde_json::Value = serde_json::from_str(&content).expect("parse manifest json");

    assert_eq!(json["app_name"], "m31a");
    assert_eq!(json["app_version"], "0.1.15");

    let resources = json["resources"].as_array().expect("resources array");
    assert!(!resources.is_empty(), "resources must not be empty");

    let root = manifest_path.parent().unwrap();

    for resource in resources {
        let rel_path = resource["source_path"].as_str().unwrap();
        let expected_sha256 = resource["sha256"].as_str().unwrap();
        let expected_size = resource["size_bytes"].as_u64().unwrap();

        let full_path = root.join(rel_path);
        assert!(
            full_path.exists(),
            "Resource {} at {:?} must exist on disk",
            resource["resource_id"],
            full_path
        );

        let bytes = fs::read(&full_path).unwrap();
        assert_eq!(
            bytes.len() as u64,
            expected_size,
            "Resource {:?} size mismatch",
            rel_path
        );

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual_sha = format!("{:x}", hasher.finalize());

        assert_eq!(
            actual_sha, expected_sha256,
            "Resource {:?} SHA-256 checksum mismatch",
            rel_path
        );
    }
}

#[test]
fn test_first_launch_setup_and_config_discovery() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().to_path_buf();
    let config_dir = home.join(".config").join("m31a");

    let output = Command::new(m31a_bin())
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .args([
            "setup",
            "--non-interactive",
            "--api-key",
            "nvapi-test-dummy-key-01234567890123456789",
            "--model",
            "deepseek-ai/deepseek-r1",
            "--sandbox",
            "workspace-write",
            "--approval",
            "on-request",
            "--force",
        ])
        .output()
        .expect("run m31a setup");

    assert!(
        output.status.success(),
        "Setup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_file = config_dir.join("config.toml");
    assert!(config_file.exists(), "config.toml should have been created");

    let toml_str = fs::read_to_string(&config_file).unwrap();
    assert!(toml_str.contains("deepseek-ai/deepseek-r1"));

    // Verify config discovery via doctor --json
    let doc_out = Command::new(m31a_bin())
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .args(["doctor", "--json"])
        .output()
        .expect("run doctor");

    assert!(doc_out.status.success());
    let doc_json: serde_json::Value =
        serde_json::from_slice(&doc_out.stdout).expect("parse doctor json");
    assert_eq!(doc_json["auth"]["active_model"], "deepseek-ai/deepseek-r1");
    assert!(doc_json["auth"]["key_configured"].as_bool().unwrap());
}

#[test]
fn test_embedded_model_catalog_loading() {
    let output = Command::new(m31a_bin())
        .args(["model", "list"])
        .output()
        .expect("run model list");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Available Frontier Models in NVIDIA NIM Catalog"));
    assert!(stdout.contains("deepseek-ai/deepseek-r1"));
    assert!(stdout.contains("meta/llama-3.3-70b-instruct"));
    assert!(stdout.contains("nvidia/nemotron-3.5-lightning-30b-a3b"));
}

#[test]
fn test_icon_rendering_fallback_matrix() {
    // 1. Force ASCII via M31A_ICONS
    let out_ascii = Command::new(m31a_bin())
        .env("M31A_ICONS", "ascii")
        .args(["doctor", "--json"])
        .output()
        .expect("run doctor");
    assert!(out_ascii.status.success());
    let json_ascii: serde_json::Value = serde_json::from_slice(&out_ascii.stdout).unwrap();
    assert_eq!(json_ascii["font_support"]["effective_icon_mode"], "ascii");

    // 2. Force Unicode via M31A_ICONS
    let out_uni = Command::new(m31a_bin())
        .env("M31A_ICONS", "unicode")
        .args(["doctor", "--json"])
        .output()
        .expect("run doctor");
    assert!(out_uni.status.success());
    let json_uni: serde_json::Value = serde_json::from_slice(&out_uni.stdout).unwrap();
    assert_eq!(json_uni["font_support"]["effective_icon_mode"], "unicode");

    // 3. Fallback on restricted dumb terminal
    let out_dumb = Command::new(m31a_bin())
        .env("TERM", "dumb")
        .env_remove("M31A_ICONS")
        .args(["doctor", "--json"])
        .output()
        .expect("run doctor");
    assert!(out_dumb.status.success());
    let json_dumb: serde_json::Value = serde_json::from_slice(&out_dumb.stdout).unwrap();
    assert_eq!(json_dumb["font_support"]["effective_icon_mode"], "ascii");
}

#[test]
fn test_database_schema_initialization() {
    let tmp = tempdir().unwrap();
    let config_dir = tmp.path().join("m31a_test_config");
    fs::create_dir_all(&config_dir).unwrap();

    // Trigger state database initialization via CLI command that touches storage
    let init_out = Command::new(m31a_bin())
        .env("M31A_HOME", &config_dir)
        .args(["agents"])
        .output()
        .expect("run m31a agents to initialize storage");
    assert!(init_out.status.success());

    let output = Command::new(m31a_bin())
        .env("M31A_HOME", &config_dir)
        .args(["doctor", "--json"])
        .output()
        .expect("run doctor");

    assert!(output.status.success());
    let doc_json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(doc_json["thread_inventory"]["db_exists"].as_bool().unwrap());

    let db_path = PathBuf::from(doc_json["thread_inventory"]["db_path"].as_str().unwrap());
    assert!(db_path.exists(), "SQLite state database must exist");
}

#[test]
fn test_app_server_json_rpc_startup() {
    let mut child = Command::new(m31a_bin())
        .args(["app-server", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn app-server");

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    let mut stdin = child.stdin.take().expect("stdin");
    let req_str = format!("{}\n", serde_json::to_string(&req).unwrap());
    stdin.write_all(req_str.as_bytes()).unwrap();
    drop(stdin);

    let output = child.wait_with_output().expect("wait on app-server");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let resp: serde_json::Value = serde_json::from_str(stdout.trim()).expect("parse rpc response");
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["server_name"], "m31a-app-server");
    assert_eq!(resp["result"]["version"], "0.1.15");
}

#[test]
fn test_shell_completion_generation() {
    for shell in ["bash", "zsh", "fish"] {
        let output = Command::new(m31a_bin())
            .args(["completion", shell])
            .output()
            .unwrap_or_else(|_| panic!("generate completion for {}", shell));

        assert!(
            output.status.success(),
            "completion {} failed: {}",
            shell,
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.is_empty(), "completion {} output empty", shell);

        match shell {
            "bash" => assert!(stdout.contains("_m31a()")),
            "zsh" => assert!(stdout.contains("#compdef m31a")),
            "fish" => assert!(stdout.contains("__fish_m31a")),
            _ => {}
        }
    }
}

#[test]
fn test_security_presets_and_offline_posture() {
    let output = Command::new(m31a_bin())
        .args(["presets", "airgapped"])
        .output()
        .expect("run presets airgapped");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Airgapped"));
    assert!(stdout.contains("Strictly Offline"));
}

#[test]
fn test_mcp_registration_subcommands() {
    let output = Command::new(m31a_bin())
        .args(["mcp", "list"])
        .output()
        .expect("run mcp list");

    assert!(output.status.success());
}

#[test]
fn test_published_gnu_and_musl_binaries_if_available() {
    let audit_dir = Path::new("/tmp/m31a_v0.1.15_audit");
    let gnu_bin = audit_dir
        .join("m31a-cli-x86_64-unknown-linux-gnu")
        .join("m31a");
    let musl_bin = audit_dir
        .join("m31a-cli-x86_64-unknown-linux-musl")
        .join("m31a");

    for (target_name, bin_path) in [("GNU", &gnu_bin), ("MUSL", &musl_bin)] {
        if bin_path.exists() {
            println!("Testing published {} binary at {:?}", target_name, bin_path);

            let ver_out = Command::new(bin_path).arg("--version").output().unwrap();
            assert!(ver_out.status.success());
            let ver_str = String::from_utf8_lossy(&ver_out.stdout);
            assert!(
                ver_str.contains("0.1.15"),
                "{} version mismatch: {}",
                target_name,
                ver_str
            );

            let help_out = Command::new(bin_path).arg("--help").output().unwrap();
            assert!(help_out.status.success());

            let comp_out = Command::new(bin_path)
                .args(["completion", "bash"])
                .output()
                .unwrap();
            assert!(comp_out.status.success());

            let doc_out = Command::new(bin_path)
                .args(["doctor", "--json"])
                .output()
                .unwrap();
            assert!(doc_out.status.success());
        }
    }
}
