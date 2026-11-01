//! Dedicated integration tests for `m31a-telemetry` (src/lib.rs).
//!
//! Validates:
//! - SQLite audit trail exporter with real queryable schema and token calculation
//! - JSONL file exporter with line-delimited streaming persistence
//! - TelemetryReducer statistics aggregation across sessions, turns, and tools
//! - TelemetryPipeline asynchronous batching, periodic flushing, and channel lifecycle

use m31a_core::types::TokenUsage;
use m31a_telemetry::{
    JsonlExporter, SqliteAuditExporter, TelemetryEvent, TelemetryExporter, TelemetryPipeline,
    TelemetryReducer, ViolationSeverity,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_sqlite_audit_exporter_lifecycle_and_queries() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("audit.sqlite");

    let exporter = SqliteAuditExporter::new(&db_path).unwrap();

    let session_id = "sess-alpha-001";
    let events = vec![
        TelemetryEvent::SessionStart {
            session_id: session_id.to_string(),
            model: "claude-3-5-sonnet".to_string(),
            provider: "anthropic".to_string(),
            config_hash: "hash-1234".to_string(),
            timestamp: 1700000001,
        },
        TelemetryEvent::TurnStart {
            session_id: session_id.to_string(),
            turn_id: "turn-1".to_string(),
            tools_available: 5,
            timestamp: 1700000002,
        },
        TelemetryEvent::ModelCall {
            session_id: session_id.to_string(),
            turn_id: "turn-1".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            provider: "anthropic".to_string(),
            input_tokens: 1500,
            output_tokens: 500,
            reasoning_tokens: 200,
            latency_ms: 1200,
            cached: false,
            timestamp: 1700000003,
        },
        TelemetryEvent::ToolInvocation {
            session_id: session_id.to_string(),
            turn_id: "turn-1".to_string(),
            tool_name: "read_file".to_string(),
            tool_category: "filesystem".to_string(),
            arguments_hash: "arg-abc".to_string(),
            success: true,
            duration_ms: 45,
            tokens_used: Some(150),
            timestamp: 1700000004,
        },
        TelemetryEvent::SandboxViolation {
            session_id: session_id.to_string(),
            turn_id: "turn-1".to_string(),
            operation: "open".to_string(),
            path: "/etc/shadow".to_string(),
            policy: "WorkspaceWrite".to_string(),
            severity: ViolationSeverity::Critical,
            timestamp: 1700000005,
        },
        TelemetryEvent::TurnEnd {
            session_id: session_id.to_string(),
            turn_id: "turn-1".to_string(),
            success: true,
            tokens_used: TokenUsage {
                prompt_tokens: 1500,
                completion_tokens: 500,
                total_tokens: 2000,
                reasoning_tokens: 200,
            },
            duration_ms: 2500,
            timestamp: 1700000006,
        },
    ];

    exporter.export(events).unwrap();

    // Query back from SQLite
    let queried = exporter.query_events(Some(session_id), 100).unwrap();
    assert_eq!(queried.len(), 6, "All 6 exported events must be indexed");
    assert_eq!(queried[0].1, "session_start");
    assert_eq!(queried[4].1, "sandbox_violation");

    // Token accounting query
    let total_tokens = exporter.query_total_tokens(session_id).unwrap();
    // ModelCall (1500 + 500 = 2000) + TurnEnd (2000) = 4000
    assert_eq!(total_tokens, 4000);

    // Verify isolation against other session ID
    let other_events = exporter.query_events(Some("other-session"), 100).unwrap();
    assert!(other_events.is_empty());
}

#[test]
fn test_jsonl_exporter_streaming_file() {
    let tmp = tempdir().unwrap();
    let jsonl_path = tmp.path().join("telemetry.jsonl");

    let exporter = JsonlExporter::new(jsonl_path.clone());
    assert_eq!(exporter.name(), "jsonl");

    let event1 = TelemetryEvent::SessionStart {
        session_id: "sess-jsonl-1".to_string(),
        model: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        config_hash: "cfg-001".to_string(),
        timestamp: 1700000100,
    };
    let event2 = TelemetryEvent::Error {
        session_id: Some("sess-jsonl-1".to_string()),
        turn_id: Some("turn-9".to_string()),
        error_type: "NetworkTimeout".to_string(),
        error_message: "Connection reset by peer".to_string(),
        recoverable: true,
        context: HashMap::new(),
        timestamp: 1700000105,
    };

    exporter.export(vec![event1, event2]).unwrap();

    // Verify file content has exactly 2 valid JSON lines
    let file = File::open(&jsonl_path).unwrap();
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();
    assert_eq!(lines.len(), 2);

    let parsed1: TelemetryEvent = serde_json::from_str(&lines[0]).unwrap();
    if let TelemetryEvent::SessionStart { session_id, .. } = parsed1 {
        assert_eq!(session_id, "sess-jsonl-1");
    } else {
        panic!("Line 1 mismatch");
    }

    let parsed2: TelemetryEvent = serde_json::from_str(&lines[1]).unwrap();
    if let TelemetryEvent::Error { error_type, .. } = parsed2 {
        assert_eq!(error_type, "NetworkTimeout");
    } else {
        panic!("Line 2 mismatch");
    }
}

#[test]
fn test_telemetry_reducer_stats_accumulation() {
    let mut reducer = TelemetryReducer::new();

    let session_id = "sess-metrics-1".to_string();

    reducer.process(&TelemetryEvent::SessionStart {
        session_id: session_id.clone(),
        model: "m1".to_string(),
        provider: "p1".to_string(),
        config_hash: "ch1".to_string(),
        timestamp: 100,
    });

    reducer.process(&TelemetryEvent::TurnEnd {
        session_id: session_id.clone(),
        turn_id: "t1".to_string(),
        success: true,
        tokens_used: TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            reasoning_tokens: 0,
        },
        duration_ms: 500,
        timestamp: 110,
    });

    reducer.process(&TelemetryEvent::ToolInvocation {
        session_id: session_id.clone(),
        turn_id: "t1".to_string(),
        tool_name: "grep".to_string(),
        tool_category: "search".to_string(),
        arguments_hash: "gh".to_string(),
        success: true,
        duration_ms: 30,
        tokens_used: None,
        timestamp: 115,
    });

    reducer.process(&TelemetryEvent::ToolInvocation {
        session_id: session_id.clone(),
        turn_id: "t1".to_string(),
        tool_name: "grep".to_string(),
        tool_category: "search".to_string(),
        arguments_hash: "gh".to_string(),
        success: false,
        duration_ms: 25,
        tokens_used: None,
        timestamp: 120,
    });

    // Finalize session
    reducer.process(&TelemetryEvent::SessionEnd {
        session_id: session_id.clone(),
        duration_ms: 1000,
        total_turns: 1,
        total_tokens: 150,
        timestamp: 200,
    });

    // Inspect aggregated stats
    let global = reducer.get_global_stats();
    assert_eq!(global.total_sessions, 1);
    assert_eq!(global.total_turns, 1);
    assert_eq!(global.total_tokens, 150);
    assert_eq!(global.total_tool_invocations, 2);
    assert_eq!(global.total_errors, 1); // 1 failed tool invocation
}

#[tokio::test]
async fn test_pipeline_async_flow_and_flush() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("pipeline_audit.sqlite");

    let (mut pipeline, rx) = TelemetryPipeline::new(5, Duration::from_millis(100));
    let sqlite_exporter = SqliteAuditExporter::new(&db_path).unwrap();
    pipeline.add_exporter(Box::new(sqlite_exporter));

    let pipeline_arc = Arc::new(pipeline);
    let pipeline_worker = Arc::clone(&pipeline_arc);

    let handle = tokio::spawn(async move {
        pipeline_worker.run(rx).await;
    });

    // Send 3 events via convenience helpers
    pipeline_arc.session_start(
        "sess-pipe-1".to_string(),
        "deepseek-r1".to_string(),
        "deepseek".to_string(),
        "cfg-pipe".to_string(),
    );
    pipeline_arc.tool_invocation(
        "sess-pipe-1".to_string(),
        "turn-pipe-1".to_string(),
        "run_command".to_string(),
        "shell".to_string(),
        true,
        300,
    );
    pipeline_arc.model_call(
        "sess-pipe-1".to_string(),
        "turn-pipe-1".to_string(),
        "deepseek-r1".to_string(),
        "deepseek".to_string(),
        1000,
        300,
        500,
        1500,
    );

    // Drop the pipeline sender by re-acquiring / letting arc drop or wait for interval flush
    tokio::time::sleep(Duration::from_millis(250)).await;

    // Verify events were flushed to SQLite even before batch_size (5) reached, due to 100ms timer
    let reader_exporter = SqliteAuditExporter::new(&db_path).unwrap();
    let events = reader_exporter
        .query_events(Some("sess-pipe-1"), 10)
        .unwrap();
    assert_eq!(
        events.len(),
        3,
        "Periodic flush timer must persist buffered events"
    );

    // Cleanly cancel the background task
    handle.abort();
}
