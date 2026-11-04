use m31a_code_mode::runtime::{CodeModeRuntime, RuntimeExecutionLimits};
use m31a_core::SandboxPolicy;
use m31a_tools::ToolRegistry;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn test_supervisor_infinite_loop_without_stdout_terminates_at_deadline() {
    let limits = RuntimeExecutionLimits {
        timeout_ms: 1000, // 1 second deadline
        max_tool_calls: 10,
        max_output_bytes: 50_000,
    };
    let runtime = CodeModeRuntime::with_limits(limits);
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    let start = std::time::Instant::now();
    // Infinite loop that never prints to stdout
    let script = "while (true) {}";

    let outcome = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            1000,
        )
        .await
        .expect("Runtime execution should return structured outcome");

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(4),
        "Execution should have returned close to deadline, took {:?}",
        elapsed
    );
    assert!(
        !outcome.yielded,
        "Outcome must not be marked yielded on timeout"
    );
    assert_eq!(
        outcome.status,
        m31a_code_mode::ProcessLifecycleState::TimedOut
    );
    assert!(
        outcome.output.contains("[TIMEOUT"),
        "Output should indicate execution exceeded deadline: {}",
        outcome.output
    );
}

#[tokio::test]
async fn test_supervisor_infinite_loop_with_continuous_stdout_does_not_deadlock() {
    let limits = RuntimeExecutionLimits {
        timeout_ms: 1000,
        max_tool_calls: 10,
        max_output_bytes: 5_000, // strict 5KB limit
    };
    let runtime = CodeModeRuntime::with_limits(limits);
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    let start = std::time::Instant::now();
    // Continuous noisy stdout
    let script = "while (true) { text('SPAM_LINE_1234567890'); }";

    let outcome = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            1000,
        )
        .await
        .expect("Runtime execution should handle chatty isolate");

    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(4));
    assert!(!outcome.yielded);
    assert_eq!(
        outcome.status,
        m31a_code_mode::ProcessLifecycleState::TimedOut
    );
    assert!(
        outcome.output.len() <= 10_000,
        "Output size must remain bounded: len is {}",
        outcome.output.len()
    );
}

#[tokio::test]
async fn test_supervisor_infinite_loop_with_continuous_stderr_does_not_deadlock() {
    let limits = RuntimeExecutionLimits {
        timeout_ms: 1000,
        max_tool_calls: 10,
        max_output_bytes: 5_000,
    };
    let runtime = CodeModeRuntime::with_limits(limits);
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    let start = std::time::Instant::now();
    // Continuous noisy stderr
    let script = "while (true) { console.error('STDERR_SPAM_LINE'); }";

    let outcome = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            1000,
        )
        .await
        .expect("Runtime execution should handle noisy stderr without pipe deadlock");

    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(4));
    assert!(!outcome.yielded);
    assert_eq!(
        outcome.status,
        m31a_code_mode::ProcessLifecycleState::TimedOut
    );
}

#[tokio::test]
async fn test_supervisor_tool_call_budget_exhaustion() {
    let limits = RuntimeExecutionLimits {
        timeout_ms: 5000,
        max_tool_calls: 3, // only allow 3 calls
        max_output_bytes: 50_000,
    };
    let runtime = CodeModeRuntime::with_limits(limits);
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    let script = r#"
        for (let i = 0; i < 6; i++) {
            try {
                await tools.list_directory({ path: "." });
            } catch (e) {
                text("Caught: " + e.message);
            }
        }
    "#;

    let outcome = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            5000,
        )
        .await
        .expect("Execution must complete without crashing host");

    assert!(
        outcome.output.contains("Tool call budget exceeded"),
        "Output must indicate tool budget exhaustion: {}",
        outcome.output
    );
}

#[tokio::test]
async fn test_supervisor_isolate_crash_returns_clean_outcome() {
    let runtime = CodeModeRuntime::new();
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    // Script throws unhandled exception
    let script = "throw new Error('Fatal isolate test crash');";

    let outcome = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            3000,
        )
        .await
        .expect("Crashing script must not panic runtime");

    assert!(
        outcome.output.contains("Fatal isolate test crash"),
        "Error message should be reflected in output: {}",
        outcome.output
    );
}

#[tokio::test]
async fn test_supervisor_oversized_ipc_line_fails_closed() {
    let runtime = CodeModeRuntime::new();
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    // Script writes a single line exceeding 65KB boundary
    let script = r#"
        const huge = 'A'.repeat(80000);
        process.stdout.write(JSON.stringify({ type: 'text', value: huge }) + '\n');
    "#;

    let outcome = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            5000,
        )
        .await
        .expect("Runtime execution should return outcome");

    assert!(
        outcome.output.contains("IPC line size exceeded")
            || outcome.status == m31a_code_mode::ProcessLifecycleState::Failed,
        "Supervisor must catch oversized IPC line and fail closed: output: {}",
        outcome.output
    );
}

#[tokio::test]
async fn test_supervisor_malformed_json_rpc_payload() {
    let runtime = CodeModeRuntime::new();
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    // Script writes invalid JSON to stdout
    let script = r#"
        process.stdout.write("{ corrupted json payload\n");
        text("After corruption");
    "#;

    let outcome = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            5000,
        )
        .await
        .expect("Runtime execution should handle malformed line");

    assert!(
        outcome.output.contains("corrupted json payload")
            || outcome.output.contains("After corruption"),
        "Supervisor should capture child text: {}",
        outcome.output
    );
}

#[tokio::test]
async fn test_supervisor_excessive_notifications_bounded() {
    let runtime = CodeModeRuntime::new();
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    // Script floods 300 notifications
    let script = r#"
        for (let i = 0; i < 300; i++) {
            notify("Notification " + i);
        }
    "#;

    let outcome = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            5000,
        )
        .await
        .expect("Runtime should succeed");

    assert!(
        outcome.notifications.len() <= 100,
        "Notifications must be bounded to <= 100, got: {}",
        outcome.notifications.len()
    );
}

#[tokio::test]
async fn test_supervisor_untrusted_workspace_fails_closed() {
    let runtime = CodeModeRuntime::new();
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    // Do NOT trust temp.path()

    let script = "text('Should not run');";
    let res = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            3000,
        )
        .await;

    assert!(res.is_err(), "Untrusted workspace must fail closed");
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("untrusted"),
        "Error should mention untrusted: {}",
        err
    );
}

#[tokio::test]
async fn test_supervisor_denied_workspace_fails_closed() {
    let runtime = CodeModeRuntime::new();
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::Denied)
        .unwrap();

    let script = "text('Should not run');";
    let res = runtime
        .run(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            3000,
        )
        .await;

    assert!(res.is_err(), "Denied workspace must fail closed");
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("Denied"),
        "Error should mention Denied: {}",
        err
    );
}

#[tokio::test]
async fn test_supervisor_timed_out_cell_is_terminal_and_not_yielded() {
    use m31a_code_mode::orchestrator::CodeModeOrchestrator;

    let limits = RuntimeExecutionLimits {
        timeout_ms: 200,
        max_tool_calls: 10,
        max_output_bytes: 10_000,
    };
    let runtime = CodeModeRuntime::with_limits(limits);
    let orch = CodeModeOrchestrator::with_runtime(runtime);
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();

    orch.runtime()
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    let script = "while (true) {}";
    let output = orch
        .execute_batch(
            script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
        )
        .await
        .expect("execute_batch should return output");

    // Must NOT claim "Script running with cell ID" because timed out cell is terminal, not yielded
    assert!(
        !output.contains("Script running with cell ID"),
        "Timed-out cell must not be reported as running. Got: {output}"
    );
    assert!(
        output.contains("[TIMEOUT"),
        "Timed-out cell output should mention TIMEOUT: {output}"
    );
}

#[tokio::test]
async fn test_supervisor_cooperative_yield_vs_terminal_state() {
    let runtime = CodeModeRuntime::new();
    let registry = ToolRegistry::new();
    let temp = tempdir().unwrap();

    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(temp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();

    // Cooperative yield script
    let yield_script = r#"
        text("Step 1 done");
        yield_control();
        text("Step 2 done");
    "#;

    let outcome = runtime
        .run(
            yield_script,
            &registry,
            temp.path(),
            SandboxPolicy::DangerFullAccess,
            5000,
        )
        .await
        .unwrap();

    assert!(outcome.yielded, "Cooperative yield must set yielded = true");
    assert_eq!(
        outcome.status,
        m31a_code_mode::ProcessLifecycleState::Yielded
    );
    assert!(
        !outcome.status.is_terminal(),
        "Yielded is not a terminal state"
    );
}
