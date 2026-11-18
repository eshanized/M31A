//! WebAssembly Sandboxing and Adversarial Module Test Suite.

use m31a_wasm::*;
use tempfile::tempdir;

#[test]
fn test_valid_wasm_tool_invocation() {
    let runtime = WasmRuntime::new();
    let wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "m31a_tool_call") (param $np i32) (param $nl i32) (param $ap i32) (param $al i32) (result i32)
    ;; Copy "hello world" to 4096
    (i32.store8 (i32.const 4096) (i32.const 104)) ;; 'h'
    (i32.store8 (i32.const 4097) (i32.const 101)) ;; 'e'
    (i32.store8 (i32.const 4098) (i32.const 108)) ;; 'l'
    (i32.store8 (i32.const 4099) (i32.const 108)) ;; 'l'
    (i32.store8 (i32.const 4100) (i32.const 111)) ;; 'o'
    (i32.const 5)
  )
)
"#;
    let wasm_bytes = wat::parse_str(wat).unwrap();
    let manifest = WasmPluginManifest {
        name: "hello-plugin".to_string(),
        version: "1.0.0".to_string(),
        abi_version: 1,
        description: "Valid test plugin".to_string(),
        author: None,
        capabilities: vec![WasmCapability::ToolProvider],
        limits: WasmResourceLimits::default(),
    };

    let mut instance = runtime
        .load_plugin(manifest, &wasm_bytes, std::env::temp_dir(), "{}")
        .unwrap();

    let res = instance.call_tool("greet", "{}").unwrap();
    assert_eq!(res, "hello");
}

#[test]
fn test_adversarial_infinite_loop_fuel_exhaustion() {
    let runtime = WasmRuntime::new();
    // Infinite loop in WebAssembly
    let wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "m31a_tool_call") (param i32 i32 i32 i32) (result i32)
    (loop $infinite
      (br $infinite)
    )
    (i32.const 0)
  )
)
"#;
    let wasm_bytes = wat::parse_str(wat).unwrap();
    let manifest = WasmPluginManifest {
        name: "malicious-infinite-loop".to_string(),
        version: "1.0.0".to_string(),
        abi_version: 1,
        description: "Infinite loop attacker".to_string(),
        author: None,
        capabilities: Vec::new(),
        limits: WasmResourceLimits {
            max_memory_pages: 10,
            max_fuel: 50_000, // Small fuel budget
            execution_deadline_ms: 1000,
        },
    };

    let mut instance = runtime
        .load_plugin(manifest, &wasm_bytes, std::env::temp_dir(), "{}")
        .unwrap();

    let err = instance.call_tool("loop", "{}").unwrap_err();
    match err {
        WasmError::FuelExhausted { consumed, limit } => {
            assert_eq!(limit, 50_000);
            assert!(consumed >= limit);
        }
        other => panic!("Expected FuelExhausted, got {:?}", other),
    }
}

#[test]
fn test_adversarial_guest_trap_handling() {
    let runtime = WasmRuntime::new();
    let wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "m31a_tool_call") (param i32 i32 i32 i32) (result i32)
    (unreachable)
  )
)
"#;
    let wasm_bytes = wat::parse_str(wat).unwrap();
    let manifest = WasmPluginManifest {
        name: "trap-plugin".to_string(),
        version: "1.0.0".to_string(),
        abi_version: 1,
        description: "Trap generator".to_string(),
        author: None,
        capabilities: Vec::new(),
        limits: WasmResourceLimits::default(),
    };

    let mut instance = runtime
        .load_plugin(manifest, &wasm_bytes, std::env::temp_dir(), "{}")
        .unwrap();

    let err = instance.call_tool("trap", "{}").unwrap_err();
    assert!(matches!(err, WasmError::GuestTrap(_)));
}

#[test]
fn test_adversarial_memory_page_limit_enforcement() {
    let runtime = WasmRuntime::new();
    // Requests 100 initial memory pages
    let wat = r#"
(module
  (memory (export "memory") 100)
  (func (export "m31a_tool_call") (param i32 i32 i32 i32) (result i32)
    (i32.const 0)
  )
)
"#;
    let wasm_bytes = wat::parse_str(wat).unwrap();
    let manifest = WasmPluginManifest {
        name: "memory-hog".to_string(),
        version: "1.0.0".to_string(),
        abi_version: 1,
        description: "Memory hog".to_string(),
        author: None,
        capabilities: Vec::new(),
        limits: WasmResourceLimits {
            max_memory_pages: 16, // Maximum allowed is 16 pages
            max_fuel: 100_000,
            execution_deadline_ms: 1000,
        },
    };

    let res = runtime.load_plugin(manifest, &wasm_bytes, std::env::temp_dir(), "{}");
    assert!(res.is_err());
    if let Err(e) = res {
        match e {
            WasmError::MemoryLimitExceeded { pages, max_pages } => {
                assert_eq!(pages, 100);
                assert_eq!(max_pages, 16);
            }
            other => panic!("Expected MemoryLimitExceeded, got {:?}", other),
        }
    } else {
        panic!("Expected error but succeeded");
    }
}

#[test]
fn test_unauthorized_filesystem_access_blocked() {
    let dir = tempdir().unwrap();
    let secret_file = dir.path().join("secret.key");
    std::fs::write(&secret_file, "super-secret-key").unwrap();

    let allowed_file = dir.path().join("allowed.txt");
    std::fs::write(&allowed_file, "public-data").unwrap();

    // Module imports host_fs_read
    let runtime = WasmRuntime::new();
    let wat = r#"
(module
  (import "m31a_host" "host_fs_read" (func $host_fs_read (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "m31a_tool_call") (param $np i32) (param $nl i32) (param $ap i32) (param $al i32) (result i32)
    ;; Attempt to read $ap
    (call $host_fs_read (local.get $ap) (local.get $al) (i32.const 4096) (i32.const 1024))
  )
)
"#;
    let wasm_bytes = wat::parse_str(wat).unwrap();

    // Manifest grants permission only to "allowed.txt"
    let manifest = WasmPluginManifest {
        name: "fs-test".to_string(),
        version: "1.0.0".to_string(),
        abi_version: 1,
        description: "FS capability test".to_string(),
        author: None,
        capabilities: vec![WasmCapability::RestrictedFsRead {
            allowed_paths: vec!["allowed.txt".to_string()],
        }],
        limits: WasmResourceLimits::default(),
    };

    let mut instance = runtime
        .load_plugin(manifest, &wasm_bytes, dir.path(), "{}")
        .unwrap();

    // 1. Allowed path succeeds
    let allowed_res = instance.call_tool("read", "allowed.txt").unwrap();
    assert_eq!(allowed_res, "public-data");

    // 2. Secret file without capability returns error code -3 (UnauthorizedCapability)
    let secret_res = instance.call_tool("read", "secret.key");
    assert!(secret_res.is_err());

    // 3. Traversal attempt returns error code -3
    let traversal_res = instance.call_tool("read", "../secret.key");
    assert!(traversal_res.is_err());
}

#[test]
fn test_malformed_manifest_rejected() {
    let malformed = "this is not valid toml = [[[";
    assert!(WasmPluginManifest::parse_toml(malformed).is_err());

    let empty_name = r#"
name = ""
version = "1.0.0"
description = "Empty name"
"#;
    assert!(WasmPluginManifest::parse_toml(empty_name).is_err());
}
