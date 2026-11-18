//! Audit Event Data Minimization and Secret Redaction.
//!
//! Scrubs tokens, passwords, private keys, and credential filenames from
//! captured execution parameters and file paths before audit persistence.

use std::path::Path;

pub struct SecretRedactor;

impl SecretRedactor {
    pub const REDACTED_MARKER: &'static str = "[REDACTED_SECRET]";

    /// Redacts known sensitive patterns from command line arguments.
    pub fn redact_arg(arg: &str) -> String {
        let lower = arg.to_ascii_lowercase();

        // Flag patterns: --token=..., --password=..., -p=...
        if lower.starts_with("--token=")
            || lower.starts_with("--password=")
            || lower.starts_with("--api-key=")
            || lower.starts_with("--secret=")
            || lower.starts_with("--auth=")
        {
            if let Some((flag, _val)) = arg.split_once('=') {
                return format!("{}={}", flag, Self::REDACTED_MARKER);
            }
        }

        // Common bearer token patterns
        if lower.starts_with("bearer ") {
            return format!("Bearer {}", Self::REDACTED_MARKER);
        }

        // High-entropy token prefixes
        if arg.starts_with("nvapi-")
            || arg.starts_with("sk-ant-")
            || arg.starts_with("ghp_")
            || arg.starts_with("gho_")
            || arg.starts_with("glpat-")
        {
            return Self::REDACTED_MARKER.to_string();
        }

        arg.to_string()
    }

    /// Redacts a vector of arguments.
    pub fn redact_args(args: &[String]) -> Vec<String> {
        let mut out = Vec::with_capacity(args.len());
        let mut redact_next = false;

        for arg in args {
            if redact_next {
                out.push(Self::REDACTED_MARKER.to_string());
                redact_next = false;
                continue;
            }

            let lower = arg.to_ascii_lowercase();
            if lower == "--token"
                || lower == "--password"
                || lower == "--api-key"
                || lower == "--secret"
                || lower == "-p"
            {
                out.push(arg.clone());
                redact_next = true;
                continue;
            }

            out.push(Self::redact_arg(arg));
        }

        out
    }

    /// Redacts sensitive filesystem path components.
    pub fn redact_path(path_str: &str) -> String {
        let path = Path::new(path_str);
        for comp in path.components() {
            let comp_str = comp.as_os_str().to_string_lossy();
            if comp_str == ".env"
                || comp_str.starts_with(".env.")
                || comp_str == ".ssh"
                || comp_str == "id_rsa"
                || comp_str == "id_ed25519"
                || comp_str == ".aws"
                || comp_str == ".m31a-vault"
            {
                return format!("[PROTECTED_PATH]/{}", path.file_name().unwrap_or_default().to_string_lossy());
            }
        }
        path_str.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_redaction_flags() {
        let raw = vec![
            "curl".to_string(),
            "-H".to_string(),
            "Authorization: Bearer secret12345".to_string(),
            "--token=sensitive_token_abc".to_string(),
            "--api-key".to_string(),
            "nvapi-top-secret".to_string(),
        ];

        let redacted = SecretRedactor::redact_args(&raw);
        assert_eq!(redacted[0], "curl");
        assert_eq!(redacted[3], "--token=[REDACTED_SECRET]");
        assert_eq!(redacted[4], "--api-key");
        assert_eq!(redacted[5], "[REDACTED_SECRET]");
    }

    #[test]
    fn test_secret_redaction_paths() {
        let p1 = "/home/user/.ssh/id_ed25519";
        let p2 = "/workspace/.env.production";
        let p3 = "/workspace/src/main.rs";

        assert_eq!(SecretRedactor::redact_path(p1), "[PROTECTED_PATH]/id_ed25519");
        assert_eq!(SecretRedactor::redact_path(p2), "[PROTECTED_PATH]/.env.production");
        assert_eq!(SecretRedactor::redact_path(p3), "/workspace/src/main.rs");
    }
}
