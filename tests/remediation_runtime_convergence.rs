//! Architectural verification of runtime convergence & remediation invariants.

use std::sync::Arc;
use tempfile::tempdir;

use m31a::capability::family::CapabilityFamily;
use m31a::capability::instance::CapabilityInstance;
use m31a::capability::permissions::CapabilityPermissions;
use m31a::capability::providers::LocalTerminalProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::eval::scenarios::{EvalScenario, ScenarioA};
use m31a::git::hosting::{GitHubCliHostingProvider, RepoHostingProvider};
use m31a::repo::lsp::{LspBackend, LspService};
use m31a::state_machine::agent::AgentRole;
use m31a::tools::definition::{ToolExecutionContext, TypedTool};
use m31a::tools::registry::ToolRegistry;
use m31a::tools::terminal::*;

#[tokio::test]
async fn test_terminal_model_facing_tools() {
    let tmp = tempdir().unwrap();
    let reg = Arc::new(CapabilityRegistry::new());
    let terminal_provider = Arc::new(LocalTerminalProvider::new());
    reg.register_terminal(terminal_provider);
    reg.register_instance(CapabilityInstance::new(
        "terminal.local",
        "Local Interactive Terminal",
        "1.0.0",
        CapabilityFamily::Terminal,
        "local_terminal",
        CapabilityPermissions::full_access(),
    ));

    let ctx = ToolExecutionContext::new(
        reg.clone(),
        tmp.path().to_path_buf(),
        tokio_util::sync::CancellationToken::new(),
    );

    let session_id = "test-session-1".to_string();

    // 1. Fail-closed: writing input to unstarted session must fail with NotFound
    let write_tool = TerminalWriteInputTool;
    let write_res = write_tool
        .execute(
            &ctx,
            TerminalWriteInputInput {
                session_id: session_id.clone(),
                input: "echo test\n".to_string(),
            },
        )
        .await;
    assert!(
        write_res.is_err(),
        "Writing to unstarted session must fail closed"
    );

    // 2. Start session explicitly
    let start_tool = TerminalStartSessionTool;
    let start_res = start_tool
        .execute(
            &ctx,
            TerminalStartSessionInput {
                session_id: session_id.clone(),
                command: Some("sh".to_string()),
                args: None,
                cwd: Some(tmp.path().display().to_string()),
                env: None,
                cols: Some(80),
                rows: Some(24),
            },
        )
        .await
        .expect("start_session should succeed");
    assert!(start_res.started);

    // 3. Status check
    let status_tool = TerminalSessionStatusTool;
    let status_res = status_tool
        .execute(
            &ctx,
            TerminalSessionStatusInput {
                session_id: session_id.clone(),
            },
        )
        .await
        .expect("session_status should succeed");
    assert!(status_res.is_alive);
    assert_eq!(status_res.cols, 80);
    assert_eq!(status_res.rows, 24);

    // 4. Resize
    let resize_tool = TerminalResizeTool;
    let resize_res = resize_tool
        .execute(
            &ctx,
            TerminalResizeInput {
                session_id: session_id.clone(),
                cols: 120,
                rows: 40,
            },
        )
        .await
        .expect("resize should succeed");
    assert!(resize_res.resized);
    assert_eq!(resize_res.cols, 120);

    // 5. Write and read
    let write_res = write_tool
        .execute(
            &ctx,
            TerminalWriteInputInput {
                session_id: session_id.clone(),
                input: "echo M31A_TERMINAL_TEST\n".to_string(),
            },
        )
        .await
        .expect("write_input should succeed");
    assert!(write_res.bytes_written > 0);

    let read_tool = TerminalReadStreamTool;
    let read_res = read_tool
        .execute(
            &ctx,
            TerminalReadStreamInput {
                session_id: session_id.clone(),
                timeout_ms: Some(500),
            },
        )
        .await
        .expect("read_stream should succeed");
    assert!(
        read_res.output.contains("M31A_TERMINAL_TEST"),
        "Output should contain echoed text: got '{}'",
        read_res.output
    );

    // 6. Terminate session
    let term_tool = TerminalTerminateSessionTool;
    let term_res = term_tool
        .execute(
            &ctx,
            TerminalTerminateSessionInput {
                session_id: session_id.clone(),
            },
        )
        .await
        .expect("terminate_session should succeed");
    assert!(term_res.terminated);

    // Status after termination must be NotFound (fail-closed, cleaned up)
    let post_status = status_tool
        .execute(
            &ctx,
            TerminalSessionStatusInput {
                session_id: session_id.clone(),
            },
        )
        .await;
    assert!(
        post_status.is_err(),
        "Terminated session should not be found"
    );
}

#[tokio::test]
async fn test_lsp_backend_dispatch_and_degraded_reporting() {
    let tmp = tempdir().unwrap();
    let lsp = LspService::new(tmp.path());

    // 1. Detect backend for empty dir
    let backend = lsp.detect_backend().await;
    assert_eq!(backend, LspBackend::FallbackSyntactic);

    // 2. Query definitions with syntactic fallback
    let def_res = lsp
        .goto_definition("test.rs", 1, 1, Some("my_func"))
        .await
        .expect("goto_definition should return Ok result");
    assert_eq!(def_res.backend_used, LspBackend::FallbackSyntactic);
    assert!(def_res.fallback_used);
    assert!(def_res.degraded_reason.is_some());

    // 3. Query references
    let ref_res = lsp
        .find_references("test.rs", 1, 1, Some("my_func"))
        .await
        .expect("find_references should return Ok result");
    assert_eq!(ref_res.backend_used, LspBackend::FallbackSyntactic);
    assert!(ref_res.fallback_used);

    // 4. Query hover
    let hover_res = lsp
        .hover("test.rs", 1, 1, Some("my_func"))
        .await
        .expect("hover should return Ok result");
    assert_eq!(hover_res.backend_used, LspBackend::FallbackSyntactic);
    assert!(hover_res.fallback_used);

    // 5. Query symbols
    let sym_res = lsp
        .workspace_symbols("my_func")
        .await
        .expect("workspace_symbols should return Ok result");
    assert_eq!(sym_res.backend_used, LspBackend::FallbackSyntactic);
    assert!(sym_res.fallback_used);
}

#[tokio::test]
async fn test_hosting_provider_fails_closed_without_mock_fallback() {
    let tmp = tempdir().unwrap();
    let provider = GitHubCliHostingProvider::new(tmp.path());

    // Pull request operations must fail closed with GitError, never mock success
    let pr_res = provider
        .create_pull_request("test-branch", "main", "Test PR", "Description")
        .await;
    assert!(
        pr_res.is_err(),
        "GitHubCliHostingProvider must fail closed when gh is not functional"
    );

    let status_res = provider.get_pull_request_status(999_999_999).await;
    assert!(
        status_res.is_err(),
        "GitHubCliHostingProvider status must fail closed with GitError, never mock success"
    );
}

#[tokio::test]
async fn test_offline_eval_telemetry_is_truthful() {
    let scenario = ScenarioA;
    let res = scenario.run().await.expect("Scenario A run should succeed");
    assert_eq!(
        res.tokens_used, 0,
        "Offline fixture scenarios must report 0 tokens used without LLM calls"
    );
    assert_eq!(
        res.cost_usd, 0.0,
        "Offline fixture scenarios must report 0.0 cost without LLM calls"
    );
}

#[test]
fn test_tool_registry_extended_tools_includes_terminal() {
    let mut registry = ToolRegistry::new();
    registry.register_extended_tools();
    assert!(registry.get("terminal_start_session").is_some());
    assert!(registry.get("terminal_write_input").is_some());
    assert!(registry.get("terminal_read_stream").is_some());
    assert!(registry.get("terminal_resize").is_some());
    assert!(registry.get("terminal_session_status").is_some());
    assert!(registry.get("terminal_terminate_session").is_some());
}

#[test]
fn test_tool_authority_scope_role_rebinding() {
    let reg = Arc::new(CapabilityRegistry::new());
    let mut tool_reg = ToolRegistry::new_default(reg.clone());
    tool_reg.register_extended_tools();
    let tool_reg = Arc::new(tool_reg);

    let scope_impl = m31a::tools::filter::ToolAuthorityScope::new(
        AgentRole::implementer(),
        reg.clone(),
        tool_reg.clone(),
        m31a::state_machine::AutonomyMode::Autonomous,
    );
    let impl_schemas = scope_impl.model_tool_schemas();
    let impl_names: Vec<&str> = impl_schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();
    assert!(impl_names.contains(&"write_file"));
    assert!(impl_names.contains(&"git_commit"));

    // Rebind to reviewer (read-only role envelope)
    let scope_rev = scope_impl.for_role(AgentRole::reviewer());
    let rev_schemas = scope_rev.model_tool_schemas();
    let rev_names: Vec<&str> = rev_schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();

    // Reviewer MUST NOT receive mutating tools
    assert!(
        !rev_names.contains(&"write_file"),
        "Reviewer role must not receive write_file schema"
    );
    assert!(
        !rev_names.contains(&"git_commit"),
        "Reviewer role must not receive git_commit schema"
    );
}

#[tokio::test]
async fn test_adapt_strategy_tool_is_proposal_only() {
    let tmp = tempdir().unwrap();
    let reg = Arc::new(CapabilityRegistry::new());
    let ctx = ToolExecutionContext::new(
        reg,
        tmp.path().to_path_buf(),
        tokio_util::sync::CancellationToken::new(),
    );

    let tool = m31a::tools::definition::AdaptStrategyTool;
    let res = tool
        .execute(
            &ctx,
            m31a::tools::definition::AdaptStrategyInput {
                strategy: "investigate_then_act".to_string(),
                reason: "Prior task encountered flaky tests; deeper diagnosis needed".to_string(),
                tasks_to_supersede: vec!["task-123".to_string()],
                new_tasks: vec!["Investigate flakiness".to_string()],
            },
        )
        .await
        .expect("AdaptStrategyTool proposal execution should succeed");

    assert!(res.adapted);
    assert_eq!(res.new_strategy, "investigate_then_act");
    assert_eq!(res.tasks_superseded, 1);
    assert_eq!(res.new_tasks_added, 1);
}
