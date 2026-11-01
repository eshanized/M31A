//! Dedicated integration tests for `m31a-browser` (src/cdp.rs).
//!
//! Validates:
//! - CDP request, response, and error frame creation and Serde serialization
//! - CDP asynchronous event notifications
//! - Console message model representation and comparison
//! - Network interceptor record fields and headers
//! - Heap snapshot metrics and delta leak analysis structures

use m31a_browser::cdp::{
    CdpError, CdpEvent, CdpRequest, CdpResponse, ConsoleMessage, HeapDiff, HeapSnapshotSummary,
    NetworkRecord, TraceResult,
};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_cdp_request_and_response_serialization() {
    let req = CdpRequest::new(42, "Page.navigate", Some(json!({ "url": "https://example.com" })));
    let req_json = serde_json::to_string(&req).expect("serialize request");
    assert!(req_json.contains("\"id\":42"));
    assert!(req_json.contains("\"method\":\"Page.navigate\""));
    assert!(req_json.contains("\"url\":\"https://example.com\""));

    // Deserialization roundtrip
    let deserialized_req: CdpRequest = serde_json::from_str(&req_json).expect("deserialize request");
    assert_eq!(deserialized_req.id, 42);
    assert_eq!(deserialized_req.method, "Page.navigate");

    // Success response
    let resp = CdpResponse {
        id: 42,
        result: Some(json!({ "frameId": "F100" })),
        error: None,
    };
    let resp_json = serde_json::to_string(&resp).expect("serialize response");
    assert!(resp_json.contains("\"frameId\":\"F100\""));

    // Error response
    let err_resp = CdpResponse {
        id: 43,
        result: None,
        error: Some(CdpError {
            code: -32000,
            message: "Cannot navigate to invalid URL".to_string(),
            data: None,
        }),
    };
    let err_json = serde_json::to_string(&err_resp).expect("serialize error response");
    assert!(err_json.contains("-32000"));
    assert!(err_json.contains("Cannot navigate"));
}

#[test]
fn test_cdp_event_and_console_message() {
    let event = CdpEvent {
        method: "Runtime.consoleAPICalled".to_string(),
        params: json!({
            "type": "log",
            "args": [{ "value": "Application initialized" }]
        }),
    };
    assert_eq!(event.method, "Runtime.consoleAPICalled");

    let console_msg = ConsoleMessage {
        level: "warn".to_string(),
        text: "Deprecated API called".to_string(),
        timestamp_ms: 1700000000000,
        source: Some("runtime.js:14".to_string()),
    };
    let msg_json = serde_json::to_string(&console_msg).expect("serialize console msg");
    let deserialized_msg: ConsoleMessage = serde_json::from_str(&msg_json).unwrap();
    assert_eq!(deserialized_msg, console_msg);
}

#[test]
fn test_network_record_and_trace_result() {
    let mut req_headers = HashMap::new();
    req_headers.insert("User-Agent".to_string(), "M31A-Agent/1.0".to_string());
    let mut resp_headers = HashMap::new();
    resp_headers.insert("Content-Type".to_string(), "application/json".to_string());

    let record = NetworkRecord {
        request_id: "req-1234".to_string(),
        url: "https://api.example.com/v1/status".to_string(),
        method: "GET".to_string(),
        status: Some(200),
        resource_type: "fetch".to_string(),
        headers: req_headers,
        response_headers: resp_headers,
        mime_type: Some("application/json".to_string()),
        encoded_data_length: Some(4096),
        timestamp_ms: 1700000000100,
    };

    assert_eq!(record.status, Some(200));
    assert_eq!(record.method, "GET");
    assert_eq!(record.mime_type.as_deref(), Some("application/json"));

    let trace = TraceResult {
        events_count: 150,
        duration_ms: 2500,
        events_json: "[{\"cat\":\"disabled-by-default-v8.cpu_profiler\"}]".to_string(),
    };
    assert_eq!(trace.events_count, 150);
    assert_eq!(trace.duration_ms, 2500);
}

#[test]
fn test_heap_snapshot_summary_and_diff() {
    let snap = HeapSnapshotSummary {
        total_heap_size_bytes: 50 * 1024 * 1024,
        used_heap_size_bytes: 32 * 1024 * 1024,
        heap_size_limit_bytes: 128 * 1024 * 1024,
        documents_count: 2,
        nodes_count: 1450,
        listeners_count: 28,
        timestamp_ms: 1700000000200,
    };
    assert_eq!(snap.documents_count, 2);

    let diff = HeapDiff {
        delta_used_bytes: 10 * 1024 * 1024,
        delta_nodes: 500,
        delta_listeners: 15,
        potential_leak: true,
        analysis: "Rapid growth in detached DOM nodes detected.".to_string(),
    };
    assert!(diff.potential_leak);
    assert_eq!(diff.delta_nodes, 500);
}
