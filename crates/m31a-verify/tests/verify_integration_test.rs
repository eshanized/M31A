//! Integration tests for plan checking and verification engines.
//!
//! Exercises 8-dimension plan checking, goal-backward verification, and evaluation scope resolution.

use m31a_verify::{
    AutonomousVerifier, DiagnosticParser, DiagnosticSeverity, ProjectKind, ProjectRunner,
    SelfHealingGenerator,
};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_project_detection_all_types() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Generic
    assert_eq!(ProjectRunner::detect(root), ProjectKind::Generic);

    // Rust
    let rust_dir = root.join("rust_proj");
    fs::create_dir_all(&rust_dir).unwrap();
    fs::write(rust_dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    assert_eq!(ProjectRunner::detect(&rust_dir), ProjectKind::Rust);

    // Node/TypeScript with tsconfig
    let ts_dir = root.join("ts_proj");
    fs::create_dir_all(&ts_dir).unwrap();
    fs::write(ts_dir.join("tsconfig.json"), "{}").unwrap();
    assert_eq!(ProjectRunner::detect(&ts_dir), ProjectKind::NodeTypeScript);

    // Node with package.json
    let node_dir = root.join("node_proj");
    fs::create_dir_all(&node_dir).unwrap();
    fs::write(node_dir.join("package.json"), "{}").unwrap();
    assert_eq!(
        ProjectRunner::detect(&node_dir),
        ProjectKind::NodeTypeScript
    );

    // Python
    let py_dir = root.join("py_proj");
    fs::create_dir_all(&py_dir).unwrap();
    fs::write(py_dir.join("pyproject.toml"), "").unwrap();
    assert_eq!(ProjectRunner::detect(&py_dir), ProjectKind::Python);

    // Go
    let go_dir = root.join("go_proj");
    fs::create_dir_all(&go_dir).unwrap();
    fs::write(go_dir.join("go.mod"), "module example.com/test").unwrap();
    assert_eq!(ProjectRunner::detect(&go_dir), ProjectKind::Go);

    // Bazel
    let bazel_dir = root.join("bazel_proj");
    fs::create_dir_all(&bazel_dir).unwrap();
    fs::write(bazel_dir.join("BUILD.bazel"), "").unwrap();
    assert_eq!(ProjectRunner::detect(&bazel_dir), ProjectKind::Bazel);

    // CMake
    let cmake_dir = root.join("cmake_proj");
    fs::create_dir_all(&cmake_dir).unwrap();
    fs::write(cmake_dir.join("CMakeLists.txt"), "project(test)").unwrap();
    assert_eq!(ProjectRunner::detect(&cmake_dir), ProjectKind::CMake);

    // Gradle
    let gradle_dir = root.join("gradle_proj");
    fs::create_dir_all(&gradle_dir).unwrap();
    fs::write(gradle_dir.join("build.gradle"), "").unwrap();
    assert_eq!(ProjectRunner::detect(&gradle_dir), ProjectKind::Gradle);
}

#[test]
fn test_parse_rust_json_compiler_diagnostics() {
    let json_diagnostic = r#"{"$message_type":"diagnostic","message":"mismatched types","code":{"code":"E0308","explanation":null},"level":"error","spans":[{"file_name":"src/main.rs","byte_start":45,"byte_end":47,"line_start":4,"line_end":4,"column_start":18,"column_end":20,"is_primary":true,"text":[{"text":"    let x: usize = \"42\";","highlight_start":18,"highlight_end":20}],"label":"expected `usize`, found `&str`","suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[{"message":"expected `usize` because of return type","code":null,"level":"note","spans":[],"children":[],"rendered":null}],"rendered":"error[E0308]: mismatched types\n --> src/main.rs:4:18\n"}"#;

    let diags = DiagnosticParser::parse(json_diagnostic);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.severity, DiagnosticSeverity::Error);
    assert_eq!(d.code.as_deref(), Some("E0308"));
    assert_eq!(d.file_path, PathBuf::from("src/main.rs"));
    assert_eq!(d.line, 4);
    assert_eq!(d.column, Some(18));
    assert!(d.message.contains("mismatched types"));
    assert!(
        d.snippet
            .as_deref()
            .unwrap_or("")
            .contains("let x: usize = \"42\";")
    );
}

#[test]
fn test_parse_rust_human_compiler_diagnostics() {
    let output = r#"
error[E0425]: cannot find value `unresolved_var` in this scope
 --> src/lib.rs:12:9
  |
12|     unresolved_var + 10
  |     ^^^^^^^^^^^^^^ not found in this scope
help: a local variable with a similar name exists: `resolved_var`
error: aborting due to 1 previous error
"#;

    let diags = DiagnosticParser::parse(output);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.severity, DiagnosticSeverity::Error);
    assert_eq!(d.code.as_deref(), Some("E0425"));
    assert_eq!(d.file_path, PathBuf::from("src/lib.rs"));
    assert_eq!(d.line, 12);
    assert_eq!(d.column, Some(9));
    assert!(
        d.suggested_fix
            .as_deref()
            .unwrap_or("")
            .contains("resolved_var")
    );
}

#[test]
fn test_parse_typescript_and_python_diagnostics() {
    let ts_output = "src/services/auth.ts:42:15 - error TS2322: Type 'string' is not assignable to type 'number'.\n";
    let ts_diags = DiagnosticParser::parse(ts_output);
    assert_eq!(ts_diags.len(), 1);
    assert_eq!(ts_diags[0].file_path, PathBuf::from("src/services/auth.ts"));
    assert_eq!(ts_diags[0].line, 42);
    assert_eq!(ts_diags[0].column, Some(15));
    assert_eq!(ts_diags[0].code.as_deref(), Some("TS2322"));

    let py_output =
        "FAILED tests/test_calc.py::test_division - ZeroDivisionError: division by zero\n";
    let py_diags = DiagnosticParser::parse(py_output);
    assert_eq!(py_diags.len(), 1);
    assert_eq!(py_diags[0].file_path, PathBuf::from("tests/test_calc.py"));
    assert_eq!(py_diags[0].code.as_deref(), Some("test_division"));
    assert!(py_diags[0].message.contains("ZeroDivisionError"));
}

#[test]
fn test_self_healing_generator_report() {
    let raw = m31a_verify::RawRunResult {
        project_kind: ProjectKind::Rust,
        command: "cargo check".to_string(),
        stdout: String::new(),
        stderr: "error[E0308]: mismatched types".to_string(),
        exit_code: 101,
        success: false,
        timed_out: false,
    };

    let diags = vec![m31a_verify::Diagnostic {
        file_path: PathBuf::from("src/main.rs"),
        line: 10,
        column: Some(4),
        severity: DiagnosticSeverity::Error,
        code: Some("E0308".to_string()),
        message: "mismatched types".to_string(),
        snippet: Some("let a: i32 = \"str\";".to_string()),
        suggested_fix: Some("use 42 instead of \"str\"".to_string()),
    }];

    let report = SelfHealingGenerator::evaluate(raw, diags);
    assert!(!report.success);
    assert_eq!(report.error_count, 1);
    assert_eq!(report.exit_code, 101);
    assert!(report.self_healing_prompt.is_some());
    let prompt = report.self_healing_prompt.unwrap();
    assert!(prompt.contains("Automated verification failed"));
    assert!(prompt.contains("E0308"));
    assert!(prompt.contains("src/main.rs:10:4"));
    assert!(prompt.contains("use 42 instead of \"str\""));
}

#[tokio::test]
async fn test_autonomous_verifier_real_execution() {
    let tmp = tempdir().unwrap();
    // In a generic folder with no build tool
    let report = AutonomousVerifier::verify_check(tmp.path(), Duration::from_secs(5))
        .await
        .unwrap();

    assert!(report.command.contains("echo"));
    assert_eq!(report.exit_code, 0);
    assert!(report.success);
    assert_eq!(report.error_count, 0);
    assert!(report.self_healing_prompt.is_none());

    // Test verify_tests
    let test_report = AutonomousVerifier::verify_tests(tmp.path(), Duration::from_secs(5))
        .await
        .unwrap();
    assert!(test_report.command.contains("No test suite"));
    assert!(test_report.success);

    // Test Python traceback parsing
    let py_traceback = r#"Traceback (most recent call last):
  File "src/math.py", line 42, in <module>
    compute(0)
ZeroDivisionError: division by zero
"#;
    let trace_diags = DiagnosticParser::parse(py_traceback);
    assert_eq!(trace_diags.len(), 1);
    assert_eq!(trace_diags[0].file_path, PathBuf::from("src/math.py"));
    assert_eq!(trace_diags[0].line, 42);
    assert!(trace_diags[0].message.contains("ZeroDivisionError"));
    assert_eq!(trace_diags[0].snippet.as_deref(), Some("compute(0)"));

    // Test Rust human diagnostic with help
    let rust_help = r#"error[E0425]: cannot find value `xyz` in this scope
  --> src/lib.rs:12:9
   |
12 |     let _ = xyz;
   |             ^^^ not found in this scope
help: a local variable with a similar name exists
   |
12 |     let _ = abc;
   |             ~~~
"#;
    let rust_diags = DiagnosticParser::parse(rust_help);
    assert_eq!(rust_diags.len(), 1);
    assert_eq!(rust_diags[0].file_path, PathBuf::from("src/lib.rs"));
    assert_eq!(rust_diags[0].line, 12);
    assert_eq!(rust_diags[0].code.as_deref(), Some("E0425"));
    assert!(rust_diags[0].suggested_fix.is_some());
}
