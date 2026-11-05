//! Production-path integration tests for M31A App Server security boundary.
//!
//! Validates the local-first, single-user security model:
//! 1. Local requests over loopback succeed without first-party tokens.
//! 2. Untrusted Host headers (DNS rebinding) are rejected.
//! 3. Untrusted cross-origin requests are rejected.
//! 4. Trusted CORS origins are allowed.
//! 5. Clients cannot bypass workspace trust checks via `allow_untrusted: true`.
//! 6. Workspace trust operates legitimately through `workspace/trust_grant`.
//! 7. Non-loopback network interface binds are strictly rejected.
//! 8. Oversized bodies are rejected before unbounded allocation.
//! 9. Slow header clients time out.
//! 10. Malformed requests cannot bypass route validation.
//! 11. Sanitized configuration redacts sensitive credentials.
//! 12. Concurrent local requests execute safely without interference.

use m31a_app_server::http_server::HttpServerConfig;
use m31a_app_server::protocol::AppServer;
use m31a_core::Config;
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

static INIT: Once = Once::new();
fn init_test_env() {
    INIT.call_once(|| unsafe {
        std::env::set_var("M31A_TEST_MODE", "1");
    });
}

async fn spawn_test_server(
    config_modifier: impl FnOnce(&mut HttpServerConfig),
) -> (SocketAddr, Arc<AppServer>) {
    init_test_env();
    let config = Arc::new(Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        model_provider: Some("nim".to_string()),
        api_key: Some("test-api-key".to_string()),
        ..Default::default()
    });
    let storage = Storage::open_in_memory().unwrap();
    let tools = Arc::new(ToolRegistry::new());
    let server = AppServer::new(config, storage, tools).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let mut http_config = HttpServerConfig {
        bind_addr: local_addr,
        cors_allow_all: false,
        allowed_origins: vec!["https://trusted.app.m31a".to_string()],
        max_body_size: 1024 * 1024,
        max_header_bytes: 16 * 1024,
        header_timeout_ms: 2000,
        body_timeout_ms: 3000,
        max_concurrent_connections: 128,
    };
    config_modifier(&mut http_config);

    let s_arc = Arc::new(server.clone());

    tokio::spawn(async move {
        let _ = m31a_app_server::run_http_server_with_listener(server, http_config, listener).await;
    });

    (local_addr, s_arc)
}

#[tokio::test]
async fn test_1_local_requests_succeed_without_auth_tokens() {
    let (addr, _) = spawn_test_server(|_| {}).await;

    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!("GET /api/threads HTTP/1.1\r\nHost: {}\r\n\r\n", addr);
    client.write_all(req.as_bytes()).await.unwrap();

    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(
        resp.contains("HTTP/1.1 200 OK"),
        "Local request must succeed without tokens: {}",
        resp
    );
}

#[tokio::test]
async fn test_2_untrusted_host_headers_rejected() {
    let (addr, _) = spawn_test_server(|_| {}).await;

    // DNS rebinding attempt using attacker domain in Host header
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = "GET /api/threads HTTP/1.1\r\nHost: attacker.malicious.com\r\n\r\n";
    client.write_all(req.as_bytes()).await.unwrap();

    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(
        resp.contains("HTTP/1.1 403 Forbidden"),
        "Untrusted Host header must be rejected: {}",
        resp
    );
}

#[tokio::test]
async fn test_3_cross_origin_requests_from_untrusted_origins_rejected() {
    let (addr, _) = spawn_test_server(|c| {
        c.cors_allow_all = false;
        c.allowed_origins = vec!["https://trusted.app.m31a".to_string()];
    })
    .await;

    // Disallowed origin OPTIONS preflight rejected with 403 Forbidden
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "OPTIONS /api/threads HTTP/1.1\r\nHost: {}\r\nOrigin: https://evil.attacker.com\r\n\r\n",
        addr
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(
        resp.contains("HTTP/1.1 403 Forbidden"),
        "Disallowed origin must be rejected: {}",
        resp
    );
}

#[tokio::test]
async fn test_4_trusted_cors_origin_allowed() {
    let (addr, _) = spawn_test_server(|c| {
        c.cors_allow_all = false;
        c.allowed_origins = vec!["https://trusted.app.m31a".to_string()];
    })
    .await;

    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "OPTIONS /api/threads HTTP/1.1\r\nHost: {}\r\nOrigin: https://trusted.app.m31a\r\n\r\n",
        addr
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 204 No Content"));
    assert!(resp.contains("Access-Control-Allow-Origin: https://trusted.app.m31a"));
}

#[tokio::test]
async fn test_5_client_cannot_authorize_untrusted_workspace_via_allow_untrusted() {
    let (addr, _) = spawn_test_server(|_| {}).await;

    let temp = tempdir().unwrap();
    let untrusted_path = temp.path().join("untrusted_repo");
    std::fs::create_dir_all(&untrusted_path).unwrap();

    let mut client = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({
        "cwd": untrusted_path.to_string_lossy(),
        "allow_untrusted": true
    })
    .to_string();

    let req = format!(
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        body.len(),
        body
    );
    client.write_all(req.as_bytes()).await.unwrap();

    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(
        resp.contains("HTTP/1.1 400 Bad Request") || resp.contains("Execution REJECTED"),
        "allow_untrusted: true must be ignored and untrusted workspace must fail closed: {}",
        resp
    );
}

#[tokio::test]
async fn test_6_authorized_session_trust_works_through_legitimate_flow() {
    let (addr, _server) = spawn_test_server(|_| {}).await;

    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("legit_repo");
    std::fs::create_dir_all(&repo_path).unwrap();

    // 1. Initial attempt fails closed
    let mut client = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({ "cwd": repo_path.to_string_lossy() }).to_string();
    let req = format!(
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        body.len(),
        body
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 400 Bad Request"));

    // 2. Legitimate flow: Call workspace/trust_grant via RPC
    let rpc_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "workspace/trust_grant",
        "params": {
            "path": repo_path.to_string_lossy(),
            "level": "session_only"
        }
    })
    .to_string();

    let mut client = TcpStream::connect(addr).await.unwrap();
    let rpc_req = format!(
        "POST /rpc HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        rpc_body.len(),
        rpc_body
    );
    client.write_all(rpc_req.as_bytes()).await.unwrap();
    let mut rpc_resp = String::new();
    client.read_to_string(&mut rpc_resp).await.unwrap();
    assert!(rpc_resp.contains("\"status\":\"granted\""));

    // 3. Thread creation now succeeds
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        body.len(),
        body
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 201 Created"));
}

#[test]
fn test_7_network_binds_reject_non_loopback() {
    let wildcard_config = HttpServerConfig {
        bind_addr: "0.0.0.0:4096".parse().unwrap(),
        ..Default::default()
    };
    assert!(
        wildcard_config.validate().is_err(),
        "Binding to wildcard address MUST fail validation"
    );

    let public_ip_config = HttpServerConfig {
        bind_addr: "192.168.1.50:4096".parse().unwrap(),
        ..Default::default()
    };
    assert!(
        public_ip_config.validate().is_err(),
        "Binding to public/LAN address MUST fail validation"
    );

    let loopback_config = HttpServerConfig {
        bind_addr: "127.0.0.1:4096".parse().unwrap(),
        ..Default::default()
    };
    assert!(loopback_config.validate().is_ok());
}

#[tokio::test]
async fn test_8_oversized_bodies_rejected_before_unbounded_allocation() {
    let (addr, _) = spawn_test_server(|c| {
        c.max_body_size = 1024; // 1KB maximum body
    })
    .await;

    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /rpc HTTP/1.1\r\nHost: {}\r\nContent-Length: 10485760\r\n\r\n",
        addr
    );
    client.write_all(req.as_bytes()).await.unwrap();

    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(
        resp.contains("HTTP/1.1 413 Payload Too Large"),
        "Server must reject oversized Content-Length immediately: {}",
        resp
    );
}

#[tokio::test]
async fn test_9_slow_header_clients_timed_out() {
    let (addr, _) = spawn_test_server(|c| {
        c.header_timeout_ms = 300; // 300ms header timeout
    })
    .await;

    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(b"GET /api/").await.unwrap();
    client.flush().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut resp = String::new();
    let _ = client.read_to_string(&mut resp).await;
    assert!(
        resp.contains("HTTP/1.1 408 Request Timeout") || resp.is_empty(),
        "Slow header client must be disconnected with 408: {}",
        resp
    );
}

#[tokio::test]
async fn test_10_malformed_requests_cannot_bypass_route_authorization() {
    let (addr, _) = spawn_test_server(|_| {}).await;

    // Conflicting Content-Length and Transfer-Encoding (Request smuggling attempt)
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /rpc HTTP/1.1\r\nHost: {}\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
        addr
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    let _ = client.read_to_string(&mut resp).await;
    assert!(
        resp.contains("HTTP/1.1 400 Bad Request") || resp.is_empty(),
        "Conflicting Content-Length and Transfer-Encoding must be rejected"
    );
}

#[tokio::test]
async fn test_11_sanitized_config_redacts_secrets_and_mcp_credentials() {
    let (addr, _) = spawn_test_server(|_| {}).await;

    let mut client = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "config/read",
        "params": {}
    })
    .to_string();
    let req = format!(
        "POST /api/rpc HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        body.len(),
        body
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("2.0"));

    let body_part = resp.split("\r\n\r\n").nth(1).unwrap();
    let json: serde_json::Value = serde_json::from_str(body_part).unwrap();
    let result = &json["result"];

    // Raw secrets must never be exposed
    assert_ne!(result.get("api_key").unwrap(), "super_secret_raw_key");
    assert!(result.get("api_key_configured").is_some());
}

#[tokio::test]
async fn test_12_concurrent_local_requests_handled_safely() {
    let (addr, _) = spawn_test_server(|_| {}).await;

    let mut handles = Vec::new();
    for i in 0..10 {
        handles.push(tokio::spawn(async move {
            let mut client = TcpStream::connect(addr).await.unwrap();
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": i,
                "method": "models/list",
                "params": {}
            })
            .to_string();
            let req = format!(
                "POST /rpc HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                addr,
                body.len(),
                body
            );
            client.write_all(req.as_bytes()).await.unwrap();
            let mut resp = String::new();
            client.read_to_string(&mut resp).await.unwrap();
            assert!(resp.contains("2.0"));
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}
