//! Transparent Outbound Sandbox Network Proxy & Domain Firewall.
//!
//! Network egress proxy enforcing domain and IP allowlists for sandboxed processes:
//! - Blocks SSRF against private subnets (RFC 1918) and cloud metadata IP (169.254.169.254).
//! - Evaluates hostname/domain allowlists with wildcard support (`*.github.com`).
//! - Logs audit entries for all connection decisions.
//! - Exports proxy environment variables (`HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`).

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Action decided for an outbound connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyDecision {
    Allow,
    Block,
    RequireApproval,
}

/// Rule for filtering outbound network requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkFirewallRule {
    pub pattern: String,
    pub decision: ProxyDecision,
    pub reason: String,
}

/// Audit log record for an intercepted or evaluated connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionAuditRecord {
    pub timestamp_epoch_secs: u64,
    pub destination_host: String,
    pub destination_port: u16,
    pub decision: ProxyDecision,
    pub matched_rule: Option<String>,
    pub reason: String,
}

/// Outbound network proxy firewall and access evaluator.
#[derive(Debug, Clone)]
pub struct NetworkProxyFirewall {
    rules: Arc<Mutex<Vec<NetworkFirewallRule>>>,
    audit_log: Arc<Mutex<Vec<ConnectionAuditRecord>>>,
    block_private_ips: bool,
    block_cloud_metadata: bool,
}

impl Default for NetworkProxyFirewall {
    fn default() -> Self {
        Self::new(true, true)
    }
}

impl NetworkProxyFirewall {
    /// Creates a new proxy firewall.
    pub fn new(block_private_ips: bool, block_cloud_metadata: bool) -> Self {
        let default_rules = vec![
            NetworkFirewallRule {
                pattern: "github.com".to_string(),
                decision: ProxyDecision::Allow,
                reason: "Git repository access".to_string(),
            },
            NetworkFirewallRule {
                pattern: "*.github.com".to_string(),
                decision: ProxyDecision::Allow,
                reason: "GitHub services".to_string(),
            },
            NetworkFirewallRule {
                pattern: "crates.io".to_string(),
                decision: ProxyDecision::Allow,
                reason: "Rust package registry".to_string(),
            },
            NetworkFirewallRule {
                pattern: "*.crates.io".to_string(),
                decision: ProxyDecision::Allow,
                reason: "Rust package CDN".to_string(),
            },
            NetworkFirewallRule {
                pattern: "registry.npmjs.org".to_string(),
                decision: ProxyDecision::Allow,
                reason: "Node.js package registry".to_string(),
            },
            NetworkFirewallRule {
                pattern: "pypi.org".to_string(),
                decision: ProxyDecision::Allow,
                reason: "Python package index".to_string(),
            },
            NetworkFirewallRule {
                pattern: "files.pythonhosted.org".to_string(),
                decision: ProxyDecision::Allow,
                reason: "Python package downloads".to_string(),
            },
            NetworkFirewallRule {
                pattern: "*.api.nvidia.com".to_string(),
                decision: ProxyDecision::Allow,
                reason: "NVIDIA NIM endpoints".to_string(),
            },
            NetworkFirewallRule {
                pattern: "integrate.api.nvidia.com".to_string(),
                decision: ProxyDecision::Allow,
                reason: "NVIDIA NIM inference API".to_string(),
            },
        ];

        Self {
            rules: Arc::new(Mutex::new(default_rules)),
            audit_log: Arc::new(Mutex::new(Vec::new())),
            block_private_ips,
            block_cloud_metadata,
        }
    }

    /// Adds a custom firewall rule with highest evaluation priority (prepended).
    pub fn add_rule(&self, rule: NetworkFirewallRule) {
        let mut rules = self.rules.lock().unwrap();
        rules.insert(0, rule);
    }

    /// Evaluates whether an outbound connection to `host:port` is permitted.
    pub fn evaluate_connection(&self, host: &str, port: u16) -> (ProxyDecision, String) {
        let clean_host = host.trim().to_lowercase();

        // 1. Check IP-based restrictions
        if let Ok(ip) = clean_host.parse::<IpAddr>()
            && let Some(decision) = self.check_ip_restrictions(ip)
        {
            self.record_audit(&clean_host, port, decision.0, None, &decision.1);
            return decision;
        }

        // 2. Evaluate pattern rules
        let rules = self.rules.lock().unwrap();
        for r in rules.iter() {
            if Self::matches_pattern(&clean_host, &r.pattern) {
                self.record_audit(&clean_host, port, r.decision, Some(&r.pattern), &r.reason);
                return (r.decision, r.reason.clone());
            }
        }

        // 3. Default fallback: deny or require approval for unknown hosts
        let fallback_decision = ProxyDecision::RequireApproval;
        let fallback_reason = format!(
            "Destination host '{}' is not in network allowlist",
            clean_host
        );
        self.record_audit(&clean_host, port, fallback_decision, None, &fallback_reason);
        (fallback_decision, fallback_reason)
    }

    fn check_ip_restrictions(&self, ip: IpAddr) -> Option<(ProxyDecision, String)> {
        match ip {
            IpAddr::V4(ipv4) => {
                // Cloud metadata IP: 169.254.169.254
                if self.block_cloud_metadata && ipv4 == Ipv4Addr::new(169, 254, 169, 254) {
                    return Some((
                        ProxyDecision::Block,
                        "Blocked access to cloud instance metadata endpoint (169.254.169.254)"
                            .to_string(),
                    ));
                }

                // Loopback 127.0.0.0/8
                if ipv4.is_loopback() {
                    return Some((
                        ProxyDecision::Block,
                        "Blocked access to local host loopback (127.0.0.1)".to_string(),
                    ));
                }

                // Private RFC 1918 subnets
                if self.block_private_ips && ipv4.is_private() {
                    return Some((
                        ProxyDecision::Block,
                        format!("Blocked access to private network subnet IP: {}", ipv4),
                    ));
                }

                // Link-local 169.254.0.0/16
                if self.block_cloud_metadata && ipv4.is_link_local() {
                    return Some((
                        ProxyDecision::Block,
                        format!("Blocked access to link-local address: {}", ipv4),
                    ));
                }
            }
            IpAddr::V6(ipv6) => {
                if ipv6.is_loopback() {
                    return Some((
                        ProxyDecision::Block,
                        "Blocked access to IPv6 loopback (::1)".to_string(),
                    ));
                }
            }
        }
        None
    }

    fn matches_pattern(host: &str, pattern: &str) -> bool {
        let pat = pattern.trim().to_lowercase();
        if pat == "*" {
            return true;
        }
        if pat.starts_with("*.") {
            let suffix = &pat[1..]; // e.g. ".github.com"
            return host.ends_with(suffix);
        }
        host == pat
    }

    fn record_audit(
        &self,
        host: &str,
        port: u16,
        decision: ProxyDecision,
        rule: Option<&str>,
        reason: &str,
    ) {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let record = ConnectionAuditRecord {
            timestamp_epoch_secs: ts,
            destination_host: host.to_string(),
            destination_port: port,
            decision,
            matched_rule: rule.map(|s| s.to_string()),
            reason: reason.to_string(),
        };

        if let Ok(mut log) = self.audit_log.lock() {
            log.push(record);
        }
    }

    /// Retrieve copy of audit ledger.
    pub fn audit_records(&self) -> Vec<ConnectionAuditRecord> {
        self.audit_log.lock().unwrap().clone()
    }

    /// Build standard proxy environment variables to redirect sandboxed commands to local proxy.
    pub fn build_proxy_environment(proxy_port: u16) -> Vec<(String, String)> {
        let proxy_url = format!("http://127.0.0.1:{}", proxy_port);
        vec![
            ("HTTP_PROXY".to_string(), proxy_url.clone()),
            ("HTTPS_PROXY".to_string(), proxy_url.clone()),
            ("http_proxy".to_string(), proxy_url.clone()),
            ("https_proxy".to_string(), proxy_url),
            ("NO_PROXY".to_string(), "127.0.0.1,localhost".to_string()),
            ("no_proxy".to_string(), "127.0.0.1,localhost".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_default_whitelisted_domains() {
        let firewall = NetworkProxyFirewall::default();

        let (dec, _) = firewall.evaluate_connection("github.com", 443);
        assert_eq!(dec, ProxyDecision::Allow);

        let (dec, _) = firewall.evaluate_connection("api.github.com", 443);
        assert_eq!(dec, ProxyDecision::Allow);

        let (dec, _) = firewall.evaluate_connection("crates.io", 443);
        assert_eq!(dec, ProxyDecision::Allow);

        let (dec, _) = firewall.evaluate_connection("integrate.api.nvidia.com", 443);
        assert_eq!(dec, ProxyDecision::Allow);
    }

    #[test]
    fn test_block_cloud_metadata_ip() {
        let firewall = NetworkProxyFirewall::default();
        let (dec, reason) = firewall.evaluate_connection("169.254.169.254", 80);
        assert_eq!(dec, ProxyDecision::Block);
        assert!(reason.contains("cloud instance metadata"));
    }

    #[test]
    fn test_block_private_subnets_and_loopback() {
        let firewall = NetworkProxyFirewall::default();

        let (dec, _) = firewall.evaluate_connection("127.0.0.1", 8080);
        assert_eq!(dec, ProxyDecision::Block);

        let (dec, _) = firewall.evaluate_connection("192.168.1.1", 80);
        assert_eq!(dec, ProxyDecision::Block);

        let (dec, _) = firewall.evaluate_connection("10.0.0.5", 443);
        assert_eq!(dec, ProxyDecision::Block);
    }

    #[test]
    fn test_require_approval_for_unknown_domain() {
        let firewall = NetworkProxyFirewall::default();
        let (dec, _) = firewall.evaluate_connection("untrusted-exfil-server.com", 443);
        assert_eq!(dec, ProxyDecision::RequireApproval);
    }

    #[test]
    fn test_custom_rule_addition() {
        let firewall = NetworkProxyFirewall::default();
        firewall.add_rule(NetworkFirewallRule {
            pattern: "*.internal-corp.net".to_string(),
            decision: ProxyDecision::Allow,
            reason: "Internal company intranet".to_string(),
        });

        let (dec, reason) = firewall.evaluate_connection("gitlab.internal-corp.net", 443);
        assert_eq!(dec, ProxyDecision::Allow);
        assert_eq!(reason, "Internal company intranet");

        let audits = firewall.audit_records();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].destination_host, "gitlab.internal-corp.net");
    }

    #[test]
    fn test_proxy_env_vars() {
        let vars = NetworkProxyFirewall::build_proxy_environment(8080);
        assert!(
            vars.iter()
                .any(|(k, v)| k == "HTTP_PROXY" && v == "http://127.0.0.1:8080")
        );
        assert!(
            vars.iter()
                .any(|(k, v)| k == "NO_PROXY" && v == "127.0.0.1,localhost")
        );
    }
}
