//! Integration tests for Code Mode runtime and IPC channels.
//!
//! Validates ambient type generation, isolate process execution, and programmatic tool invocation.

use m31a_code_mode::{
    AmbientDtsGenerator, AmbientToolSpec, CellActor, CellExecutionBudget, CodeModeOrchestrator,
    CodeModeRuntime, render_json_schema_to_typescript,
};
use m31a_core::SandboxPolicy;
use m31a_tools::ToolRegistry;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn test_code_mode_runtime_kv_and_output() {
    let tmp = tempdir().unwrap();
    let runtime = CodeModeRuntime::new();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(tmp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();
    let registry = ToolRegistry::new();

    let script = r#"
        await store("my_key", { count: 42 });
        text("Hello from Code Mode!");
    "#;

    let outcome = runtime
        .run(
            script,
            &registry,
            tmp.path(),
            SandboxPolicy::DangerFullAccess,
            1000,
        )
        .await
        .expect("runtime run");

    assert!(outcome.output.contains("Hello from Code Mode!"));

    let stored = runtime.load("my_key");
    assert_eq!(stored, Some(json!({ "count": 42 })));
}

#[tokio::test]
async fn test_code_mode_real_tool_dispatch() {
    let tmp = tempdir().unwrap();
    let runtime = CodeModeRuntime::new();
    runtime
        .trust_store()
        .lock()
        .unwrap()
        .trust(tmp.path(), m31a_trust::TrustLevel::SessionOnly)
        .unwrap();
    let registry = ToolRegistry::new();

    let test_file = tmp.path().join("output.txt");
    let script = format!(
        r#"
        await tools.write_file({{ path: "{}", content: "CODE_MODE_GENERATED" }});
        text("File written successfully.");
        "#,
        test_file.to_string_lossy()
    );

    let outcome = runtime
        .run(
            &script,
            &registry,
            tmp.path(),
            SandboxPolicy::DangerFullAccess,
            2000,
        )
        .await
        .expect("run tool in code mode");

    assert!(outcome.output.contains("File written successfully."));
    assert!(test_file.exists());
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "CODE_MODE_GENERATED");
}

#[tokio::test]
async fn test_cell_actor_budget_and_execution() {
    let actor = CellActor::new(
        1,
        CellExecutionBudget {
            max_execution_time_ms: 5000,
            max_tool_calls: 50,
            max_output_chars: 10000,
        },
    );

    assert_eq!(actor.state(), m31a_code_mode::CellLifecycleState::Pending);
}

#[test]
fn test_ambient_dts_generation() {
    let tools = vec![AmbientToolSpec::new(
        "search_docs",
        "Search documentation",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        }),
    )];

    let generator = AmbientDtsGenerator::default();
    let dts = generator.generate_ambient_dts(&tools);
    assert!(dts.contains("declare namespace tools"));
    assert!(dts.contains("search_docs"));
    assert!(dts.contains("declare function yieldToUser"));
}

#[test]
fn test_render_json_schema_to_typescript() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "description": "The user's full name" },
            "age": { "type": "integer" },
            "tags": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["name"]
    });

    let ts = render_json_schema_to_typescript(&schema);
    assert!(ts.contains("name: string;"));
    assert!(ts.contains("age?: number;"));
    assert!(ts.contains("tags?: Array<string>;"));
}

#[test]
fn test_orchestrator_tool_definitions() {
    let orchestrator = CodeModeOrchestrator::new();
    let base_tools = vec![];
    let cm_tools = orchestrator.tool_definitions(&base_tools);

    assert_eq!(cm_tools.len(), 2);
    assert!(cm_tools.iter().any(|t| t.function.name == "exec"));
    assert!(cm_tools.iter().any(|t| t.function.name == "wait"));
}
