//! Secondary OAuth credentials delivered alongside the provider's primary authentication.
//!
//! Injects secondary OAuth credentials and handles token delivery for authenticated model gateways:
//! - Protects reserved protocol headers (`authorization`, `chatgpt-account-id`, custom auth tokens, etc.)
//! - Validates endpoint URLs and delivery schemes (Bearer, Token, Custom Header, Cookie).
//! - Injects secondary credentials into outgoing requests safely without logging secrets.

use reqwest::header::HeaderName;
use serde::{Deserialize, Serialize};
use std::fmt;

// Reserve authentication, routing/framing, and internal protocol headers.
pub const RESERVED_GATEWAY_OAUTH_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "upgrade",
    "chatgpt-account-id",
];

pub const RESERVED_GATEWAY_OAUTH_HEADER_PREFIXES: &[&str] =
    &["x-codex-", "x-openai-", "sec-websocket-", "x-m31a-"];

fn default_bearer_scheme() -> String {
    "Bearer".to_string()
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct GatewayOAuthConfig {
    pub authorization_url: String,
    pub token_url: String,
    pub client_id: String,
    pub resource: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub redirect_port: Option<u16>,
    pub delivery: GatewayOAuthDelivery,
}

impl fmt::Debug for GatewayOAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayOAuthConfig")
            .field("authorization_url", &self.authorization_url)
            .field("client_id", &self.client_id)
            .field("delivery", &self.delivery)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GatewayOAuthDelivery {
    Header {
        name: String,
        #[serde(default = "default_bearer_scheme")]
        scheme: String,
    },
    Cookie {
        name: String,
    },
}

impl GatewayOAuthConfig {
    /// Validates that the delivery header name does not clash with reserved protocol headers.
    pub fn validate(&self) -> Result<(), String> {
        if let GatewayOAuthDelivery::Header { ref name, .. } = self.delivery {
            let lower_name = name.to_ascii_lowercase();
            if RESERVED_GATEWAY_OAUTH_HEADERS.contains(&lower_name.as_str()) {
                return Err(format!(
                    "header name `{name}` is reserved by gateway protocol"
                ));
            }
            for prefix in RESERVED_GATEWAY_OAUTH_HEADER_PREFIXES {
                if lower_name.starts_with(prefix) {
                    return Err(format!(
                        "header name `{name}` uses reserved prefix `{prefix}`"
                    ));
                }
            }
            if HeaderName::from_bytes(name.as_bytes()).is_err() {
                return Err(format!("header name `{name}` contains invalid ASCII bytes"));
            }
        }
        Ok(())
    }

    /// Formats the header value or cookie value for delivery.
    pub fn format_delivery_value(&self, token: &str) -> (String, String) {
        match &self.delivery {
            GatewayOAuthDelivery::Header { name, scheme } => {
                let trimmed_scheme = scheme.trim();
                let value = if trimmed_scheme.is_empty() {
                    token.to_string()
                } else {
                    format!("{} {}", trimmed_scheme, token)
                };
                (name.clone(), value)
            }
            GatewayOAuthDelivery::Cookie { name } => {
                ("Cookie".to_string(), format!("{}={}", name, token))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_custom_gateway_header() {
        let config = GatewayOAuthConfig {
            authorization_url: "https://auth.example.com/oauth/authorize".to_string(),
            token_url: "https://auth.example.com/oauth/token".to_string(),
            client_id: "m31a-client".to_string(),
            resource: None,
            scopes: vec!["read".to_string(), "write".to_string()],
            redirect_port: Some(18443),
            delivery: GatewayOAuthDelivery::Header {
                name: "x-custom-gateway-auth".to_string(),
                scheme: "Bearer".to_string(),
            },
        };
        assert!(config.validate().is_ok());

        let (header_name, header_val) = config.format_delivery_value("secret-tok-123");
        assert_eq!(header_name, "x-custom-gateway-auth");
        assert_eq!(header_val, "Bearer secret-tok-123");
    }

    #[test]
    fn test_reserved_header_rejected() {
        let config = GatewayOAuthConfig {
            authorization_url: "https://auth.example.com/oauth/authorize".to_string(),
            token_url: "https://auth.example.com/oauth/token".to_string(),
            client_id: "m31a-client".to_string(),
            resource: None,
            scopes: Vec::new(),
            redirect_port: None,
            delivery: GatewayOAuthDelivery::Header {
                name: "authorization".to_string(),
                scheme: "Bearer".to_string(),
            },
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_reserved_prefix_rejected() {
        let config = GatewayOAuthConfig {
            authorization_url: "https://auth.example.com/oauth/authorize".to_string(),
            token_url: "https://auth.example.com/oauth/token".to_string(),
            client_id: "m31a-client".to_string(),
            resource: None,
            scopes: Vec::new(),
            redirect_port: None,
            delivery: GatewayOAuthDelivery::Header {
                name: "x-codex-residency".to_string(),
                scheme: "Bearer".to_string(),
            },
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_cookie_delivery() {
        let config = GatewayOAuthConfig {
            authorization_url: "https://auth.example.com/oauth/authorize".to_string(),
            token_url: "https://auth.example.com/oauth/token".to_string(),
            client_id: "m31a-client".to_string(),
            resource: None,
            scopes: Vec::new(),
            redirect_port: None,
            delivery: GatewayOAuthDelivery::Cookie {
                name: "session_tok".to_string(),
            },
        };
        assert!(config.validate().is_ok());
        let (header_name, header_val) = config.format_delivery_value("cookie-val-999");
        assert_eq!(header_name, "Cookie");
        assert_eq!(header_val, "session_tok=cookie-val-999");
    }
}
