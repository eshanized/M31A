//! Static pattern and vulnerability analyzer for diff additions.

use regex::Regex;
use std::path::PathBuf;

use crate::model::{ReviewCategory, ReviewFinding, ReviewSeverity};

pub struct StaticDiffAnalyzer;

impl StaticDiffAnalyzer {
    /// Analyzes added and modified lines across a diff to detect common security and reliability anti-patterns.
    pub fn analyze_diff(diff_text: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        let mut current_file = PathBuf::from("unknown");
        let mut current_line_num = 1;

        let secret_regex = Regex::new(r#"(?i)(nvapi-[a-zA-Z0-9_\-]{20,}|sk-[a-zA-Z0-9_\-]{20,}|ghp_[a-zA-Z0-9]{30,}|AKIA[0-9A-Z]{16}|BEGIN (RSA|EC|OPENSSH|DSA|PGP) PRIVATE KEY)"#).unwrap();
        let sql_injection_regex = Regex::new(
            r#"(?i)format!\s*\(\s*".*\b(SELECT|INSERT|UPDATE|DELETE|DROP)\b.*\{\}.*"\s*,"#,
        )
        .unwrap();
        let shell_injection_regex = Regex::new(r#"(?i)Command::new\s*\(\s*"(sh|bash)"\s*\)\s*\.arg\s*\(\s*"-c"\s*\)\s*\.arg\s*\(\s*format!"#).unwrap();

        for line in diff_text.lines() {
            if line.starts_with("+++ b/") || line.starts_with("+++ ") {
                let raw_path = line
                    .strip_prefix("+++ b/")
                    .or_else(|| line.strip_prefix("+++ "))
                    .unwrap()
                    .trim();
                current_file = PathBuf::from(raw_path);
                current_line_num = 1;
                continue;
            }

            if line.starts_with("@@ ") {
                // Parse @@ -old,count +new,count @@
                if let Some(plus_idx) = line.find('+')
                    && let Some(comma_or_space) = line[plus_idx + 1..].find([',', ' '])
                {
                    let num_str = &line[plus_idx + 1..plus_idx + 1 + comma_or_space];
                    if let Ok(n) = num_str.parse::<usize>() {
                        current_line_num = n;
                    }
                }
                continue;
            }

            // Only analyze added lines (lines starting with `+` and not `+++`)
            if let Some(added_line) = line.strip_prefix('+') {
                let trimmed = added_line.trim();
                let is_test_file = current_file.to_string_lossy().contains("test")
                    || current_file.to_string_lossy().contains("mock");

                // 1. Hardcoded secrets check
                if secret_regex.is_match(trimmed) {
                    findings.push(ReviewFinding {
                        file_path: current_file.clone(),
                        line_number: Some(current_line_num),
                        severity: ReviewSeverity::Critical,
                        category: ReviewCategory::Security,
                        title: "Hardcoded secret or credential detected".to_string(),
                        description: "A sensitive API token, private key, or credential was found committed directly in code.".to_string(),
                        code_snippet: Some(trimmed.to_string()),
                        suggested_fix: Some("Store secrets in environment variables or an encrypted secrets vault.".to_string()),
                    });
                }

                // 2. Shell command injection check
                if shell_injection_regex.is_match(trimmed) {
                    findings.push(ReviewFinding {
                        file_path: current_file.clone(),
                        line_number: Some(current_line_num),
                        severity: ReviewSeverity::High,
                        category: ReviewCategory::Security,
                        title: "Potential shell command injection".to_string(),
                        description: "Executing a shell wrapper `sh -c` with dynamic `format!` strings can allow malicious input to execute arbitrary commands.".to_string(),
                        code_snippet: Some(trimmed.to_string()),
                        suggested_fix: Some("Pass command arguments as separate array elements instead of string interpolation.".to_string()),
                    });
                }

                // 3. SQL injection check
                if sql_injection_regex.is_match(trimmed) {
                    findings.push(ReviewFinding {
                        file_path: current_file.clone(),
                        line_number: Some(current_line_num),
                        severity: ReviewSeverity::High,
                        category: ReviewCategory::Security,
                        title: "Potential SQL injection vulnerability".to_string(),
                        description: "SQL query string constructed via string interpolation rather than parameterized queries.".to_string(),
                        code_snippet: Some(trimmed.to_string()),
                        suggested_fix: Some("Use parameterized queries or prepared statements.".to_string()),
                    });
                }

                // 4. Path traversal check
                if trimmed.contains("join(") && trimmed.contains("..") {
                    findings.push(ReviewFinding {
                        file_path: current_file.clone(),
                        line_number: Some(current_line_num),
                        severity: ReviewSeverity::Medium,
                        category: ReviewCategory::Security,
                        title: "Relative path traversal risk".to_string(),
                        description: "Joining paths containing `..` without canonicalization may allow accessing files outside the workspace root.".to_string(),
                        code_snippet: Some(trimmed.to_string()),
                        suggested_fix: Some("Validate paths with `.canonicalize()` and verify they remain within the target root directory.".to_string()),
                    });
                }

                // 5. Unchecked unwrap in production code
                if !is_test_file && (trimmed.contains(".unwrap()") || trimmed.contains(".expect("))
                {
                    // Filter out trivial patterns like `Mutex::lock().unwrap()` or lazy statics
                    if !trimmed.contains("lock().unwrap()")
                        && !trimmed.contains("Regex::new(")
                        && !trimmed.contains("Lazy::new(")
                    {
                        findings.push(ReviewFinding {
                            file_path: current_file.clone(),
                            line_number: Some(current_line_num),
                            severity: ReviewSeverity::Low,
                            category: ReviewCategory::Correctness,
                            title: "Unchecked panic vector (`.unwrap()` / `.expect()`)".to_string(),
                            description: "Calling `.unwrap()` in production code can panic the process if an error occurs.".to_string(),
                            code_snippet: Some(trimmed.to_string()),
                            suggested_fix: Some("Use pattern matching (`match`/`if let`) or error propagation with `?`.".to_string()),
                        });
                    }
                }

                // 6. Infinite sleep/spin loop
                if trimmed.starts_with("loop {")
                    && (trimmed.contains("continue") && !trimmed.contains("sleep"))
                {
                    findings.push(ReviewFinding {
                        file_path: current_file.clone(),
                        line_number: Some(current_line_num),
                        severity: ReviewSeverity::Medium,
                        category: ReviewCategory::Performance,
                        title: "Potential busy-spin loop".to_string(),
                        description: "Loop structure appears to spin without yielding to the scheduler or sleeping, causing 100% CPU usage.".to_string(),
                        code_snippet: Some(trimmed.to_string()),
                        suggested_fix: Some("Introduce an async `sleep()` or event notification channel.".to_string()),
                    });
                }

                // 7. Dangerous static mut / data race vector
                if trimmed.contains("static mut ") {
                    findings.push(ReviewFinding {
                        file_path: current_file.clone(),
                        line_number: Some(current_line_num),
                        severity: ReviewSeverity::High,
                        category: ReviewCategory::Security,
                        title: "Mutable global state (`static mut`)".to_string(),
                        description: "Using `static mut` introduces undefined behavior and data races across multithreaded tasks.".to_string(),
                        code_snippet: Some(trimmed.to_string()),
                        suggested_fix: Some("Replace with `std::sync::atomic` types or `tokio::sync::RwLock` / `std::sync::Mutex`.".to_string()),
                    });
                }

                // 8. Weak hashing algorithms (MD5 / SHA-1)
                if (trimmed.contains("md5::")
                    || trimmed.contains("sha1::")
                    || trimmed.contains("Md5::new()")
                    || trimmed.contains("Sha1::new()"))
                    && !is_test_file
                {
                    findings.push(ReviewFinding {
                        file_path: current_file.clone(),
                        line_number: Some(current_line_num),
                        severity: ReviewSeverity::Medium,
                        category: ReviewCategory::Security,
                        title: "Weak cryptographic hash function detected (MD5/SHA1)".to_string(),
                        description: "MD5 and SHA-1 suffer from known collision vulnerabilities and should not be used for security-sensitive hashing.".to_string(),
                        code_snippet: Some(trimmed.to_string()),
                        suggested_fix: Some("Upgrade to SHA-256 (`sha2::Sha256`), BLAKE3, or ChaCha20-Poly1305.".to_string()),
                    });
                }

                // 9. Plaintext HTTP transport
                if trimmed.contains("http://")
                    && !trimmed.contains("localhost")
                    && !trimmed.contains("127.0.0.1")
                    && !is_test_file
                {
                    findings.push(ReviewFinding {
                        file_path: current_file.clone(),
                        line_number: Some(current_line_num),
                        severity: ReviewSeverity::Low,
                        category: ReviewCategory::Security,
                        title: "Insecure unencrypted HTTP connection".to_string(),
                        description: "External network connection using plaintext `http://` instead of encrypted `https://`.".to_string(),
                        code_snippet: Some(trimmed.to_string()),
                        suggested_fix: Some("Enforce TLS encryption using `https://`.".to_string()),
                    });
                }

                current_line_num += 1;
            } else if line.starts_with(' ') {
                current_line_num += 1;
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_detection() {
        let diff = r#"
--- a/src/config.rs
+++ b/src/config.rs
@@ -10,2 +10,3 @@
 let default_port = 8080;
+let api_key = "nvapi-secret12345678901234567890";
"#;
        let findings = StaticDiffAnalyzer::analyze_diff(diff);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, ReviewSeverity::Critical);
        assert_eq!(findings[0].category, ReviewCategory::Security);
    }

    #[test]
    fn test_unwrap_detection_in_prod() {
        let diff = r#"
--- a/src/server.rs
+++ b/src/server.rs
@@ -25,2 +25,3 @@
 fn parse(s: &str) -> i32 {
+    s.parse::<i32>().unwrap()
 }
"#;
        let findings = StaticDiffAnalyzer::analyze_diff(diff);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, ReviewSeverity::Low);
    }
}
