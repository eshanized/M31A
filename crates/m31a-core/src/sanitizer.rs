//! Secret redaction and sensitive information sanitizer.
//!
//! Scans text streams and error messages to mask API keys, passwords, private keys, and authorization headers.

use regex::Regex;
use std::sync::LazyLock;

static OPENAI_KEY_REGEX: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"\bsk-[A-Za-z0-9_\-]{20,}\b"));
static NVIDIA_KEY_REGEX: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"nvapi-[A-Za-z0-9_\-]{20,}"));
static ANTHROPIC_KEY_REGEX: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"sk-ant-[A-Za-z0-9_\-]{20,}"));
static GITHUB_TOKEN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{30,}\b"));
static AWS_ACCESS_KEY_ID_REGEX: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"\bAKIA[0-9A-Z]{16}\b"));
static BEARER_TOKEN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"(?i:\bBearer)[ \t]+[A-Za-z0-9._~+/-]{16,}=*"));
static SECRET_ASSIGNMENT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(r#"(?i)\b(api[_-]?key|token|secret|password)\b(\s*[:=]\s*)(["']?)[^\s"']{8,}"#)
});
static PRIVATE_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(r"(?s)-----BEGIN [A-Z ]+ PRIVATE KEY-----.*?-----END [A-Z ]+ PRIVATE KEY-----")
});

fn compile_regex(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(err) => panic!("invalid regex pattern `{pattern}`: {err}"),
    }
}

/// Redact credentials, API keys, bearer tokens, private keys, and secret assignments from a String.
/// Operates on a best-effort basis following standard credential signatures.
pub fn redact_secrets(input: &str) -> String {
    let redacted = PRIVATE_KEY_REGEX.replace_all(input, "[REDACTED_PRIVATE_KEY]");
    let redacted = BEARER_TOKEN_REGEX.replace_all(&redacted, "Bearer [REDACTED_SECRET]");
    let redacted = OPENAI_KEY_REGEX.replace_all(&redacted, "[REDACTED_SECRET]");
    let redacted = NVIDIA_KEY_REGEX.replace_all(&redacted, "[REDACTED_SECRET]");
    let redacted = ANTHROPIC_KEY_REGEX.replace_all(&redacted, "[REDACTED_SECRET]");
    let redacted = GITHUB_TOKEN_REGEX.replace_all(&redacted, "[REDACTED_SECRET]");
    let redacted = AWS_ACCESS_KEY_ID_REGEX.replace_all(&redacted, "[REDACTED_SECRET]");
    let redacted = SECRET_ASSIGNMENT_REGEX.replace_all(&redacted, "$1$2$3[REDACTED_SECRET]");

    redacted.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_bearer_tokens() {
        let input = "Authorization: Bearer sk_test_secret_long_token_value_here123==";
        let sanitized = redact_secrets(input);
        assert_eq!(sanitized, "Authorization: Bearer [REDACTED_SECRET]");
    }

    #[test]
    fn test_redact_openai_and_nvidia_keys() {
        let input = "export OPENAI_API_KEY=sk-abcdefghijklmnopqrstuvwxyz123456\nexport NVIDIA_API_KEY=nvapi-1234567890abcdefghijklmnopqrstuvwxyz";
        let sanitized = redact_secrets(input);
        assert!(!sanitized.contains("abcdefghijklmnopqrstuvwxyz"));
        assert!(sanitized.contains("[REDACTED_SECRET]"));
    }

    #[test]
    fn test_redact_aws_and_github() {
        let input = "AWS key is AKIA1234567890ABCDEF and github is ghp_1234567890abcdefghijklmnopqrstuvwxyz";
        let sanitized = redact_secrets(input);
        assert!(!sanitized.contains("AKIA1234567890ABCDEF"));
        assert!(!sanitized.contains("ghp_1234567890abcdef"));
        assert_eq!(
            sanitized,
            "AWS key is [REDACTED_SECRET] and github is [REDACTED_SECRET]"
        );
    }

    #[test]
    fn test_redact_private_key_pem() {
        let input =
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0...\n-----END RSA PRIVATE KEY-----";
        let sanitized = redact_secrets(input);
        assert_eq!(sanitized, "[REDACTED_PRIVATE_KEY]");
    }

    #[test]
    fn test_redact_assignments() {
        let input = "api_key = 'super_secret_password_here'; token: 1234567890abcdef";
        let sanitized = redact_secrets(input);
        assert_eq!(
            sanitized,
            "api_key = '[REDACTED_SECRET]'; token: [REDACTED_SECRET]"
        );
    }
}
