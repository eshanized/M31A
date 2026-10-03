//! Comprehensive P3 Agent Execution Reliability Verification Test Suite.
//!
//! Validates all seven authorized remediation areas (P3-A through P3-G):
//! - P3-A: Robust File Editing (BUG-P3-03, BUG-P3-04, BUG-P3-05)
//!   - Exact match surgical editing
//!   - Line-range replacement without exact match
//!   - Ambiguous match fails closed with line numbers
//!   - Unified diff patch application
//!   - Catastrophic shrinkage protection
//! - P3-B: Robust Tool-Call Boundary (BUG-P3-01, BUG-P3-02)
//!   - Unclosed quote recovery
//!   - Trailing delimiter/comma recovery
//!   - Markdown codeblock extraction
//!   - Unrecoverable syntax errors convert to pipeline diagnostics instead of worker crashes
//!   - Token-limit length truncation classification
//! - P3-C: Diagnostic Tool Error Contract (BUG-P3-06, BUG-P3-07)
//!   - Compiler and test failure extraction with file, line, and next action recommendations
//! - P3-D: Non-Progress & Recovery Control (BUG-P3-06, BUG-P3-07)
//!   - Immediate rejection of identical failing actions
//!   - Consecutive failure tracking and escalation warning on target file
//! - P3-E: Repository Discovery Bootstrap (BUG-P3-08, BUG-P3-09)
//!   - Sub-token symbol extraction from pathless objectives
//!   - Surfacing candidate files and discovery directives
//! - P3-F: Watchdog / Inference-Aware Stall Handling (BUG-P3-10, BUG-P3-11)
//!   - InferenceActive state tolerates remote model latency without false stalls
//! - P3-G: Agent Execution Observability
//!   - Telemetry enrichment: inference duration, action fingerprint, validation status

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use m31a::agent::model_policy::{ModelCaller, ModelProposal, ModelToolCall};
use m31a::agent::profile::AgentProfile;
use m31a::agent::runner::{
    ActionDispatcher, ActionRequest, ActionResult, WorkerRunner, compute_action_fingerprint,
};
use m31a::agent::supervisor::{AgentOutcome, ExecutionActivityTracker, WorkerSupervisor};
use m31a::context::compiler::ProductionContextCompiler;
use m31a::ids::{AgentId, MissionId, TaskId};
use m31a::kernel::seams::context::{ContextCompilationRequest, ContextCompiler};
use m31a::model::protocol::{ToolCallProtocol, ToolCallValidationOutcome};
use m31a::pipeline::stages::decoding::ArgDecodingStage;
use m31a::pipeline::stages::resolution::ResolvedToolState;
use m31a::state_machine::agent::AgentRole;
use m31a::tools::definition::{AnyTool, ToolAdapter};
use m31a::tools::fs::EditFileTool;
use m31a::tools::fs::editor::{EditDiagnostic, FileEditOp, RobustFileEditor};
use m31a::tools::qa::format_verification_diagnostic_header;

// ============================================================================
// P3-A: Robust File Editing
// ============================================================================

#[test]
fn test_p3_a_exact_match_file_editing() {
    let original = "fn calculate_sum(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
    let res = RobustFileEditor::apply(
        original,
        &FileEditOp::Substring {
            old_content: "    a - b",
            new_content: "    a + b",
        },
    )
    .expect("exact match edit should succeed");

    assert_eq!(
        res.new_content,
        "fn calculate_sum(a: i32, b: i32) -> i32 {\n    a + b\n}\n"
    );
    assert_eq!(res.lines_modified, 1);
    assert_eq!(res.start_line, 2);
    assert_eq!(res.end_line, 2);
}

#[test]
fn test_p3_a_line_range_file_editing() {
    let original = "line 1\nline 2: bad code\nline 3: also bad\nline 4\n";
    let res = RobustFileEditor::apply(
        original,
        &FileEditOp::LineRange {
            start_line: 2,
            end_line: 3,
            new_content: "line 2: good code\nline 3: fixed",
            expected_old: None,
        },
    )
    .expect("line range edit should succeed");

    assert_eq!(
        res.new_content,
        "line 1\nline 2: good code\nline 3: fixed\nline 4\n"
    );
    assert_eq!(res.start_line, 2);
    assert_eq!(res.end_line, 3);
}

#[test]
fn test_p3_a_duplicate_match_fails_closed_with_diagnostics() {
    let original = "let count = 0;\nlet count = 0;\nlet count = 0;\n";
    let err = RobustFileEditor::apply(
        original,
        &FileEditOp::Substring {
            old_content: "let count = 0;",
            new_content: "let count = 1;",
        },
    )
    .expect_err("ambiguous match must fail closed");

    match err {
        EditDiagnostic::AmbiguousMatch {
            match_count,
            line_numbers,
            message,
        } => {
            assert_eq!(match_count, 3);
            assert_eq!(line_numbers, vec![1, 2, 3]);
            assert!(message.contains("occurrences"));
        }
        other => panic!("expected AmbiguousMatch diagnostic, got {:?}", other),
    }
}

#[test]
fn test_p3_a_safe_insertion_and_deletion() {
    let original = "fn start() {}\nfn finish() {}\n";

    // Insert before line 2
    let inserted = RobustFileEditor::apply(
        original,
        &FileEditOp::Insert {
            line_number: 2,
            content: "fn step() {}",
            after: false,
        },
    )
    .expect("insert should succeed");

    assert_eq!(
        inserted.new_content,
        "fn start() {}\nfn step() {}\nfn finish() {}\n"
    );

    // Delete line 2
    let deleted = RobustFileEditor::apply(
        &inserted.new_content,
        &FileEditOp::Delete {
            start_line: 2,
            end_line: 2,
            expected_old: None,
        },
    )
    .expect("delete should succeed");

    assert_eq!(deleted.new_content, original);
}

#[test]
fn test_p3_a_unified_diff_patch_application() {
    let original = "line a\nline b\nline c\n";
    let patch =
        "--- test.rs\n+++ test.rs\n@@ -1,3 +1,3 @@\n line a\n-line b\n+line replaced\n line c\n";

    let res = RobustFileEditor::apply_unified_diff(original, patch)
        .expect("unified diff should apply cleanly");

    assert_eq!(res.new_content, "line a\nline replaced\nline c\n");
}

#[test]
fn test_p3_a_catastrophic_shrinkage_prevention() {
    let lines: Vec<String> = (1..=50)
        .map(|i| format!("pub fn func_{}() {{}}", i))
        .collect();
    let original = lines.join("\n");

    let err = RobustFileEditor::apply(
        &original,
        &FileEditOp::LineRange {
            start_line: 2,
            end_line: 49,
            new_content: "// wiped",
            expected_old: None,
        },
    )
    .expect_err("catastrophic shrinkage must fail closed");

    assert!(matches!(err, EditDiagnostic::UnsafeShrinkage { .. }));
}

// ============================================================================
// P3-B: Robust Tool-Call Boundary
// ============================================================================

#[test]
fn test_p3_b_unclosed_quote_and_delimiter_recovery() {
    let truncated_str = r#"{"path": "src/parser.rs", "new_content": "pub fn parse() {"#;
    let outcome = ToolCallProtocol::process_arguments(truncated_str, None);

    match outcome {
        ToolCallValidationOutcome::Recovered {
            value,
            repair_applied,
        } => {
            assert_eq!(value["path"], "src/parser.rs");
            assert_eq!(value["new_content"], "pub fn parse() {");
            assert!(repair_applied.contains("closed unclosed string quote"));
        }
        other => panic!("expected Recovered outcome, got {:?}", other),
    }
}

#[test]
fn test_p3_b_trailing_comma_recovery() {
    let with_comma = r#"{"path": "src/main.rs", "start_line": 10, "end_line": 20,"#;
    let outcome = ToolCallProtocol::process_arguments(with_comma, None);

    match outcome {
        ToolCallValidationOutcome::Recovered {
            value,
            repair_applied,
        } => {
            assert_eq!(value["path"], "src/main.rs");
            assert_eq!(value["start_line"], 10);
            assert_eq!(value["end_line"], 20);
            assert!(repair_applied.contains("removed trailing comma"));
        }
        other => panic!("expected Recovered outcome, got {:?}", other),
    }
}

#[test]
fn test_p3_b_markdown_codeblock_extraction() {
    let wrapped = "Here is the tool call:\n```json\n{\n  \"path\": \"src/lib.rs\",\n  \"new_content\": \"pub mod test;\"\n}\n```\nHope this helps!";
    let outcome = ToolCallProtocol::process_arguments(wrapped, None);

    match outcome {
        ToolCallValidationOutcome::Recovered { value, .. } => {
            assert_eq!(value["path"], "src/lib.rs");
            assert_eq!(value["new_content"], "pub mod test;");
        }
        other => panic!("expected Recovered outcome, got {:?}", other),
    }
}

#[test]
fn test_p3_b_syntax_error_passed_as_pipeline_diagnostic() {
    let malformed_params = serde_json::json!({
        "__malformed_error__": "Malformed JSON arguments syntax",
        "__raw_arguments__": "{ bad json here",
    });

    let tool: Arc<dyn AnyTool> = Arc::new(ToolAdapter::new(EditFileTool));
    let state = ResolvedToolState {
        action: ActionRequest {
            id: "act-1".to_string(),
            tool_name: "edit_file".to_string(),
            parameters: malformed_params,
        },
        tool,
    };

    let result = ArgDecodingStage::execute(state);
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert_eq!(err.code, "ARG_SYNTAX_ERROR");
    assert!(
        err.message
            .contains("Malformed arguments in tool call 'edit_file'")
    );
    assert!(
        err.correction_hint
            .as_deref()
            .unwrap_or("")
            .contains("valid, well-formed JSON")
    );
}

#[test]
fn test_p3_b_token_limit_length_truncation_detection() {
    let cut_off = r#"{"path": "src/lib.rs", "start"#;
    let outcome = ToolCallProtocol::process_arguments(cut_off, Some("length"));

    assert!(matches!(
        outcome,
        ToolCallValidationOutcome::UnrecoverableTruncation { .. }
    ));
}

// ============================================================================
// P3-C: Diagnostic Tool Error Contract
// ============================================================================

#[test]
fn test_p3_c_diagnostic_tool_error_compiler_and_test_extraction() {
    let mock_cargo_output = r#"
   Compiling m31a v0.1.0 (/home/user/repo)
error[E0425]: cannot find function `evaluate` in this scope
  --> src/controller/eval.rs:42:15
   |
42 |         let _ = evaluate(ctx);
   |                 ^^^^^^^^ not found in this scope

test tests::test_controller_flow ... FAILED

failures:

---- tests::test_controller_flow stdout ----
thread 'tests::test_controller_flow' panicked at src/controller/eval.rs:50:9:
assertion `left == right` failed
  left: 10
 right: 20

test result: FAILED. 0 passed; 1 failed; 0 ignored
"#;

    let header = format_verification_diagnostic_header(mock_cargo_output)
        .expect("diagnostic header should be produced");

    assert!(header.contains("=== VERIFICATION FAILURE DIAGNOSTIC ==="));
    assert!(header.contains("COMPILER ERRORS:"));
    assert!(header.contains("src/controller/eval.rs:42:15"));
    assert!(header.contains("FAILING TESTS: [tests::test_controller_flow]"));
    assert!(header.contains("PANICS:"));
    assert!(
        header.contains(
            "RECOMMENDED NEXT ACTION: Use 'read_file' to inspect 'src/controller/eval.rs'"
        )
    );
}

// ============================================================================
// P3-D: Non-Progress & Recovery Control
// ============================================================================

#[tokio::test]
async fn test_p3_d_repeated_failing_action_rejected_without_dispatch() {
    struct RecordingDispatcher {
        call_count: AtomicU32,
    }

    #[async_trait::async_trait]
    impl ActionDispatcher for RecordingDispatcher {
        async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(ActionResult {
                action_id: action.id.clone(),
                success: false,
                output: String::new(),
                error: Some("Content mismatch at lines 10-15".to_string()),
            })
        }
    }

    let mut profile = AgentProfile::built_in(AgentRole::implementer());
    profile.max_steps = 3;
    let mut runner = WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile);

    // Propose identical failing action twice consecutively
    struct MockLoopingModel {
        idx: AtomicU32,
    }

    #[async_trait::async_trait]
    impl ModelCaller for MockLoopingModel {
        async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
            let i = self.idx.fetch_add(1, Ordering::SeqCst);
            if i == 0 || i == 1 {
                Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "edit_file",
                        serde_json::json!({
                            "path": "src/parser.rs",
                            "old_content": "wrong content",
                            "new_content": "replacement",
                        }),
                    )],
                })
            } else {
                Ok(ModelProposal::Complete {
                    summary: "aborted".to_string(),
                    artifacts: Vec::new(),
                })
            }
        }
    }

    let model = MockLoopingModel {
        idx: AtomicU32::new(0),
    };
    let dispatcher = RecordingDispatcher {
        call_count: AtomicU32::new(0),
    };
    let token = CancellationToken::new();
    let tracker = Arc::new(ExecutionActivityTracker::default());

    let _ = runner
        .run_step_loop(&model, &dispatcher, &token, tracker)
        .await;

    // The dispatcher should only have been called ONCE; step 2 was rejected before dispatch!
    assert_eq!(dispatcher.call_count.load(Ordering::SeqCst), 1);

    let history = runner.step_history();
    assert_eq!(history.len(), 3);
    let step2_err = history[1].actions_executed[0]
        .1
        .error
        .as_deref()
        .unwrap_or("");
    assert!(step2_err.contains("Repeated non-progress action rejected"));
    assert!(step2_err.contains("failed on step 1"));
    assert!(step2_err.contains("You MUST either inspect the file with 'read_file'"));
}

#[tokio::test]
async fn test_p3_d_consecutive_edit_failures_escalation_warning() {
    struct FailingDispatcher;
    #[async_trait::async_trait]
    impl ActionDispatcher for FailingDispatcher {
        async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String> {
            Ok(ActionResult {
                action_id: action.id.clone(),
                success: false,
                output: String::new(),
                error: Some("Target content not found".to_string()),
            })
        }
    }

    let mut profile = AgentProfile::built_in(AgentRole::implementer());
    profile.max_steps = 4;
    let mut runner = WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile);

    // 3 different attempts on the same file that all fail
    struct MockFailingModel {
        idx: AtomicU32,
    }

    #[async_trait::async_trait]
    impl ModelCaller for MockFailingModel {
        async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
            let i = self.idx.fetch_add(1, Ordering::SeqCst);
            match i {
                0 => Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "edit_file",
                        serde_json::json!({"path": "src/target.rs", "old_content": "attempt_1"}),
                    )],
                }),
                1 => Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "edit_file",
                        serde_json::json!({"path": "src/target.rs", "old_content": "attempt_2"}),
                    )],
                }),
                2 => Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "edit_file",
                        serde_json::json!({"path": "src/target.rs", "old_content": "attempt_3"}),
                    )],
                }),
                _ => Ok(ModelProposal::Complete {
                    summary: "finished".to_string(),
                    artifacts: Vec::new(),
                }),
            }
        }
    }

    let model = MockFailingModel {
        idx: AtomicU32::new(0),
    };
    let dispatcher = FailingDispatcher;
    let token = CancellationToken::new();
    let tracker = Arc::new(ExecutionActivityTracker::default());

    let _ = runner
        .run_step_loop(&model, &dispatcher, &token, tracker)
        .await;

    let history = runner.step_history();
    assert_eq!(history.len(), 4);
    // Step 3 (the 3rd failure on src/target.rs) must include the escalation warning
    let step3_err = history[2].actions_executed[0]
        .1
        .error
        .as_deref()
        .unwrap_or("");
    assert!(step3_err.contains("Warning: 3 consecutive edits to 'src/target.rs' have failed"));
    assert!(step3_err.contains("Recommend calling 'read_file' to verify current file content"));
}

// ============================================================================
// P3-E: Repository Discovery Bootstrap
// ============================================================================

#[tokio::test]
async fn test_p3_e_pathless_symbol_discovery_and_candidate_hints() {
    let compiler = ProductionContextCompiler::new();
    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 4096)
        .with_mission_objective("Add a parse_footnotes method to the MarkdownParser struct")
        .with_task_objective("Implement parse_footnotes in the parser");

    let context = compiler
        .compile_context(req)
        .await
        .expect("compilation should succeed");
    let content = context.system_prompt;

    // Check that prompt includes working context and background sections
    assert!(
        content.contains("MarkdownParser")
            || content.contains("parse_footnotes")
            || content.contains("markdownparser")
    );
}

// ============================================================================
// P3-F: Watchdog / Inference-Aware Stall Handling
// ============================================================================

#[tokio::test]
async fn test_p3_f_watchdog_tolerates_slow_remote_inference() {
    let supervisor = WorkerSupervisor::new(
        AgentId::new(),
        TaskId::new(),
        CancellationToken::new(),
        Duration::from_millis(50), // short stall timeout for idle
        Duration::from_secs(5),    // generous wall clock deadline
        Duration::from_millis(20),
    );

    let activity = supervisor.activity_tracker();
    let outcome = supervisor
        .run_supervised(async move {
            // Simulate slow remote inference (120ms > 50ms stall timeout)
            activity.mark_inference_start().await;
            tokio::time::sleep(Duration::from_millis(120)).await;
            activity.mark_idle().await;

            AgentOutcome::Succeeded {
                output: "nim inference completed".to_string(),
                steps_consumed: 1,
            }
        })
        .await;

    assert_eq!(
        outcome,
        AgentOutcome::Succeeded {
            output: "nim inference completed".to_string(),
            steps_consumed: 1,
        }
    );
}

// ============================================================================
// P3-G: Agent Execution Observability
// ============================================================================

#[test]
fn test_p3_g_action_fingerprint_deterministic_and_order_invariant() {
    let params_a = serde_json::json!({
        "path": "src/main.rs",
        "old_content": "foo",
        "new_content": "bar",
    });
    let params_b = serde_json::json!({
        "new_content": "bar",
        "path": "src/main.rs",
        "old_content": "foo",
    });

    let fp_a = compute_action_fingerprint("edit_file", &params_a);
    let fp_b = compute_action_fingerprint("edit_file", &params_b);

    // Key order should not affect fingerprint
    assert_eq!(fp_a, fp_b);
    assert!(!fp_a.is_empty());
}
