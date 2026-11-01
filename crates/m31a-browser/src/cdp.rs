//! Chrome DevTools Protocol (CDP) serialization and types.
//!
//! Provides typed definitions for CDP requests, responses, and events,
//! along with performance tracing and heap profiling metadata.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A Chrome DevTools Protocol request frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpRequest {
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl CdpRequest {
    pub fn new(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }
}

/// A Chrome DevTools Protocol response frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpResponse {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CdpError>,
}

/// Detailed error payload returned by Chrome DevTools Protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// An unsolicited Chrome DevTools Protocol event notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
}

/// Console message captured from the browser Runtime or Console domain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsoleMessage {
    pub level: String, // "log", "warn", "error", "info"
    pub text: String,
    pub timestamp_ms: i64,
    pub source: Option<String>,
}

/// Intercepted HTTP network request record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRecord {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub status: Option<u16>,
    pub resource_type: String,
    pub headers: HashMap<String, String>,
    pub response_headers: HashMap<String, String>,
    pub mime_type: Option<String>,
    pub encoded_data_length: Option<u64>,
    pub timestamp_ms: i64,
}

/// Performance tracing capture outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceResult {
    pub events_count: usize,
    pub duration_ms: u64,
    pub events_json: String,
}

/// V8 heap memory metrics and DOM object counters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeapSnapshotSummary {
    pub total_heap_size_bytes: usize,
    pub used_heap_size_bytes: usize,
    pub heap_size_limit_bytes: usize,
    pub documents_count: usize,
    pub nodes_count: usize,
    pub listeners_count: usize,
    pub timestamp_ms: i64,
}

/// Delta analysis comparing two heap snapshots to detect memory leak vectors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeapDiff {
    pub delta_used_bytes: i64,
    pub delta_nodes: i64,
    pub delta_listeners: i64,
    pub potential_leak: bool,
    pub analysis: String,
}
