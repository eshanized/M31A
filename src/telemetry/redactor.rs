//! Multi-tier Secret Redaction & Sanitization Boundary (OBS-02, D-02).
//!
//! Enforces deterministic, unconditional scrubbing of API tokens, private keys,
//! AWS credentials, registered secret tokens, ANSI escapes, and log injection controls
//! before telemetry data is stored in SQLite or appended to NDJSON streams.

use std::collections::HashSet;
use std::sync::RwLock;

use regex::Regex;
use serde_json::Value;

/// Multi-tier deterministic secret redactor.
/// Multi-tier deterministic secret redactor.
pub struct SecretRedactor {
    registered_secrets: RwLock<HashSet<String>>,
    bearer_regex: Regex,
    aws_key_regex: Regex,
    aws_secret_regex: Regex,
    private_key_regex: Regex,
    nvidia_key_regex: Regex,
    github_token_regex: Regex,
    gitlab_token_regex: Regex,
    generic_token_regex: Regex,
    jwt_regex: Regex,
    db_url_regex: Regex,
    auth_header_regex: Regex,
    credential_kv_regex: Regex,
}

impl Default for SecretRedactor {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretRedactor {
    /// Create a new SecretRedactor with comprehensive credential scrub patterns.
    pub fn new() -> Self {
        Self {
            registered_secrets: RwLock::new(HashSet::new()),
            bearer_regex: Regex::new(
                r"(?i)bearer\s+[A-Za-z0-9\-_=]+\.[A-Za-z0-9\-_=]+\.?[A-Za-z0-9\-_.+/=]*",
            )
            .expect("valid bearer regex"),
            aws_key_regex: Regex::new(r"\bAKIA[0-9A-Z]{16}\b").expect("valid aws key regex"),
            aws_secret_regex: Regex::new(
                r#"(?i)(aws_secret_access_key|aws_session_token)(\s*[=:]\s*["']?)([A-Za-z0-9/+=]{40})(["']?)"#,
            )
            .expect("valid aws secret regex"),
            private_key_regex: Regex::new(
                r"-----BEGIN [A-Z0-9 -]+ PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 -]+ PRIVATE KEY-----",
            )
            .expect("valid private key regex"),
            nvidia_key_regex: Regex::new(r"\bnvapi-[A-Za-z0-9\-_]{20,}\b")
                .expect("valid nvidia key regex"),
            github_token_regex: Regex::new(r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,}\b")
                .expect("valid github token regex"),
            gitlab_token_regex: Regex::new(r"\bglpat-[A-Za-z0-9\-_]{20,}\b")
                .expect("valid gitlab token regex"),
            generic_token_regex: Regex::new(r"\bsk-[A-Za-z0-9\-_]{20,}\b")
                .expect("valid generic token regex"),
            jwt_regex: Regex::new(
                r"\beyJ[A-Za-z0-9-_]{10,}\.eyJ[A-Za-z0-9-_]{10,}\.[A-Za-z0-9-_+/=]{10,}\b",
            )
            .expect("valid jwt regex"),
            db_url_regex: Regex::new(r"(?i)([a-z0-9+.-]+://[^:\s]+:)([^@\s]+)(@)")
                .expect("valid db url regex"),
            auth_header_regex: Regex::new(
                r"(?i)(?:authorization|proxy-authorization):\s*(?:basic|digest)\s+[A-Za-z0-9+/=]+",
            )
            .expect("valid auth header regex"),
            credential_kv_regex: Regex::new(
                r#"(?i)\b(password|passwd|api_key|apikey|secret_key|access_token|auth_token)(\s*[=:]\s*["']?)([^\s"';&]+)(["']?)"#,
            )
            .expect("valid credential kv regex"),
        }
    }

    /// Register a sensitive secret token for exact-match replacement.
    ///
    /// Ignores tokens shorter than 6 characters to prevent destructive over-redaction.
    pub fn register_secret(&self, secret: &str) {
        let trimmed = secret.trim();
        if trimmed.len() >= 6 {
            let mut secrets = self
                .registered_secrets
                .write()
                .expect("write lock for secrets");
            secrets.insert(trimmed.to_string());
        }
    }

    /// Sanitize a string by stripping ANSI codes, carriage returns, registered secrets, and regex patterns.
    pub fn redact_string(&self, input: &str) -> String {
        if input.is_empty() {
            return String::new();
        }

        // 1. Strip raw ANSI escape sequences
        let stripped_bytes = strip_ansi_escapes::strip(input.as_bytes());
        let mut text = String::from_utf8_lossy(&stripped_bytes).into_owned();

        // 2. Strip carriage returns to prevent terminal log injection / line forging
        text = text.replace('\r', "");

        // 3. Exact match scrubbing against registered tokens
        {
            let secrets = self
                .registered_secrets
                .read()
                .expect("read lock for secrets");
            for secret in secrets.iter() {
                if text.contains(secret) {
                    text = text.replace(secret, "[REDACTED:EXACT_SECRET]");
                }
            }
        }

        // 4. Scrub known regex credential patterns
        text = self
            .private_key_regex
            .replace_all(&text, "[REDACTED:PRIVATE_KEY]")
            .into_owned();
        text = self
            .bearer_regex
            .replace_all(&text, "[REDACTED:BEARER_TOKEN]")
            .into_owned();
        text = self
            .jwt_regex
            .replace_all(&text, "[REDACTED:JWT_TOKEN]")
            .into_owned();
        text = self
            .nvidia_key_regex
            .replace_all(&text, "[REDACTED:NVIDIA_KEY]")
            .into_owned();
        text = self
            .github_token_regex
            .replace_all(&text, "[REDACTED:GITHUB_TOKEN]")
            .into_owned();
        text = self
            .gitlab_token_regex
            .replace_all(&text, "[REDACTED:GITLAB_TOKEN]")
            .into_owned();
        text = self
            .aws_key_regex
            .replace_all(&text, "[REDACTED:AWS_KEY]")
            .into_owned();
        text = self
            .aws_secret_regex
            .replace_all(&text, "$1$2[REDACTED:AWS_SECRET]$4")
            .into_owned();
        text = self
            .generic_token_regex
            .replace_all(&text, "[REDACTED:API_TOKEN]")
            .into_owned();
        text = self
            .db_url_regex
            .replace_all(&text, "${1}[REDACTED:DB_PASSWORD]${3}")
            .into_owned();
        text = self
            .auth_header_regex
            .replace_all(&text, "[REDACTED:AUTH_HEADER]")
            .into_owned();
        text = self
            .credential_kv_regex
            .replace_all(&text, "$1$2[REDACTED:CREDENTIAL]$4")
            .into_owned();

        text
    }

    /// Alias for `redact_string`.
    pub fn redact_text(&self, input: &str) -> String {
        self.redact_string(input)
    }

    /// Sanitize an error or diagnostic message by stripping ANSI sequences and scrubbing credentials.
    pub fn sanitize_error(&self, err: &str) -> String {
        self.redact_string(err)
    }

    /// Recursively redact a JSON value in-place.
    ///
    /// Sensitive field names have their entire value replaced, and any string
    /// fields are filtered through `redact_string`.
    pub fn redact_value(&self, value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (k, v) in map.iter_mut() {
                    let key_lower = k.to_lowercase();
                    if key_lower.contains("secret")
                        || key_lower.contains("token")
                        || key_lower.contains("password")
                        || key_lower.contains("passwd")
                        || key_lower.contains("credential")
                        || key_lower.contains("authorization")
                        || key_lower.contains("conn_str")
                        || key_lower.contains("database_url")
                        || (key_lower.contains("auth") && !key_lower.contains("author"))
                        || (key_lower.contains("key")
                            && !key_lower.contains("keyword")
                            && !key_lower.contains("keys")
                            && !key_lower.contains("key_"))
                    {
                        *v = Value::String("[REDACTED:SENSITIVE_KEY]".to_string());
                    } else {
                        self.redact_value(v);
                    }
                }
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    self.redact_value(item);
                }
            }
            Value::String(s) => {
                *s = self.redact_string(s);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_exact_secret_redaction() {
        let redactor = SecretRedactor::new();
        redactor.register_secret("super-secret-password-123");

        let input = "Authentication attempted with super-secret-password-123 in payload.";
        let result = redactor.redact_string(input);
        assert_eq!(
            result,
            "Authentication attempted with [REDACTED:EXACT_SECRET] in payload."
        );
    }

    #[test]
    fn test_regex_credential_scrubbing() {
        let redactor = SecretRedactor::new();
        let input = "Headers: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.t-ae9_xxxx and AWS AKIAIOSFODNN7EXAMPLE";
        let result = redactor.redact_string(input);
        assert!(result.contains("[REDACTED:BEARER_TOKEN]"));
        assert!(result.contains("[REDACTED:AWS_KEY]"));
        assert!(!result.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_ansi_and_carriage_return_sanitization() {
        let redactor = SecretRedactor::new();
        let input = "\x1b[31mDangerous Error\x1b[0m\r\nForged Log Line";
        let result = redactor.redact_string(input);
        assert_eq!(result, "Dangerous Error\nForged Log Line");
    }

    #[test]
    fn test_json_value_recursive_redaction() {
        let redactor = SecretRedactor::new();
        redactor.register_secret("my-api-key-9999");

        let mut data = json!({
            "api_key": "some-val",
            "user_token": "abc123456",
            "nested": {
                "password": "secret",
                "message": "User said my-api-key-9999 here"
            },
            "list": [
                "safe element",
                "Bearer abc.def.ghi"
            ]
        });

        redactor.redact_value(&mut data);

        assert_eq!(data["api_key"], "[REDACTED:SENSITIVE_KEY]");
        assert_eq!(data["user_token"], "[REDACTED:SENSITIVE_KEY]");
        assert_eq!(data["nested"]["password"], "[REDACTED:SENSITIVE_KEY]");
        assert_eq!(
            data["nested"]["message"],
            "User said [REDACTED:EXACT_SECRET] here"
        );
        assert_eq!(data["list"][1], "[REDACTED:BEARER_TOKEN]");
    }

    #[test]
    fn test_nvidia_api_key_redaction() {
        let redactor = SecretRedactor::new();
        let input = "Client configured with NVIDIA NIM key nvapi-abc1234567890abcdef1234567890.";
        let result = redactor.redact_string(input);
        assert!(result.contains("[REDACTED:NVIDIA_KEY]"));
        assert!(!result.contains("nvapi-abc1234567890"));
    }

    #[test]
    fn test_github_and_gitlab_token_redaction() {
        let redactor = SecretRedactor::new();
        let input = "Tokens: ghp_111122223333444455556666777788889999 and glpat-abcdef12345678901234 for repos.";
        let result = redactor.redact_string(input);
        assert!(result.contains("[REDACTED:GITHUB_TOKEN]"));
        assert!(result.contains("[REDACTED:GITLAB_TOKEN]"));
        assert!(!result.contains("ghp_1111"));
        assert!(!result.contains("glpat-"));
    }

    #[test]
    fn test_jwt_and_private_key_redaction() {
        let redactor = SecretRedactor::new();
        let jwt = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgN_dummy_signature_value";
        let priv_key =
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0...\n-----END RSA PRIVATE KEY-----";
        let input = format!("JWT token: {}\nKey:\n{}", jwt, priv_key);

        let result = redactor.redact_string(&input);
        assert!(result.contains("[REDACTED:PRIVATE_KEY]"));
        assert!(!result.contains("MIIEowIBAAKCAQEA0"));
        assert!(
            result.contains("[REDACTED:BEARER_TOKEN]") || result.contains("[REDACTED:JWT_TOKEN]")
        );
    }

    #[test]
    fn test_database_url_credential_redaction() {
        let redactor = SecretRedactor::new();
        let input =
            "Connecting to postgres://admin:super_secret_pw123@db.prod.internal:5432/m31a_db";
        let result = redactor.redact_string(input);
        assert!(
            result
                .contains("postgres://admin:[REDACTED:DB_PASSWORD]@db.prod.internal:5432/m31a_db")
        );
        assert!(!result.contains("super_secret_pw123"));
    }

    #[test]
    fn test_auth_header_and_env_kv_redaction() {
        let redactor = SecretRedactor::new();
        let input = "Authorization: Basic dXNlcjpwYXNz\npassword=MySecretPassword!\napi_key=\"sk-12345678901234567890\"";
        let result = redactor.redact_string(input);
        assert!(result.contains("[REDACTED:AUTH_HEADER]"));
        assert!(result.contains("password=[REDACTED:CREDENTIAL]"));
        assert!(!result.contains("MySecretPassword!"));
    }

    #[test]
    fn test_sanitize_error_helper() {
        let redactor = SecretRedactor::new();
        let err = "\x1b[31mFailed to authenticate with token sk-abcdef12345678901234\x1b[0m\r";
        let sanitized = redactor.sanitize_error(err);
        assert!(!sanitized.contains("\x1b[31m"));
        assert!(!sanitized.contains('\r'));
        assert!(sanitized.contains("[REDACTED:API_TOKEN]"));
        assert!(!sanitized.contains("sk-abcdef"));
    }
}
