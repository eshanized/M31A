//! In-memory credential broker managing brokered secrets, token lifetimes, and scoped access.
//!
//! Securely provisions short-lived authorization credentials to subagents and external tools
//! with format-preserving dummy tokens and byte-exact HTTP header and body secret substitution.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokeredCredential {
    pub id: String,
    pub destination_domains: Vec<String>,
    pub dummy_token: String,
    pub real_token: String,
    pub header_name: String,
    pub header_template: String, // e.g. "Bearer {}" or "{}"
}

impl BrokeredCredential {
    pub fn new(
        id: impl Into<String>,
        destination_domains: Vec<String>,
        real_token: impl Into<String>,
        header_name: impl Into<String>,
        header_template: impl Into<String>,
    ) -> Self {
        let id_str = id.into();
        let real_token_str = real_token.into();
        let dummy = Self::generate_format_preserving_dummy(&id_str, &real_token_str);
        Self {
            id: id_str,
            destination_domains,
            dummy_token: dummy,
            real_token: real_token_str,
            header_name: header_name.into(),
            header_template: header_template.into(),
        }
    }

    /// Generates format-preserving synthetic dummy tokens matching downstream SDK validation regexes.
    pub fn generate_format_preserving_dummy(id: &str, real_token: &str) -> String {
        let uuid_str = Uuid::new_v4().simple().to_string();
        let id_lower = id.to_lowercase();
        let token_lower = real_token.to_lowercase();

        if id_lower.contains("openai") || token_lower.starts_with("sk-proj-") {
            // OpenAI project token regex: ^sk-proj-[A-Za-z0-9_-]{48,}$
            format!(
                "sk-proj-masked_{}_{}",
                uuid_str, "0123456789abcdef0123456789abcdef"
            )
        } else if id_lower.contains("github") || token_lower.starts_with("ghp_") {
            // GitHub personal access token regex: ^ghp_[A-Za-z0-9]{36}$
            format!("ghp_masked_{}", &uuid_str[..25])
        } else if id_lower.contains("anthropic") || token_lower.starts_with("sk-ant-") {
            // Anthropic API token regex: ^sk-ant-api03-[A-Za-z0-9_-]{90,}$
            format!(
                "sk-ant-api03-masked_{}_{}_{}",
                uuid_str,
                uuid_str,
                &uuid_str[..20]
            )
        } else {
            format!("m31a-broker-{}-{}", id, uuid_str)
        }
    }

    /// Checks if a request host matches any allowed destination domains for this credential.
    pub fn matches_host(&self, host: &str) -> bool {
        let clean_host = host.split(':').next().unwrap_or(host).to_lowercase();
        for domain in &self.destination_domains {
            let clean_domain = domain.to_lowercase();
            if clean_host == clean_domain || clean_host.ends_with(&format!(".{clean_domain}")) {
                return true;
            }
        }
        false
    }

    /// Formats the real header value.
    pub fn format_real_header(&self) -> String {
        if self.header_template.contains("{}") {
            self.header_template.replace("{}", &self.real_token)
        } else {
            self.real_token.clone()
        }
    }
}

/// Dynamic zero-trust credential broker.
///
/// Prevents untrusted sandbox child processes and shell scripts from accessing real
/// credentials on disk or environment variables. The broker provides synthetic dummy
/// tokens to the sandbox environment and replaces them with authentic credentials only
/// within proxied outbound network requests to authorized destination domains.
#[derive(Clone, Debug, Default)]
pub struct CredentialBroker {
    credentials: Arc<RwLock<Vec<BrokeredCredential>>>,
}

impl CredentialBroker {
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Registers a credential for broker management.
    pub fn register(&self, credential: BrokeredCredential) {
        let mut list = self.credentials.write().unwrap();
        list.retain(|c| c.id != credential.id);
        list.push(credential);
    }

    /// Replaces real credentials in the environment with dummy tokens.
    pub fn mask_environment(&self, env: &mut HashMap<String, String>) {
        let list = self.credentials.read().unwrap();
        for cred in list.iter() {
            if cred.real_token.is_empty() {
                continue;
            }
            for val in env.values_mut() {
                if val.contains(&cred.real_token) {
                    *val = val.replace(&cred.real_token, &cred.dummy_token);
                }
            }
        }
    }

    /// Inspects outbound HTTP headers for a given destination host.
    /// If the host matches an authorized credential domain and contains a matching dummy token,
    /// replaces it in-place with the real authentic credential.
    pub fn inject_into_headers(&self, host: &str, headers: &mut HashMap<String, String>) -> bool {
        let list = self.credentials.read().unwrap();
        let mut modified = false;

        for cred in list.iter() {
            if !cred.matches_host(host) {
                continue;
            }

            // Check if target header exists and contains dummy token
            for (key, val) in headers.iter_mut() {
                if key.eq_ignore_ascii_case(&cred.header_name) && val.contains(&cred.dummy_token) {
                    *val = val.replace(&cred.dummy_token, &cred.real_token);
                    modified = true;
                }
            }
        }

        modified
    }

    /// Bounded byte-exact secret substitution for request headers and body.
    /// Replaces format-preserving dummy tokens with authentic credentials for authorized destinations.
    pub fn substitute_request_secrets(
        &self,
        host: &str,
        headers: &mut HashMap<String, String>,
        body: &mut Vec<u8>,
    ) -> bool {
        let list = self.credentials.read().unwrap();
        let mut modified = false;

        for cred in list.iter() {
            if !cred.matches_host(host) {
                continue;
            }

            // 1. Substitute in all headers
            for val in headers.values_mut() {
                if val.contains(&cred.dummy_token) {
                    *val = val.replace(&cred.dummy_token, &cred.real_token);
                    modified = true;
                }
            }

            // 2. Exact byte substitution in request body
            let dummy_bytes = cred.dummy_token.as_bytes();
            let real_bytes = cred.real_token.as_bytes();
            if !dummy_bytes.is_empty() && body.len() >= dummy_bytes.len() {
                let mut new_body = Vec::new();
                let mut cursor = 0;
                while cursor <= body.len().saturating_sub(dummy_bytes.len()) {
                    if &body[cursor..cursor + dummy_bytes.len()] == dummy_bytes {
                        new_body.extend_from_slice(real_bytes);
                        cursor += dummy_bytes.len();
                        modified = true;
                    } else {
                        new_body.push(body[cursor]);
                        cursor += 1;
                    }
                }
                new_body.extend_from_slice(&body[cursor..]);
                *body = new_body;
            }
        }

        modified
    }

    /// Redacts any accidentally exposed real credentials from log strings.
    pub fn sanitize_log(&self, text: &str) -> String {
        let list = self.credentials.read().unwrap();
        let mut out = text.to_string();
        for cred in list.iter() {
            if !cred.real_token.is_empty() && out.contains(&cred.real_token) {
                out = out.replace(&cred.real_token, "[BROKER_REDACTED_CREDENTIAL]");
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_preserving_dummies() {
        let openai = BrokeredCredential::new(
            "openai",
            vec!["api.openai.com".to_string()],
            "sk-proj-supersecret1234567890abcdef1234567890abcdef",
            "Authorization",
            "Bearer {}",
        );
        assert!(openai.dummy_token.starts_with("sk-proj-masked_"));
        assert!(openai.dummy_token.len() >= 48);

        let github = BrokeredCredential::new(
            "github",
            vec!["api.github.com".to_string()],
            "ghp_actualsecrettoken1234567890123456",
            "Authorization",
            "token {}",
        );
        assert!(github.dummy_token.starts_with("ghp_masked_"));
        assert_eq!(github.dummy_token.len(), 36);

        let anthropic = BrokeredCredential::new(
            "anthropic",
            vec!["api.anthropic.com".to_string()],
            "sk-ant-api03-actualsecrettoken1234567890123456",
            "x-api-key",
            "{}",
        );
        assert!(anthropic.dummy_token.starts_with("sk-ant-api03-masked_"));
    }

    #[test]
    fn test_brokered_credential_host_matching() {
        let cred = BrokeredCredential::new(
            "github",
            vec!["github.com".to_string(), "api.github.com".to_string()],
            "ghp_actual_real_secret_token_12345",
            "Authorization",
            "Bearer {}",
        );

        assert!(cred.matches_host("api.github.com"));
        assert!(cred.matches_host("api.github.com:443"));
        assert!(cred.matches_host("raw.github.com"));
        assert!(!cred.matches_host("evil-attacker.com"));
    }

    #[test]
    fn test_substitute_request_secrets_in_body_and_headers() {
        let broker = CredentialBroker::new();
        let cred = BrokeredCredential::new(
            "openai",
            vec!["api.openai.com".to_string()],
            "sk-proj-real_secret_key_12345",
            "Authorization",
            "Bearer {}",
        );
        let dummy = cred.dummy_token.clone();
        broker.register(cred);

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", dummy));

        let mut body = format!("{{\"model\":\"gpt-4o\",\"key\":\"{}\"}}", dummy).into_bytes();

        let modified = broker.substitute_request_secrets("api.openai.com", &mut headers, &mut body);
        assert!(modified);

        // Header substituted
        assert_eq!(
            headers.get("Authorization").unwrap(),
            "Bearer sk-proj-real_secret_key_12345"
        );

        // Body substituted
        let body_str = String::from_utf8(body).unwrap();
        assert!(!body_str.contains(&dummy));
        assert!(body_str.contains("sk-proj-real_secret_key_12345"));
    }

    #[test]
    fn test_sanitize_log() {
        let broker = CredentialBroker::new();
        let cred = BrokeredCredential::new(
            "aws",
            vec!["amazonaws.com".to_string()],
            "AKIAIOSFODNN7EXAMPLE",
            "X-Amz-Security-Token",
            "{}",
        );
        broker.register(cred);

        let log = "Connecting with access key AKIAIOSFODNN7EXAMPLE to us-east-1";
        let sanitized = broker.sanitize_log(log);
        assert!(!sanitized.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(sanitized.contains("[BROKER_REDACTED_CREDENTIAL]"));
    }
}
