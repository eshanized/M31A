//! Phase 12 Telemetry, CorrelationContext & Secret Redaction Verification Suite (OBS-01, OBS-02, OBS-03).

use std::sync::Arc;
use tempfile::tempdir;

use m31a::cli::args::{Cli, Commands, TelemetryArgs, TelemetryCommands};
use m31a::cli::dispatch::CliDispatcher;
use m31a::ids::{AgentId, MissionId, TaskId};
use m31a::persistence::sqlite::repositories::SqliteTelemetryRepository;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::telemetry::{
    CorrelationContext, MetricSample, NdjsonStreamWriter, SecretRedactor, SpanKind, SpanStatus,
    TelemetryCollector, TelemetrySpan, inspect_telemetry,
};

#[tokio::test]
async fn test_correlation_context_and_redaction() {
    // 1. Verify CorrelationContext properties
    let mission_id = MissionId::new();
    let root = CorrelationContext::new_root(mission_id);

    assert_eq!(root.trace_id.len(), 32);
    assert_eq!(root.span_id.len(), 16);
    assert!(root.parent_span_id.is_none());
    assert_eq!(root.mission_id, Some(mission_id));

    let child = root.child_span();
    assert_eq!(child.trace_id, root.trace_id);
    assert_eq!(child.span_id.len(), 16);
    assert_ne!(child.span_id, root.span_id);
    assert_eq!(child.parent_span_id, Some(root.span_id));

    // 2. Verify SecretRedactor
    let redactor = SecretRedactor::new();
    redactor.register_secret("super_sensitive_api_secret_key_123");

    let dirty = "Payload with super_sensitive_api_secret_key_123 and Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.doNotLeakThisToken\r\n\x1b[32mOK\x1b[0m";
    let cleaned = redactor.redact_string(dirty);

    assert!(!cleaned.contains("super_sensitive_api_secret_key_123"));
    assert!(!cleaned.contains("doNotLeakThisToken"));
    assert!(!cleaned.contains("\r"));
    assert!(!cleaned.contains("\x1b[32m"));
    assert!(cleaned.contains("[REDACTED:EXACT_SECRET]"));
    assert!(cleaned.contains("[REDACTED:BEARER_TOKEN]"));
    assert!(cleaned.contains("OK"));
}

#[tokio::test]
async fn test_correlation_context_hierarchy() {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();

    let root = CorrelationContext::new_root(mission_id);
    let task_span = root.child_span().with_task(task_id);
    let tool_span = task_span
        .child_span()
        .with_agent(agent_id)
        .with_tool_call("tool_exec_42");

    assert_eq!(tool_span.trace_id, root.trace_id);
    assert_eq!(tool_span.parent_span_id, Some(task_span.span_id));
    assert_eq!(tool_span.mission_id, Some(mission_id));
    assert_eq!(tool_span.task_id, Some(task_id));
    assert_eq!(tool_span.agent_id, Some(agent_id));
    assert_eq!(tool_span.tool_call_id, Some("tool_exec_42".to_string()));
}

#[tokio::test]
async fn test_multi_tier_secret_redaction() {
    let redactor = SecretRedactor::new();
    redactor.register_secret("my_db_password_xyz");

    // Test AWS key scrubbing
    let aws_input = "Access: AKIAIOSFODNN7EXAMPLE allowed.";
    assert_eq!(
        redactor.redact_string(aws_input),
        "Access: [REDACTED:AWS_KEY] allowed."
    );

    // Test Private key scrubbing
    let rsa_input =
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0...fake\n-----END RSA PRIVATE KEY-----";
    assert_eq!(redactor.redact_string(rsa_input), "[REDACTED:PRIVATE_KEY]");

    // Test JSON recursive redaction
    let mut obj = serde_json::json!({
        "status": "ok",
        "nested": {
            "token": "sensitive_val",
            "api_key": "some_key",
            "secret_field": "val",
            "normal_field": "User password is my_db_password_xyz"
        },
        "items": [
            "safe",
            "Bearer eyJ.payload.sig"
        ]
    });

    redactor.redact_value(&mut obj);

    assert_eq!(obj["nested"]["token"], "[REDACTED:SENSITIVE_KEY]");
    assert_eq!(obj["nested"]["api_key"], "[REDACTED:SENSITIVE_KEY]");
    assert_eq!(obj["nested"]["secret_field"], "[REDACTED:SENSITIVE_KEY]");
    assert_eq!(
        obj["nested"]["normal_field"],
        "User password is [REDACTED:EXACT_SECRET]"
    );
    assert_eq!(obj["items"][1], "[REDACTED:BEARER_TOKEN]");
}

#[tokio::test]
async fn test_sqlite_telemetry_storage() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("m31a_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let repo = SqliteTelemetryRepository::new(pool);
    let mission_id = MissionId::new();

    let mut span1 = TelemetrySpan::new(
        "span_01".to_string(),
        "trace_01".to_string(),
        None,
        mission_id,
        "run_model".to_string(),
        SpanKind::Model,
        1000,
    );
    span1.finish(2500, SpanStatus::Ok, None);
    repo.record_span(&span1).await.unwrap();

    let mut span2 = TelemetrySpan::new(
        "span_02".to_string(),
        "trace_01".to_string(),
        Some("span_01".to_string()),
        mission_id,
        "execute_tool".to_string(),
        SpanKind::Tool,
        2600,
    );
    span2.finish(3100, SpanStatus::Ok, None);
    repo.record_span(&span2).await.unwrap();

    let metric = MetricSample {
        id: None,
        mission_id,
        timestamp_us: 3200,
        metric_name: "tokens_total".to_string(),
        metric_value: 1542.0,
        metric_unit: "tokens".to_string(),
        labels: serde_json::json!({"model": "test-llm"}),
    };
    repo.record_metric_sample(&metric).await.unwrap();

    let cost_metric = MetricSample {
        id: None,
        mission_id,
        timestamp_us: 3201,
        metric_name: "cost_usd".to_string(),
        metric_value: 0.045,
        metric_unit: "USD".to_string(),
        labels: serde_json::json!({}),
    };
    repo.record_metric_sample(&cost_metric).await.unwrap();

    let spans = repo.get_spans_for_mission(&mission_id).await.unwrap();
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].name, "run_model");
    assert_eq!(spans[1].name, "execute_tool");

    let metrics = repo.get_metrics_for_mission(&mission_id).await.unwrap();
    assert_eq!(metrics.len(), 2);

    let summary = repo.get_summary(&mission_id).await.unwrap();
    assert_eq!(summary.total_spans, 2);
    assert_eq!(summary.successful_spans, 2);
    assert_eq!(summary.model_calls, 1);
    assert_eq!(summary.tool_calls, 1);
    assert_eq!(summary.total_tokens, 1542);
    assert!((summary.estimated_cost_usd - 0.045).abs() < 1e-6);
}

#[tokio::test]
async fn test_ndjson_stream_output() {
    let dir = tempdir().unwrap();
    let stream_dir = dir.path().join("telemetry");
    let writer = NdjsonStreamWriter::new(stream_dir);
    let mission_id = MissionId::new();

    let entry = serde_json::json!({
        "event": "step_finish",
        "step": 1,
        "token_usage": 450
    });

    writer.append_entry(&mission_id, &entry).await.unwrap();

    let entries = writer.read_entries(&mission_id).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["step"], 1);
    assert_eq!(entries[0]["token_usage"], 450);
}

#[tokio::test]
async fn test_cli_telemetry_inspect() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("m31a_cli_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let stream_dir = dir.path().join("telemetry");
    let writer = NdjsonStreamWriter::new(stream_dir);
    let repo = SqliteTelemetryRepository::new(pool.clone());
    let redactor = Arc::new(SecretRedactor::new());
    redactor.register_secret("my_classified_token");

    let collector = TelemetryCollector::new(redactor, repo.clone(), writer.clone());
    let mission_id = MissionId::new();
    let root_ctx = CorrelationContext::new_root(mission_id);

    // Run spans through TelemetryCollector
    let span_id = collector
        .start_span(&root_ctx, "model_inference", SpanKind::Model)
        .await
        .unwrap();

    collector
        .finish_span(
            &span_id,
            SpanStatus::Ok,
            None,
            serde_json::json!({
                "prompt_tokens": 120,
                "secret_info": "my_classified_token"
            }),
        )
        .await
        .unwrap();

    collector
        .record_metric(
            mission_id,
            "tokens_total",
            120.0,
            "tokens",
            serde_json::json!({"auth": "Bearer eyJhbGciOiJIUzI1NiJ9.abc.def"}),
        )
        .await
        .unwrap();

    // 1. Inspect as human-readable text
    let text_output = inspect_telemetry(&repo, &writer, &mission_id, true, true, true, None)
        .await
        .unwrap();

    assert!(text_output.text.contains("Mission Telemetry"));
    assert!(text_output.text.contains("Model Invocations:  1"));
    assert!(text_output.text.contains("model_inference"));
    assert!(text_output.text.contains("tokens_total"));

    // 2. Inspect as machine-readable JSON
    let json_output =
        inspect_telemetry(&repo, &writer, &mission_id, true, true, true, Some("json"))
            .await
            .unwrap();

    assert_eq!(json_output.data["summary"]["total_spans"], 1);
    assert_eq!(json_output.data["spans"][0]["name"], "model_inference");
    // Verify secret is redacted in stored/inspected data!
    assert_eq!(
        json_output.data["spans"][0]["attributes"]["secret_info"],
        "[REDACTED:SENSITIVE_KEY]"
    );

    // 3. Verify CLI dispatcher routing
    let cli = Cli {
        config: None,
        profile: None,
        model: None,
        autonomy: None,
        output: m31a::cli::args::OutputFormat::Text,
        quiet: false,
        verbose: false,
        workspace: None,
        command: Some(Commands::Telemetry(TelemetryArgs {
            command: TelemetryCommands::Inspect {
                mission_id: mission_id.to_string(),
                summary: true,
                spans: true,
                metrics: false,
                export: None,
            },
        })),
    };

    let dispatcher = CliDispatcher::new().with_pool(pool);

    let runtime_cmd = dispatcher.parse_command(&cli).expect("parsed command");
    let res = dispatcher
        .dispatch(runtime_cmd)
        .await
        .expect("dispatched inspect");
    assert!(res.text.contains("Mission Telemetry"));
}
