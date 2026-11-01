//! Fine-Grained URL Path and HTTP Method Authorization Policy.
//!
//! Restricts outbound HTTP/HTTPS requests traversing the proxy by matching destination
//! domain, allowed HTTP methods, and URL path prefixes, preventing untrusted processes
//! from invoking sensitive admin, credential, or deployment APIs.

use serde::{Deserialize, Serialize};

/// Decision outcome for path authorization check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathAuthDecision {
    Allow,
    DenyMethodNotAllowed {
        method: String,
        allowed: Vec<String>,
    },
    DenyPathExplicit {
        path: String,
        matched_denial: String,
    },
    DenyPathNotWhitelisted {
        path: String,
    },
    NoRuleMatched,
}

/// Authorization policy rule for a specific domain pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathAuthorizationRule {
    pub domain_glob: String,
    pub allowed_methods: Vec<String>,
    pub allowed_path_prefixes: Vec<String>,
    pub denied_path_prefixes: Vec<String>,
}

impl PathAuthorizationRule {
    pub fn new(domain_glob: impl Into<String>) -> Self {
        Self {
            domain_glob: domain_glob.into(),
            allowed_methods: Vec::new(),
            allowed_path_prefixes: Vec::new(),
            denied_path_prefixes: Vec::new(),
        }
    }

    pub fn with_method(mut self, method: &str) -> Self {
        self.allowed_methods.push(method.to_ascii_uppercase());
        self
    }

    pub fn with_allowed_prefix(mut self, prefix: &str) -> Self {
        self.allowed_path_prefixes.push(prefix.to_string());
        self
    }

    pub fn with_denied_prefix(mut self, prefix: &str) -> Self {
        self.denied_path_prefixes.push(prefix.to_string());
        self
    }
}

/// Evaluator for fine-grained path authorization rules.
#[derive(Debug, Clone, Default)]
pub struct PathAuthorizer {
    rules: Vec<PathAuthorizationRule>,
}

impl PathAuthorizer {
    pub fn new(rules: Vec<PathAuthorizationRule>) -> Self {
        Self { rules }
    }

    /// Authorize an outbound HTTP request.
    pub fn authorize(&self, domain: &str, method: &str, path: &str) -> PathAuthDecision {
        let upper_method = method.to_ascii_uppercase();

        for rule in &self.rules {
            if !Self::matches_domain(&rule.domain_glob, domain) {
                continue;
            }

            // Check denied path prefixes first (deny takes precedence)
            for denied in &rule.denied_path_prefixes {
                if path.starts_with(denied) {
                    return PathAuthDecision::DenyPathExplicit {
                        path: path.to_string(),
                        matched_denial: denied.clone(),
                    };
                }
            }

            // Check allowed methods if configured
            if !rule.allowed_methods.is_empty() && !rule.allowed_methods.contains(&upper_method) {
                return PathAuthDecision::DenyMethodNotAllowed {
                    method: upper_method,
                    allowed: rule.allowed_methods.clone(),
                };
            }

            // Check allowed path prefixes if configured
            if !rule.allowed_path_prefixes.is_empty() {
                let matches_allowed = rule
                    .allowed_path_prefixes
                    .iter()
                    .any(|allowed| path.starts_with(allowed));
                if !matches_allowed {
                    return PathAuthDecision::DenyPathNotWhitelisted {
                        path: path.to_string(),
                    };
                }
            }

            return PathAuthDecision::Allow;
        }

        PathAuthDecision::NoRuleMatched
    }

    fn matches_domain(pattern: &str, domain: &str) -> bool {
        if pattern == "*" || pattern == domain {
            return true;
        }

        if let Some(suffix) = pattern.strip_prefix("*.") {
            domain.ends_with(suffix)
                && (domain.len() == suffix.len()
                    || domain.as_bytes()[domain.len() - suffix.len() - 1] == b'.')
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_authorization_allow_and_method_restriction() {
        let rule = PathAuthorizationRule::new("api.github.com")
            .with_method("GET")
            .with_method("HEAD")
            .with_allowed_prefix("/repos/")
            .with_denied_prefix("/repos/secret/");

        let authorizer = PathAuthorizer::new(vec![rule]);

        // GET /repos/owner/repo -> Allowed
        assert_eq!(
            authorizer.authorize("api.github.com", "GET", "/repos/owner/repo"),
            PathAuthDecision::Allow
        );

        // POST /repos/owner/repo -> Denied method
        let res_post = authorizer.authorize("api.github.com", "POST", "/repos/owner/repo");
        assert!(matches!(
            res_post,
            PathAuthDecision::DenyMethodNotAllowed { .. }
        ));

        // GET /repos/secret/keys -> Denied path explicit
        let res_denied = authorizer.authorize("api.github.com", "GET", "/repos/secret/keys");
        assert!(matches!(
            res_denied,
            PathAuthDecision::DenyPathExplicit { .. }
        ));

        // GET /user -> Denied not whitelisted
        let res_other = authorizer.authorize("api.github.com", "GET", "/user");
        assert!(matches!(
            res_other,
            PathAuthDecision::DenyPathNotWhitelisted { .. }
        ));

        // Different domain -> No rule matched
        assert_eq!(
            authorizer.authorize("example.com", "GET", "/repos/owner/repo"),
            PathAuthDecision::NoRuleMatched
        );
    }
}
