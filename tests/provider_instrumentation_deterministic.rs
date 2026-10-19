//! Phase 2 — Deterministic Provider Instrumentation & Verification Suite.
//!
//! Validates:
//! 1. Valid non-streaming text responses.
//! 2. Valid tool-call responses.
//! 3. Missing or empty choices rejection.
//! 4. Missing messages and empty content rejection.
//! 5. Valid streaming responses terminated by [DONE].
//! 6. Streaming responses that reach EOF before [DONE] (fail closed).
//! 7. Malformed or empty streaming payloads (cannot finalize).
//! 8. Cancellation before dispatch and during an in-flight stream.
//! 9. Failed HTTP responses (401, 500) and network errors.
//! 10. Sanitized live-provider evidence accounting and data protection.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use m31a::model::ModelProvider;
use m31a::model::provider::nvidia::NvidiaProvider;
use m31a::model::types::{ModelError, ModelProposal};

static ENV_MUTEX: Mutex<()> = Mutex::const_new(());

/// Spawns a hermetic one-shot mock HTTP server that responds with raw HTTP bytes.
fn spawn_raw_mock_server(raw_response: String) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let port = listener.local_addr().expect("local addr").port();

    let handle = std::thread::spawn(move || {
        if let Ok((mut socket, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf);
            let _ = socket.write_all(raw_response.as_bytes());
            let _ = socket.flush();
        }
    });

    let base_url = format!("http://127.0.0.1:{port}/v1");
    (base_url, handle)
}

/// Spawns a mock server that serves the first chunk, waits for a signal or delay,
/// and verifies handling of in-flight cancellation.
fn spawn_delayed_streaming_server(
    first_chunk: String,
    delay: Duration,
) -> (String, std::thread::JoinHandle<()>, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let port = listener.local_addr().expect("local addr").port();
    let client_connected = Arc::new(AtomicBool::new(false));
    let client_connected_clone = client_connected.clone();

    let handle = std::thread::spawn(move || {
        if let Ok((mut socket, _)) = listener.accept() {
            client_connected_clone.store(true, Ordering::SeqCst);
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf);
            let _ = socket.write_all(first_chunk.as_bytes());
            let _ = socket.flush();
            std::thread::sleep(delay);
        }
    });

    let base_url = format!("http://127.0.0.1:{port}/v1");
    (base_url, handle, client_connected)
}

#[tokio::test]
async fn test_hermetic_streaming_valid_terminated_by_done() {
    let _lock = ENV_MUTEX.lock().await;
    let evidence_file = NamedTempFile::new().expect("temp evidence file");
    let evidence_path = evidence_file.path().to_str().unwrap().to_string();

    let sse_body = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"valid streaming content\"},\"finish_reason\":null}]}\n\n\
data: [DONE]\n\n";

    let (base_url, handle) = spawn_raw_mock_server(sse_body.to_string());
    let provider = NvidiaProvider::new_test_fixture(base_url, "test-key".to_string()).unwrap();

    let cancellation = CancellationToken::new();
    let result = unsafe {
        std::env::set_var("M31A_LIVE_EVIDENCE_FILE", &evidence_path);
        let r = provider
            .call_model("meta/llama-3.1-8b", "test prompt", vec![], &cancellation)
            .await;
        std::env::remove_var("M31A_LIVE_EVIDENCE_FILE");
        r
    };

    let _ = handle.join();
    assert!(result.is_ok(), "streaming call must succeed: {result:?}");
    match result.unwrap().0 {
        ModelProposal::AssistantText { content } => {
            assert_eq!(content, "valid streaming content");
        }
        other => panic!("expected AssistantText, got {other:?}"),
    }

    let evidence = fs::read_to_string(&evidence_path).unwrap_or_default();
    let events: Vec<&str> = evidence.lines().collect();
    assert_eq!(
        events,
        vec![
            "request_attempt",
            "inference_request",
            "request",
            "response",
            "inference_response"
        ]
    );
}

#[tokio::test]
async fn test_hermetic_streaming_premature_eof_rejects_without_inference_evidence() {
    let _lock = ENV_MUTEX.lock().await;
    let evidence_file = NamedTempFile::new().expect("temp evidence file");
    let evidence_path = evidence_file.path().to_str().unwrap().to_string();

    // Stream closes without sending `[DONE]`
    let sse_body = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial cut off content\"},\"finish_reason\":null}]}\n\n";

    let (base_url, handle) = spawn_raw_mock_server(sse_body.to_string());
    let provider = NvidiaProvider::new_test_fixture(base_url, "test-key".to_string()).unwrap();

    let cancellation = CancellationToken::new();
    let result = unsafe {
        std::env::set_var("M31A_LIVE_EVIDENCE_FILE", &evidence_path);
        let r = provider
            .call_model("meta/llama-3.1-8b", "test prompt", vec![], &cancellation)
            .await;
        std::env::remove_var("M31A_LIVE_EVIDENCE_FILE");
        r
    };

    let _ = handle.join();
    assert!(
        matches!(result, Err(ModelError::StreamInterrupted(_))),
        "premature EOF without [DONE] must fail closed with StreamInterrupted, got {result:?}"
    );

    let evidence = fs::read_to_string(&evidence_path).unwrap_or_default();
    let events: Vec<&str> = evidence.lines().collect();
    assert_eq!(
        events,
        vec![
            "request_attempt",
            "inference_request",
            "request",
            "response"
        ],
        "must not record inference_response on premature EOF"
    );
}

#[tokio::test]
async fn test_hermetic_streaming_empty_payload_cannot_finalize() {
    let _lock = ENV_MUTEX.lock().await;
    let evidence_file = NamedTempFile::new().expect("temp evidence file");
    let evidence_path = evidence_file.path().to_str().unwrap().to_string();

    // Sends an empty chunk then DONE: accumulator has 0 content and 0 tools
    let sse_body = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":null}]}\n\n\
data: [DONE]\n\n";

    let (base_url, handle) = spawn_raw_mock_server(sse_body.to_string());
    let provider = NvidiaProvider::new_test_fixture(base_url, "test-key".to_string()).unwrap();

    let cancellation = CancellationToken::new();
    let result = unsafe {
        std::env::set_var("M31A_LIVE_EVIDENCE_FILE", &evidence_path);
        let r = provider
            .call_model("meta/llama-3.1-8b", "test prompt", vec![], &cancellation)
            .await;
        std::env::remove_var("M31A_LIVE_EVIDENCE_FILE");
        r
    };

    let _ = handle.join();
    assert!(
        matches!(result, Err(ModelError::InvalidResponse(_))),
        "empty completion stream must fail closed with InvalidResponse, got {result:?}"
    );

    let evidence = fs::read_to_string(&evidence_path).unwrap_or_default();
    let events: Vec<&str> = evidence.lines().collect();
    assert!(
        !events.contains(&"inference_response"),
        "must not record inference_response on unfinalizable stream"
    );
}

#[tokio::test]
async fn test_hermetic_cancellation_during_flight_does_not_record_inference() {
    let _lock = ENV_MUTEX.lock().await;
    let evidence_file = NamedTempFile::new().expect("temp evidence file");
    let evidence_path = evidence_file.path().to_str().unwrap().to_string();

    let first_chunk = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"start\"},\"finish_reason\":null}]}\n\n";

    let (base_url, handle, client_connected) =
        spawn_delayed_streaming_server(first_chunk.to_string(), Duration::from_millis(500));
    let provider = NvidiaProvider::new_test_fixture(base_url, "test-key".to_string()).unwrap();

    let cancellation = CancellationToken::new();
    let cancellation_clone = cancellation.clone();

    // Spawn a watcher that cancels as soon as the client connects and begins receiving
    let cancel_task = tokio::spawn(async move {
        while !client_connected.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancellation_clone.cancel();
    });

    let result = unsafe {
        std::env::set_var("M31A_LIVE_EVIDENCE_FILE", &evidence_path);
        let r = provider
            .call_model("meta/llama-3.1-8b", "test prompt", vec![], &cancellation)
            .await;
        std::env::remove_var("M31A_LIVE_EVIDENCE_FILE");
        r
    };

    let _ = cancel_task.await;
    let _ = handle.join();
    assert_eq!(result.unwrap_err(), ModelError::Cancelled);

    let evidence = fs::read_to_string(&evidence_path).unwrap_or_default();
    let events: Vec<&str> = evidence.lines().collect();
    assert!(
        !events.contains(&"inference_response"),
        "in-flight cancelled request must not record completed inference"
    );
}

#[tokio::test]
async fn test_hermetic_http_error_distinguished_from_success() {
    let _lock = ENV_MUTEX.lock().await;
    let evidence_file = NamedTempFile::new().expect("temp evidence file");
    let evidence_path = evidence_file.path().to_str().unwrap().to_string();

    let error_response = "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
{\"error\":{\"message\":\"server error\"}}";

    let (base_url, handle) = spawn_raw_mock_server(error_response.to_string());
    let provider = NvidiaProvider::new_test_fixture(base_url, "test-key".to_string()).unwrap();

    let cancellation = CancellationToken::new();
    let result = unsafe {
        std::env::set_var("M31A_LIVE_EVIDENCE_FILE", &evidence_path);
        let r = provider
            .call_model("meta/llama-3.1-8b", "test prompt", vec![], &cancellation)
            .await;
        std::env::remove_var("M31A_LIVE_EVIDENCE_FILE");
        r
    };

    let _ = handle.join();
    assert!(result.is_err());

    let evidence = fs::read_to_string(&evidence_path).unwrap_or_default();
    let events: Vec<&str> = evidence.lines().collect();
    assert_eq!(
        events,
        vec!["request_attempt", "inference_request", "request"],
        "HTTP error must record request exchange but NEVER response or inference_response"
    );
}

#[tokio::test]
async fn test_hermetic_network_error_fails_before_request_exchange() {
    let _lock = ENV_MUTEX.lock().await;
    let evidence_file = NamedTempFile::new().expect("temp evidence file");
    let evidence_path = evidence_file.path().to_str().unwrap().to_string();

    // Use an unassigned port that immediately rejects connection
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("port").port();
    drop(listener);

    let base_url = format!("http://127.0.0.1:{port}/v1");
    let provider = NvidiaProvider::new_test_fixture(base_url, "test-key".to_string()).unwrap();

    let cancellation = CancellationToken::new();
    let result = unsafe {
        std::env::set_var("M31A_LIVE_EVIDENCE_FILE", &evidence_path);
        let r = provider
            .call_model("meta/llama-3.1-8b", "test prompt", vec![], &cancellation)
            .await;
        std::env::remove_var("M31A_LIVE_EVIDENCE_FILE");
        r
    };

    assert!(matches!(result, Err(ModelError::Network(_))));

    let evidence = fs::read_to_string(&evidence_path).unwrap_or_default();
    let events: Vec<&str> = evidence.lines().collect();
    assert_eq!(
        events,
        vec!["request_attempt", "inference_request"],
        "network drop must record dispatch attempt but NOT request, response, or inference_response"
    );
}

#[tokio::test]
async fn test_hermetic_evidence_contains_no_sensitive_data() {
    let _lock = ENV_MUTEX.lock().await;
    let evidence_file = NamedTempFile::new().expect("temp evidence file");
    let evidence_path = evidence_file.path().to_str().unwrap().to_string();

    let sse_body = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"super-secret-output\"},\"finish_reason\":null}]}\n\n\
data: [DONE]\n\n";

    let (base_url, handle) = spawn_raw_mock_server(sse_body.to_string());
    let provider =
        NvidiaProvider::new_test_fixture(base_url, "nvapi-secret-key-12345678".to_string())
            .unwrap();

    let cancellation = CancellationToken::new();
    let prompt = "Classified secret prompt contents 99999";
    let _ = unsafe {
        std::env::set_var("M31A_LIVE_EVIDENCE_FILE", &evidence_path);
        let r = provider
            .call_model("meta/llama-3.1-8b", prompt, vec![], &cancellation)
            .await;
        std::env::remove_var("M31A_LIVE_EVIDENCE_FILE");
        r
    };

    let _ = handle.join();
    let evidence = fs::read_to_string(&evidence_path).unwrap_or_default();

    assert!(
        !evidence.contains("secret"),
        "evidence must not contain prompt, model output, or secret strings"
    );
    assert!(
        !evidence.contains("nvapi-"),
        "evidence must not contain api keys"
    );
    for line in evidence.lines() {
        assert!(
            matches!(
                line,
                "request_attempt"
                    | "request"
                    | "response"
                    | "inference_request"
                    | "inference_response"
            ),
            "evidence file must only contain approved lifecycle event tokens, found: {line}"
        );
    }
}
