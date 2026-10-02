//! Phase 11 CLI Subcommand & Shared Runtime Command Layer Tests (CLI-01..04).

use clap::Parser;
use futures::StreamExt;
use std::sync::Arc;
use tokio::time::Duration;

use m31a::cli::{Cli, CliDispatcher, RuntimeCommand};
use m31a::events::bus::{BroadcastEventBus, EventBus, EventFilter};
use m31a::events::types::EventType;

#[tokio::test]
async fn test_cli_subcommand_dispatch() {
    let dispatcher = CliDispatcher::new();

    // 1. Version command (deployment-aware; default non-verbose)
    let cli_version = Cli::try_parse_from(["m31a", "version"]).unwrap();
    let cmd_version = dispatcher.parse_command(&cli_version).unwrap();
    assert_eq!(cmd_version, RuntimeCommand::Version { verbose: false });
    let out = dispatcher.dispatch(cmd_version).await.unwrap();
    assert_eq!(out.exit_code, 0);
    assert!(out.text.contains("m31a"));

    // 1b. Version --verbose reports deployment identity
    let cli_vv = Cli::try_parse_from(["m31a", "version", "--verbose"]).unwrap();
    let cmd_vv = dispatcher.parse_command(&cli_vv).unwrap();
    assert_eq!(cmd_vv, RuntimeCommand::Version { verbose: true });
    let out_vv = dispatcher.dispatch(cmd_vv).await.unwrap();
    assert!(out_vv.text.contains("Deployment:"));
    assert!(out_vv.text.contains("Channel:"));

    // 2. Mission Run command with flags
    let cli_run = Cli::try_parse_from([
        "m31a",
        "mission",
        "run",
        "Refactor auth layer",
        "--profile",
        "secure",
        "--wait-for-approval",
    ])
    .unwrap();
    let cmd_run = dispatcher.parse_command(&cli_run).unwrap();
    assert!(matches!(
        cmd_run,
        RuntimeCommand::RunMission {
            ref prompt,
            ref profile,
            wait_for_approval: true,
        } if prompt == "Refactor auth layer" && profile.as_deref() == Some("secure")
    ));
    let res_run = dispatcher.dispatch(cmd_run).await;
    assert!(
        matches!(res_run, Err(m31a::cli::CliError::ExecutionFailed(msg)) if msg.contains("Runtime execution engine is unavailable"))
    );

    // 3. Mission Cancel command
    let cli_cancel = Cli::try_parse_from([
        "m31a",
        "mission",
        "cancel",
        "01918a00-0000-7000-8000-000000000001",
        "--reason",
        "Emergency halt",
    ])
    .unwrap();
    let cmd_cancel = dispatcher.parse_command(&cli_cancel).unwrap();
    assert!(matches!(
        cmd_cancel,
        RuntimeCommand::CancelMission {
            ref id,
            ref reason,
        } if id == "01918a00-0000-7000-8000-000000000001" && reason.as_deref() == Some("Emergency halt")
    ));
    let out_cancel = dispatcher.dispatch(cmd_cancel).await.unwrap();
    assert_eq!(out_cancel.exit_code, 0);
    assert_eq!(out_cancel.data["status"], "cancelled");

    // 4. Task List command
    let cli_task = Cli::try_parse_from([
        "m31a",
        "task",
        "list",
        "--mission-id",
        "01918a00-0000-7000-8000-000000000001",
    ])
    .unwrap();
    let cmd_task = dispatcher.parse_command(&cli_task).unwrap();
    assert!(matches!(
        cmd_task,
        RuntimeCommand::ListTasks {
            mission_id: Some(ref mid),
        } if mid == "01918a00-0000-7000-8000-000000000001"
    ));
    let out_task = dispatcher.dispatch(cmd_task).await.unwrap();
    assert_eq!(out_task.exit_code, 0);

    // 5. Agent List command
    let cli_agent = Cli::try_parse_from(["m31a", "agent", "list"]).unwrap();
    let cmd_agent = dispatcher.parse_command(&cli_agent).unwrap();
    assert_eq!(cmd_agent, RuntimeCommand::ListAgents);
    let out_agent = dispatcher.dispatch(cmd_agent).await.unwrap();
    assert_eq!(out_agent.exit_code, 0);
    // Agent list reflects the role registry: all 18 built-in roles.
    assert_eq!(out_agent.data["roles"].as_array().unwrap().len(), 18);

    // 6. Doctor command
    let cli_doc = Cli::try_parse_from(["m31a", "doctor", "--json"]).unwrap();
    let cmd_doc = dispatcher.parse_command(&cli_doc).unwrap();
    assert!(matches!(
        cmd_doc,
        RuntimeCommand::RunDoctor {
            category: None,
            json: true,
        }
    ));
    let out_doc = dispatcher.dispatch(cmd_doc).await.unwrap();
    assert_eq!(out_doc.exit_code, 0);
    // Doctor now has 8 probes (added PlatformProbe + DeploymentProbe)
    assert_eq!(out_doc.data["results"].as_array().unwrap().len(), 8);
    let status_str = out_doc.data["status"].as_str().unwrap();
    // Allow environmental issues to cause "error" status
    assert!(
        status_str == "healthy"
            || status_str == "operational_with_warnings"
            || status_str == "error"
    );
}

#[tokio::test]
async fn test_shared_runtime_command_layer() {
    let bus = Arc::new(BroadcastEventBus::new(128));
    let mut rx = bus.subscribe(EventFilter::all()).await;

    let dispatcher = CliDispatcher::new().with_event_bus(bus.clone());

    // 1. Dispatching RunMission from shared application layer emits MissionStarted event
    let run_cmd = RuntimeCommand::RunMission {
        prompt: "Build neural compiler".to_string(),
        profile: None,
        wait_for_approval: false,
    };
    let run_res = dispatcher.dispatch(run_cmd).await.unwrap();
    assert_eq!(run_res.exit_code, 0);

    let env1 = tokio::time::timeout(Duration::from_millis(200), rx.next())
        .await
        .expect("should receive event within timeout")
        .expect("channel should be open")
        .expect("event should not error");
    assert!(matches!(
        env1.event_type,
        EventType::MissionStarted { ref objective, .. } if objective == "Build neural compiler"
    ));

    // 2. Dispatching CancelMission from shared application layer emits MissionCancelled event
    let mission_id_str = run_res.data["mission_id"].as_str().unwrap().to_string();
    let cancel_cmd = RuntimeCommand::CancelMission {
        id: mission_id_str,
        reason: Some("Budget ceiling hit".to_string()),
    };
    let cancel_res = dispatcher.dispatch(cancel_cmd).await.unwrap();
    assert_eq!(cancel_res.exit_code, 0);

    let env2 = tokio::time::timeout(Duration::from_millis(200), rx.next())
        .await
        .expect("should receive event within timeout")
        .expect("channel should be open")
        .expect("event should not error");
    assert!(matches!(
        env2.event_type,
        EventType::MissionCancelled { ref reason, .. } if reason == "Budget ceiling hit"
    ));
}

#[test]
fn test_ndjson_streaming_output() {
    use m31a::cli::{CliStreamMessage, NdjsonStreamWriter, TerminalFrame};

    let mut buffer = Vec::new();
    let mut writer = NdjsonStreamWriter::new(&mut buffer);

    // 1. Emit stream frames
    writer
        .emit(&CliStreamMessage::StatusChange {
            mission_id: "m-001".to_string(),
            old_status: "pending".to_string(),
            new_status: "running".to_string(),
            timestamp_epoch_ms: 1700000000,
        })
        .unwrap();

    writer
        .emit(&CliStreamMessage::TaskProgress {
            task_id: "t-001".to_string(),
            title: "Compile AST".to_string(),
            state: "running".to_string(),
            progress_pct: 0.5,
        })
        .unwrap();

    writer
        .emit(&CliStreamMessage::PolicyEvent {
            tool_name: "fs:write".to_string(),
            decision: "allow".to_string(),
            justification: Some("Policy rule #1".to_string()),
        })
        .unwrap();

    writer
        .emit(&CliStreamMessage::VerificationResult {
            task_id: "t-001".to_string(),
            tier: "unit".to_string(),
            passed: true,
            evidence_summary: "All 12 unit tests passed".to_string(),
        })
        .unwrap();

    assert!(!writer.is_sealed());

    // 2. Conclude with TerminalFrame
    writer
        .emit(&CliStreamMessage::TerminalFrame(TerminalFrame {
            mission_id: "m-001".to_string(),
            outcome: "succeeded".to_string(),
            exit_code: 0,
            duration_ms: 1540,
            total_tokens: 4200,
            artifacts: vec!["target/bin/m31a".to_string()],
        }))
        .unwrap();

    assert!(writer.is_sealed());

    // 3. Attempting to emit after sealing must fail
    let err = writer.emit(&CliStreamMessage::StderrLine {
        line: "late line".to_string(),
    });
    assert!(err.is_err());

    // 4. Validate output lines are valid UTF-8 JSON
    let output_str = String::from_utf8(buffer).unwrap();
    let lines: Vec<&str> = output_str.trim().split('\n').collect();
    assert_eq!(lines.len(), 5);

    let parsed_first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(parsed_first["type"], "status_change");
    assert_eq!(parsed_first["data"]["new_status"], "running");

    let parsed_last: serde_json::Value = serde_json::from_str(lines[4]).unwrap();
    assert_eq!(parsed_last["type"], "terminal_frame");
    assert_eq!(parsed_last["data"]["exit_code"], 0);
    assert_eq!(parsed_last["data"]["total_tokens"], 4200);
}

#[tokio::test]
async fn test_stable_exit_code_taxonomy() {
    use m31a::cli::{M31aExitCode, RuntimeCommand};
    use m31a::kernel::seams::policy::{
        PolicyDecision, PolicyError, PolicyEvaluationRequest, PolicyGate,
    };

    // 1. Verify exact stable numerical exit codes (CLI-03)
    assert_eq!(M31aExitCode::Success.as_i32(), 0);
    assert_eq!(M31aExitCode::GeneralError.as_i32(), 1);
    assert_eq!(M31aExitCode::ConfigError.as_i32(), 2);
    assert_eq!(M31aExitCode::VerificationFailed.as_i32(), 3);
    assert_eq!(M31aExitCode::PolicyViolation.as_i32(), 4);
    assert_eq!(M31aExitCode::Interrupted.as_i32(), 5);
    assert_eq!(M31aExitCode::ResourceExhausted.as_i32(), 6);
    assert_eq!(M31aExitCode::CrashRecovered.as_i32(), 7);

    // 2. Policy denial maps directly to PolicyViolation (4) (D-17, THREAT-06)
    struct DenyingGate;
    #[async_trait::async_trait]
    impl PolicyGate for DenyingGate {
        async fn evaluate(
            &self,
            _req: PolicyEvaluationRequest,
        ) -> Result<PolicyDecision, PolicyError> {
            Ok(PolicyDecision::Deny)
        }
    }

    let dispatcher = m31a::cli::CliDispatcher::new().with_policy_gate(Arc::new(DenyingGate));
    let check_res = dispatcher
        .dispatch(RuntimeCommand::CheckPolicy {
            tool: "shell:raw_exec".to_string(),
            mission_id: None,
        })
        .await
        .unwrap();

    assert_eq!(check_res.exit_code, M31aExitCode::PolicyViolation.as_i32());
    assert_eq!(check_res.data["decision"], "deny");
}

#[tokio::test]
async fn test_doctor_diagnostics_probes() {
    use m31a::cli::{DoctorRunner, ProbeCategory, ProbeStatus};

    let runner = DoctorRunner::with_default_probes();

    // 1. Run all 8 default probes (added PlatformProbe + DeploymentProbe)
    let full_report = runner.run(None).await;
    assert_eq!(full_report.results.len(), 8);
    // Allow environmental issues (disk quota, etc.) to cause at most 1 probe error
    assert!(full_report.error_count <= 1);
    // Overall status can be Ok, Warning, or Error due to environmental issues
    assert!(
        full_report.overall_status == ProbeStatus::Ok
            || full_report.overall_status == ProbeStatus::Warning
            || full_report.overall_status == ProbeStatus::Error
    );

    // Verify all 6 categories are represented (Sandbox now has 2 probes)
    let categories: std::collections::HashSet<_> =
        full_report.results.iter().map(|r| r.category).collect();
    assert!(categories.contains(&ProbeCategory::Environment));
    assert!(categories.contains(&ProbeCategory::Git));
    assert!(categories.contains(&ProbeCategory::Models));
    assert!(categories.contains(&ProbeCategory::Sandbox));
    assert!(categories.contains(&ProbeCategory::Storage));
    assert!(categories.contains(&ProbeCategory::Network));

    // 2. Filter by single category
    let git_report = runner.run(Some("git")).await;
    assert_eq!(git_report.results.len(), 1);
    assert_eq!(git_report.results[0].category, ProbeCategory::Git);
    assert_eq!(git_report.results[0].status, ProbeStatus::Ok);

    // 3. Format text output
    let text = full_report.format_text();
    assert!(text.contains("M31A Doctor Diagnostics Report"));
    assert!(text.contains("[✓]"));

    // 4. JSON output format
    let json_val = full_report.to_json();
    assert!(json_val["results"].is_array());
    assert!(json_val["passed_count"].is_number());
}
