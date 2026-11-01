//! Dedicated test for `crates/m31a-review/src/analyzer.rs`.
//!
//! Validates `StaticDiffAnalyzer::analyze_diff` across all 9 vulnerability
//! and anti-pattern detectors, including line numbering and path extraction.

use m31a_review::analyzer::StaticDiffAnalyzer;
use m31a_review::model::{ReviewCategory, ReviewSeverity};

#[test]
fn test_analyzer_secret_patterns() {
    let diff = r#"--- a/secrets.rs
+++ b/secrets.rs
@@ -1,1 +1,5 @@
+let k1 = "nvapi-123456789012345678901234";
+let k2 = "sk-abcdefghijklmnopqrstuvwxyz12345";
+let k3 = "ghp_123456789012345678901234567890123456";
+let k4 = "AKIA1234567890ABCDEF";
+let k5 = "-----BEGIN RSA PRIVATE KEY-----";
"#;

    let findings = StaticDiffAnalyzer::analyze_diff(diff);
    assert_eq!(findings.len(), 5);
    for f in &findings {
        assert_eq!(f.severity, ReviewSeverity::Critical);
        assert_eq!(f.category, ReviewCategory::Security);
        assert!(f.title.contains("Hardcoded secret"));
    }
}

#[test]
fn test_analyzer_shell_and_sql_injections() {
    let diff = r#"--- a/exec.rs
+++ b/exec.rs
@@ -10,2 +10,4 @@
+let _ = Command::new("sh").arg("-c").arg(format!("echo {}", input));
+let _ = Command::new("bash").arg("-c").arg(format!("ls {}", input));
+let q1 = format!("SELECT * FROM users WHERE name = '{}'", input);
+let q2 = format!("DELETE FROM records WHERE id = '{}'", input);
"#;

    let findings = StaticDiffAnalyzer::analyze_diff(diff);
    assert_eq!(findings.len(), 4);

    let shell_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.title.contains("shell"))
        .collect();
    assert_eq!(shell_findings.len(), 2);
    for f in shell_findings {
        assert_eq!(f.severity, ReviewSeverity::High);
    }

    let sql_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.title.contains("SQL"))
        .collect();
    assert_eq!(sql_findings.len(), 2);
    for f in sql_findings {
        assert_eq!(f.severity, ReviewSeverity::High);
    }
}

#[test]
fn test_analyzer_path_traversal_and_busy_spin() {
    let diff = r#"--- a/worker.rs
+++ b/worker.rs
@@ -20,2 +20,4 @@
+let target = root.join("..").join(untrusted);
+loop { continue; }
"#;

    let findings = StaticDiffAnalyzer::analyze_diff(diff);
    assert_eq!(findings.len(), 2);

    let traversal = findings
        .iter()
        .find(|f| f.title.contains("path traversal"))
        .unwrap();
    assert_eq!(traversal.severity, ReviewSeverity::Medium);
    assert_eq!(traversal.category, ReviewCategory::Security);

    let spin = findings
        .iter()
        .find(|f| f.title.contains("busy-spin"))
        .unwrap();
    assert_eq!(spin.severity, ReviewSeverity::Medium);
    assert_eq!(spin.category, ReviewCategory::Performance);
}

#[test]
fn test_analyzer_static_mut_weak_hash_and_http() {
    let diff = r#"--- a/net.rs
+++ b/net.rs
@@ -5,1 +5,4 @@
+static mut GLOBAL_COUNT: u64 = 0;
+let h = md5::compute(data);
+let s = sha1::Sha1::new();
+let endpoint = "http://api.remote.service.com/v1/telemetry";
"#;

    let findings = StaticDiffAnalyzer::analyze_diff(diff);
    assert_eq!(findings.len(), 4);

    let static_mut = findings
        .iter()
        .find(|f| f.title.contains("static mut"))
        .unwrap();
    assert_eq!(static_mut.severity, ReviewSeverity::High);

    let weak_hashes: Vec<_> = findings
        .iter()
        .filter(|f| f.title.contains("Weak crypto"))
        .collect();
    assert_eq!(weak_hashes.len(), 2);

    let http = findings.iter().find(|f| f.title.contains("HTTP")).unwrap();
    assert_eq!(http.severity, ReviewSeverity::Low);
}

#[test]
fn test_analyzer_test_file_suppression() {
    // Test files should NOT trigger warnings for .unwrap(), MD5, or HTTP
    let diff = r#"--- a/tests/unit_test.rs
+++ b/tests/unit_test.rs
@@ -1,1 +1,4 @@
+let v = s.parse::<i32>().unwrap();
+let h = md5::compute(data);
+let url = "http://insecure.test.com";
"#;

    let findings = StaticDiffAnalyzer::analyze_diff(diff);
    assert!(
        findings.is_empty(),
        "test files should suppress unwraps, md5, and http warnings"
    );
}

#[test]
fn test_analyzer_line_number_tracking() {
    let diff = r#"--- a/src/app.rs
+++ b/src/app.rs
@@ -50,5 +100,5 @@
 context line 100
 context line 101
+let token = "sk-123456789012345678901234567890";
 context line 103
"#;

    let findings = StaticDiffAnalyzer::analyze_diff(diff);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].line_number, Some(102));
    assert_eq!(
        findings[0].file_path,
        std::path::PathBuf::from("src/app.rs")
    );
}
