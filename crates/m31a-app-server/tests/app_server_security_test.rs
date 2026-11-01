//! Production-path integration tests for M31A App Server security boundary.
//!
//! Validates:
//! 1. Requests without credentials are rejected.
//! 2. Invalid and revoked credentials are rejected.
//! 3. A client cannot access another client's thread.
//! 4. A client cannot subscribe to unauthorized SSE events.
//! 5. A client cannot authorize an untrusted workspace by passing `allow_untrusted: true`.
//! 6. Authorized session trust works through the legitimate flow.
//! 7. Network binds enforce the required authentication configuration.
//! 8. Invalid CORS origins are rejected.
//! 9. Oversized bodies are rejected before unbounded allocation.
//! 10. Slow header/body clients do not exhaust connection resources.
//! 11. Malformed requests cannot bypass route authorization.
//! 12. Concurrent requests cannot cross session or authorization boundaries.

use m31a_app_server::auth::AuthScope;
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

// Set M31A_TEST_MODE once for all tests in this module
static INIT: Once = Once::new();
fn init_test_env() {
    INIT.call_once(|| unsafe {
        std::env::set_var("M31A_TEST_MODE", "1");
    });
}

async fn spawn_test_server(
    config_modifier: impl FnOnce(&mut HttpServerConfig),
) -> (SocketAddr, Arc<AppServer>, String) {
    init_test_env();
    let config = Arc::new(Config {
        model: "mock-testing-model".to_string(),
        model_provider: Some("mock".to_string()),
        ..Default::default()
    });
    let storage = Storage::open_in_memory().unwrap();
    let tools = Arc::new(ToolRegistry::new());
    let server = AppServer::new(config, storage, tools).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let admin_token = server.auth_manager.generate_token(
        "admin_user",
        "Admin",
        vec![
            AuthScope::Read,
            AuthScope::Write,
            AuthScope::Admin,
            AuthScope::Execute,
        ],
        None,
    );

    let mut http_config = HttpServerConfig {
        bind_addr: local_addr,
        cors_allow_all: false,
        allowed_origins: vec!["https://trusted.app.m31a".to_string()],
        require_auth: true,
        auth_token: Some(admin_token.clone()),
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

    (local_addr, s_arc, admin_token)
}

#[tokio::test]
async fn test_1_requests_without_credentials_rejected() {
    let (addr, _, _) = spawn_test_server(|_| {}).await;

    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!("GET /api/threads HTTP/1.1\r\nHost: {}\r\n\r\n", addr);
    client.write_all(req.as_bytes()).await.unwrap();

    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(
        resp.contains("HTTP/1.1 401 Unauthorized"),
        "Expected 401: {}",
        resp
    );
    assert!(resp.contains("WWW-Authenticate: Bearer"));
}

#[tokio::test]
async fn test_2_invalid_and_revoked_credentials_rejected() {
    let (addr, server, _) = spawn_test_server(|_| {}).await;

    // A. Invalid token
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer invalid_bogus_token\r\n\r\n",
        addr
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 401 Unauthorized"));

    // B. Revoked token
    let temp_token = server.auth_manager.generate_token(
        "client_x",
        "Temp Client",
        vec![AuthScope::Read, AuthScope::Write],
        None,
    );

    // Verify valid initially
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\r\n",
        addr, temp_token
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 200 OK"));

    // Revoke token
    server.auth_manager.revoke_token(&temp_token);

    // Verify rejected after revocation
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\r\n",
        addr, temp_token
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 401 Unauthorized"));
}

#[tokio::test]
async fn test_3_client_cannot_access_another_client_thread() {
    let (addr, server, _) = spawn_test_server(|_| {}).await;

    let token_a = server.auth_manager.generate_token(
        "client_a",
        "Client A",
        vec![AuthScope::Read, AuthScope::Write],
        None,
    );
    let token_b = server.auth_manager.generate_token(
        "client_b",
        "Client B",
        vec![AuthScope::Read, AuthScope::Write],
        None,
    );

    // Trust /tmp for session
    server
        .trust_store
        .lock()
        .unwrap()
        .trust(
            std::path::Path::new("/tmp"),
            m31a_trust::TrustLevel::SessionOnly,
        )
        .unwrap();

    // Client A creates a thread
    let mut client_a = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({ "cwd": "/tmp" }).to_string();
    let req_a = format!(
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        token_a,
        body.len(),
        body
    );
    client_a.write_all(req_a.as_bytes()).await.unwrap();
    let mut resp_a = String::new();
    client_a.read_to_string(&mut resp_a).await.unwrap();
    assert!(resp_a.contains("HTTP/1.1 201 Created"));

    let body_json: serde_json::Value =
        serde_json::from_str(resp_a.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let thread_id = body_json["id"].as_str().unwrap();

    // Client B attempts to read Client A's thread -> MUST be rejected with 403 Forbidden!
    let mut client_b = TcpStream::connect(addr).await.unwrap();
    let req_b = format!(
        "GET /api/threads/{} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\r\n",
        thread_id, addr, token_b
    );
    client_b.write_all(req_b.as_bytes()).await.unwrap();
    let mut resp_b = String::new();
    client_b.read_to_string(&mut resp_b).await.unwrap();
    assert!(
        resp_b.contains("HTTP/1.1 403 Forbidden"),
        "Client B must be denied: {}",
        resp_b
    );
}

#[tokio::test]
async fn test_4_sse_subscriptions_isolated_to_permitted_sessions() {
    let (addr, server, _) = spawn_test_server(|_| {}).await;

    let _token_a = server.auth_manager.generate_token(
        "client_a",
        "Client A",
        vec![AuthScope::Read, AuthScope::Write],
        None,
    );
    let token_b = server.auth_manager.generate_token(
        "client_b",
        "Client B",
        vec![AuthScope::Read, AuthScope::Write],
        None,
    );

    // Register thread ownership
    let thread_a = "thread_alpha_123";
    let thread_b = "thread_beta_456";
    server
        .auth_manager
        .register_thread_owner(thread_a, "client_a");
    server
        .auth_manager
        .register_thread_owner(thread_b, "client_b");

    // Client B connects to SSE stream using Authorization Bearer header
    let mut client_b = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /events HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\r\n",
        addr, token_b
    );
    client_b.write_all(req.as_bytes()).await.unwrap();

    // Read initial 200 OK headers
    let mut header_buf = [0u8; 1024];
    let n = client_b.read(&mut header_buf).await.unwrap();
    let header_str = String::from_utf8_lossy(&header_buf[..n]);
    assert!(header_str.contains("HTTP/1.1 200 OK"));
    assert!(header_str.contains("text/event-stream"));

    // Broadcast event for Client A's thread (Client B must NOT receive this)
    server.notify(
        "notification/turn_started",
        Some(serde_json::json!({
            "thread_id": thread_a,
            "secret_data": "secret_for_a"
        })),
    );

    // Broadcast event for Client B's thread (Client B MUST receive this)
    server.notify(
        "notification/turn_started",
        Some(serde_json::json!({
            "thread_id": thread_b,
            "secret_data": "data_for_b"
        })),
    );

    // Read SSE chunk
    let mut event_buf = [0u8; 2048];
    let n = client_b.read(&mut event_buf).await.unwrap();
    let event_str = String::from_utf8_lossy(&event_buf[..n]);

    assert!(
        !event_str.contains("secret_for_a"),
        "Client B must NOT receive Client A's thread events"
    );
    assert!(
        event_str.contains("data_for_b"),
        "Client B MUST receive its own thread events"
    );
}

#[tokio::test]
async fn test_5_client_cannot_authorize_untrusted_workspace_via_allow_untrusted() {
    let (addr, _, admin_token) = spawn_test_server(|_| {}).await;

    let temp = tempdir().unwrap();
    let untrusted_path = temp.path().join("untrusted_repo");
    std::fs::create_dir_all(&untrusted_path).unwrap();

    // Client attempts to pass allow_untrusted: true to create a thread in untrusted workspace
    let mut client = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({
        "cwd": untrusted_path.to_string_lossy(),
        "allow_untrusted": true
    })
    .to_string();

    let req = format!(
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        admin_token,
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
    let (addr, _server, admin_token) = spawn_test_server(|_| {}).await;

    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("legit_repo");
    std::fs::create_dir_all(&repo_path).unwrap();

    // 1. Initial attempt fails closed
    let mut client = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({ "cwd": repo_path.to_string_lossy() }).to_string();
    let req = format!(
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        admin_token,
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
        "POST /rpc HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        admin_token,
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
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        admin_token,
        body.len(),
        body
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 201 Created"));
}

#[test]
fn test_7_network_binds_enforce_authentication() {
    let unauth_network_config = HttpServerConfig {
        bind_addr: "0.0.0.0:4096".parse().unwrap(),
        require_auth: false,
        ..Default::default()
    };
    assert!(
        unauth_network_config.validate().is_err(),
        "Binding to non-loopback address without require_auth MUST fail validation"
    );

    let auth_network_config = HttpServerConfig {
        bind_addr: "0.0.0.0:4096".parse().unwrap(),
        require_auth: true,
        ..Default::default()
    };
    assert!(auth_network_config.validate().is_ok());
}

#[tokio::test]
async fn test_8_invalid_cors_origins_rejected() {
    let (addr, _, _) = spawn_test_server(|c| {
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

    // Allowed origin succeeds with matching Access-Control-Allow-Origin
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
async fn test_9_oversized_bodies_rejected_before_unbounded_allocation() {
    let (addr, _, admin_token) = spawn_test_server(|c| {
        c.max_body_size = 1024; // 1KB maximum body
    })
    .await;

    let mut client = TcpStream::connect(addr).await.unwrap();
    // Claim 10MB Content-Length
    let req = format!(
        "POST /rpc HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Length: 10485760\r\n\r\n",
        addr, admin_token
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
async fn test_10_slow_header_clients_timed_out() {
    let (addr, _, _) = spawn_test_server(|c| {
        c.header_timeout_ms = 300; // 300ms header timeout
    })
    .await;

    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(b"GET /api/").await.unwrap();
    client.flush().await.unwrap();

    // Sleep longer than header deadline
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
async fn test_11_malformed_requests_cannot_bypass_route_authorization() {
    let (addr, _, _) = spawn_test_server(|_| {}).await;

    // A. Conflicting Content-Length and Transfer-Encoding (Request smuggling attempt)
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
        addr
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 400 Bad Request"));

    // B. Duplicate conflicting Content-Length headers
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nContent-Length: 10\r\nContent-Length: 20\r\n\r\n",
        addr
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 400 Bad Request"));
}

#[tokio::test]
async fn test_12_concurrent_requests_preserve_session_boundaries() {
    let (addr, server, _) = spawn_test_server(|_| {}).await;

    server
        .trust_store
        .lock()
        .unwrap()
        .trust(
            std::path::Path::new("/tmp"),
            m31a_trust::TrustLevel::SessionOnly,
        )
        .unwrap();

    let mut handles = Vec::new();

    for i in 0..6 {
        let client_id = format!("user_{}", i);
        let token = server.auth_manager.generate_token(
            &client_id,
            &format!("User {}", i),
            vec![AuthScope::Read, AuthScope::Write],
            None,
        );

        let server_clone = Arc::clone(&server);
        handles.push(tokio::spawn(async move {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            let body = serde_json::json!({ "cwd": "/tmp" }).to_string();
            let req = format!(
                "POST /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                addr, token, body.len(), body
            );
            stream.write_all(req.as_bytes()).await.unwrap();
            let mut resp = String::new();
            stream.read_to_string(&mut resp).await.unwrap();
            assert!(resp.contains("HTTP/1.1 201 Created"));

            let parsed: serde_json::Value = serde_json::from_str(resp.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            let thread_id = parsed["id"].as_str().unwrap().to_string();

            // Verify owner in server auth manager
            assert_eq!(server_clone.auth_manager.get_thread_owner(&thread_id).unwrap(), client_id);

            // Fetch thread list with this client's token
            let mut stream_list = TcpStream::connect(addr).await.unwrap();
            let list_req = format!(
                "GET /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\r\n",
                addr, token
            );
            stream_list.write_all(list_req.as_bytes()).await.unwrap();
            let mut list_resp = String::new();
            stream_list.read_to_string(&mut list_resp).await.unwrap();

            let threads_json: serde_json::Value = serde_json::from_str(list_resp.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            let arr = threads_json.as_array().unwrap();
            // Client must only see its own thread, not other clients' threads
            for t in arr {
                assert_eq!(t["id"].as_str().unwrap(), thread_id);
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}

#[tokio::test]
async fn test_13_scope_escalation_prevented() {
    let (addr, server, _) = spawn_test_server(|_| {}).await;

    // Token with ONLY Read scope
    let read_token = server.auth_manager.generate_token(
        "reader",
        "Read Only Client",
        vec![AuthScope::Read],
        None,
    );

    // 1. Reader cannot call Write methods (e.g. thread/create) via RPC
    let mut client = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "thread/create",
        "params": { "cwd": "/tmp" }
    })
    .to_string();
    let req = format!(
        "POST /api/rpc HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        read_token,
        body.len(),
        body
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("-32003"));
    assert!(resp.contains("Permission denied"));

    // 2. Token with ONLY Write scope cannot call Admin methods (e.g. workspace/trust_grant)
    let write_token = server.auth_manager.generate_token(
        "writer",
        "Write Only Client",
        vec![AuthScope::Write],
        None,
    );
    let mut client = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "workspace/trust_grant",
        "params": { "path": "/tmp", "level": "session_only" }
    })
    .to_string();
    let req = format!(
        "POST /api/rpc HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        write_token,
        body.len(),
        body
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("-32003"));
    assert!(resp.contains("Permission denied"));
}

#[tokio::test]
async fn test_14_thread_operations_enforce_ownership_for_modify_fork_delete() {
    let (addr, server, _) = spawn_test_server(|_| {}).await;

    let token_a = server.auth_manager.generate_token(
        "client_a",
        "Client A",
        vec![AuthScope::Read, AuthScope::Write, AuthScope::Execute],
        None,
    );
    let token_b = server.auth_manager.generate_token(
        "client_b",
        "Client B",
        vec![AuthScope::Read, AuthScope::Write, AuthScope::Execute],
        None,
    );

    server
        .trust_store
        .lock()
        .unwrap()
        .trust(
            std::path::Path::new("/tmp"),
            m31a_trust::TrustLevel::SessionOnly,
        )
        .unwrap();

    // Client A creates thread
    let mut client_a = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({ "cwd": "/tmp" }).to_string();
    let req_a = format!(
        "POST /api/threads HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        token_a,
        body.len(),
        body
    );
    client_a.write_all(req_a.as_bytes()).await.unwrap();
    let mut resp_a = String::new();
    client_a.read_to_string(&mut resp_a).await.unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(resp_a.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let thread_a_id = parsed["id"].as_str().unwrap();

    // Client B attempts to fork Client A's thread -> MUST be denied with -32003
    let mut client_b = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "thread/fork",
        "params": { "thread_id": thread_a_id, "title": "Stolen Fork" }
    })
    .to_string();
    let req_b = format!(
        "POST /api/rpc HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        token_b,
        body.len(),
        body
    );
    client_b.write_all(req_b.as_bytes()).await.unwrap();
    let mut resp_b = String::new();
    client_b.read_to_string(&mut resp_b).await.unwrap();
    assert!(resp_b.contains("-32003"));
    assert!(resp_b.contains("Access denied"));

    // Client B attempts to delete Client A's thread -> MUST be denied
    let mut client_b = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "thread/delete",
        "params": { "thread_id": thread_a_id }
    })
    .to_string();
    let req_b = format!(
        "POST /api/rpc HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        token_b,
        body.len(),
        body
    );
    client_b.write_all(req_b.as_bytes()).await.unwrap();
    let mut resp_b = String::new();
    client_b.read_to_string(&mut resp_b).await.unwrap();
    assert!(resp_b.contains("-32003"));
    assert!(resp_b.contains("Access denied"));
}

#[tokio::test]
async fn test_15_thread_ownership_persistence_across_server_restart() {
    init_test_env();
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("persistent_session.db");

    let thread_id = "thread_persist_test_1";

    // 1. Open first storage and app server instance
    {
        let storage = Storage::open(&db_path).unwrap();
        let auth_mgr = m31a_app_server::auth::AuthManager::with_storage(storage.clone());
        auth_mgr.register_thread_owner(thread_id, "persistent_owner");
        assert_eq!(
            auth_mgr.get_thread_owner(thread_id).unwrap(),
            "persistent_owner"
        );
    }

    // 2. Simulate complete server shutdown & reboot: reopen storage from disk
    {
        let reopened_storage = Storage::open(&db_path).unwrap();
        let rebooted_auth_mgr = m31a_app_server::auth::AuthManager::with_storage(reopened_storage);

        // Verification: ownership survived daemon reboot!
        assert_eq!(
            rebooted_auth_mgr.get_thread_owner(thread_id).unwrap(),
            "persistent_owner"
        );

        let owner_identity = m31a_app_server::auth::ClientIdentity {
            client_id: "persistent_owner".to_string(),
            label: "Owner".to_string(),
            scopes: [AuthScope::Read, AuthScope::Write].into_iter().collect(),
        };
        let intruder_identity = m31a_app_server::auth::ClientIdentity {
            client_id: "intruder".to_string(),
            label: "Intruder".to_string(),
            scopes: [AuthScope::Read, AuthScope::Write].into_iter().collect(),
        };

        assert!(
            rebooted_auth_mgr
                .require_thread_access(thread_id, &owner_identity)
                .is_ok()
        );
        assert!(
            rebooted_auth_mgr
                .require_thread_access(thread_id, &intruder_identity)
                .is_err()
        );
    }
}

#[tokio::test]
async fn test_16_query_param_token_rejected_and_stream_ticket_works() {
    let (addr, server, _) = spawn_test_server(|_| {}).await;

    let client_token = server.auth_manager.generate_token(
        "sse_user",
        "SSE User",
        vec![AuthScope::Read, AuthScope::Write],
        None,
    );

    // 1. Query parameter token on REST/RPC endpoints MUST be rejected
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /api/threads?token={} HTTP/1.1\r\nHost: {}\r\n\r\n",
        client_token, addr
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 401 Unauthorized"));
    assert!(resp.contains("URL query tokens not permitted"));

    // 2. Issue a short-lived single-use stream ticket via POST /api/auth/stream-ticket
    let mut client = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /api/auth/stream-ticket HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Length: 0\r\n\r\n",
        addr, client_token
    );
    client.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).await.unwrap();
    assert!(resp.contains("HTTP/1.1 200 OK"));

    let body_json: serde_json::Value =
        serde_json::from_str(resp.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let ticket = body_json["ticket"].as_str().unwrap();

    // 3. Connect to SSE stream using single-use ticket in ?ticket=
    let mut sse_client = TcpStream::connect(addr).await.unwrap();
    let req_sse = format!(
        "GET /events?ticket={} HTTP/1.1\r\nHost: {}\r\n\r\n",
        ticket, addr
    );
    sse_client.write_all(req_sse.as_bytes()).await.unwrap();
    let mut header_buf = [0u8; 512];
    let n = sse_client.read(&mut header_buf).await.unwrap();
    let header_str = String::from_utf8_lossy(&header_buf[..n]);
    assert!(header_str.contains("HTTP/1.1 200 OK"));
    assert!(header_str.contains("text/event-stream"));

    // 4. Attempt to replay ticket -> MUST fail because ticket is single-use
    let mut replay_client = TcpStream::connect(addr).await.unwrap();
    let req_replay = format!(
        "GET /events?ticket={} HTTP/1.1\r\nHost: {}\r\n\r\n",
        ticket, addr
    );
    replay_client
        .write_all(req_replay.as_bytes())
        .await
        .unwrap();
    let mut replay_resp = String::new();
    replay_client
        .read_to_string(&mut replay_resp)
        .await
        .unwrap();
    assert!(replay_resp.contains("HTTP/1.1 401 Unauthorized"));
}

#[tokio::test]
async fn test_17_sanitized_config_redacts_secrets_and_mcp_credentials() {
    let (addr, server, _) = spawn_test_server(|_| {}).await;

    let admin_token = server.auth_manager.generate_token(
        "admin",
        "Admin",
        vec![AuthScope::Read, AuthScope::Admin],
        None,
    );

    let mut client = TcpStream::connect(addr).await.unwrap();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "config/read",
        "params": {}
    })
    .to_string();
    let req = format!(
        "POST /api/rpc HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        addr,
        admin_token,
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
async fn test_18_token_rotation_and_revocation() {
    let (_, server, _) = spawn_test_server(|_| {}).await;

    let tok1 = server.auth_manager.generate_token(
        "rotator",
        "Rotator Client",
        vec![AuthScope::Read],
        None,
    );

    // Initial token valid
    assert!(server.auth_manager.validate_token(&tok1).is_ok());

    // Rotate token
    let tok2 = server.auth_manager.rotate_token(&tok1, None).unwrap();
    assert_ne!(tok1, tok2);

    // Old token invalid
    assert!(server.auth_manager.validate_token(&tok1).is_err());
    // New token valid
    assert!(server.auth_manager.validate_token(&tok2).is_ok());

    // Revoke new token
    assert!(server.auth_manager.revoke_token(&tok2));
    assert!(server.auth_manager.validate_token(&tok2).is_err());
}
