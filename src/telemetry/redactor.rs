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
pub struct SecretRedactor {
    registered_secrets: RwLock<HashSet<String>>,
    bearer_regex: Regex,
    aws_key_regex: Regex,
    private_key_regex: Regex,
    generic_token_regex: Regex,
}

impl Default for SecretRedactor {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretRedactor {
    /// Create a new SecretRedactor with default regex patterns.
    pub fn new() -> Self {
        Self {
            registered_secrets: RwLock::new(HashSet::new()),
            bearer_regex: Regex::new(
                r"(?i)bearer\s+[A-Za-z0-9\-_=]+\.[A-Za-z0-9\-_=]+\.?[A-Za-z0-9\-_.+/=]*",
            )
            .expect("valid bearer regex"),
            aws_key_regex: Regex::new(r"AKIA[0-9A-Z]{16}").expect("valid aws key regex"),
            private_key_regex: Regex::new(
                r"-----BEGIN [A-Z ]+ PRIVATE KEY-----[\s\S]*?-----END [A-Z ]+ PRIVATE KEY-----",
            )
            .expect("valid private key regex"),
            generic_token_regex: Regex::new(r"(?:ghp_[A-Za-z0-9]{36}|sk-[A-Za-z0-9\-_]{20,})")
                .expect("valid generic token regex"),
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
            .bearer_regex
            .replace_all(&text, "[REDACTED:BEARER_TOKEN]")
            .into_owned();
        text = self
            .aws_key_regex
            .replace_all(&text, "[REDACTED:AWS_KEY]")
            .into_owned();
        text = self
            .private_key_regex
            .replace_all(&text, "[REDACTED:PRIVATE_KEY]")
            .into_owned();
        text = self
            .generic_token_regex
            .replace_all(&text, "[REDACTED:API_TOKEN]")
            .into_owned();

        text
    }

    /// Alias for `redact_string`.
    pub fn redact_text(&self, input: &str) -> String {
        self.redact_string(input)
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
                        || key_lower.contains("credential")
                        || key_lower.contains("authorization")
                        || (key_lower.contains("key")
                            && !key_lower.contains("keyword")
                            && !key_lower.contains("keys"))
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
}
