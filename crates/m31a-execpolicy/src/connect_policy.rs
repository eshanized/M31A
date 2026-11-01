//! Network destination connection evaluation and SSRF prevention policy.
//!
//! Evaluates TCP connection targets against domain allowlists, denylists, review policies,
//! and RFC IP classification rules.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectDecision {
    Allowed,
    RequiresReview(String),
    Denied(String),
}

/// Destination connect policy controlling outbound proxy connections.
#[derive(Debug, Clone, Default)]
pub struct ConnectPolicy {
    /// Whether connections to non-public / loopback addresses are permitted by default.
    pub allow_local_binding: bool,
    /// Set of domains or IPs explicitly allowed.
    pub allowed_domains: HashSet<String>,
    /// Set of domains or IPs explicitly denied.
    pub denied_domains: HashSet<String>,
    /// Set of domains or IPs requiring interactive user approval.
    pub review_domains: HashSet<String>,
}

impl ConnectPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_allow_local_binding(mut self, allow: bool) -> Self {
        self.allow_local_binding = allow;
        self
    }

    pub fn allow_domain(&mut self, domain: impl Into<String>) {
        self.allowed_domains
            .insert(domain.into().to_ascii_lowercase());
    }

    pub fn deny_domain(&mut self, domain: impl Into<String>) {
        self.denied_domains
            .insert(domain.into().to_ascii_lowercase());
    }

    pub fn review_domain(&mut self, domain: impl Into<String>) {
        self.review_domains
            .insert(domain.into().to_ascii_lowercase());
    }

    /// Evaluates a target destination host, port, and resolved IP against the policy.
    pub fn evaluate_destination(
        &self,
        host: &str,
        port: u16,
        resolved_ip: Option<IpAddr>,
    ) -> ConnectDecision {
        let host_lower = host.trim().trim_end_matches('.').to_ascii_lowercase();

        // 1. Check explicit denylist first
        if self.is_in_set(&self.denied_domains, &host_lower, resolved_ip) {
            return ConnectDecision::Denied(format!(
                "destination `{host}:{port}` explicitly denied by network policy"
            ));
        }

        // 2. Identify if target is non-public (localhost, loopback, private RFC1918/CGNAT)
        let is_non_public = is_non_public_target(&host_lower, resolved_ip);

        if is_non_public {
            // If local binding is NOT enabled, only allow if explicitly allowlisted
            if !self.allow_local_binding {
                if !self.is_in_set(&self.allowed_domains, &host_lower, resolved_ip) {
                    return ConnectDecision::Denied(format!(
                        "non-public target `{host}:{port}` rejected by policy (allow_local_binding is false)"
                    ));
                }
            } else {
                // Local binding enabled: check if it falls under review domains
                if self.is_in_set(&self.review_domains, &host_lower, resolved_ip) {
                    return ConnectDecision::RequiresReview(format!(
                        "non-public destination `{host}:{port}` requires confirmation"
                    ));
                }
            }
        }

        // 3. Check review domains
        if self.is_in_set(&self.review_domains, &host_lower, resolved_ip) {
            return ConnectDecision::RequiresReview(format!(
                "destination `{host}:{port}` is configured for explicit confirmation"
            ));
        }

        // 4. Check allowed domains or default permit
        if self.allowed_domains.is_empty()
            || self.is_in_set(&self.allowed_domains, &host_lower, resolved_ip)
        {
            ConnectDecision::Allowed
        } else {
            // When an explicit allowlist is configured and target is not in it
            ConnectDecision::Denied(format!(
                "destination `{host}:{port}` is not in the configured network allowlist"
            ))
        }
    }

    fn is_in_set(&self, set: &HashSet<String>, host: &str, ip: Option<IpAddr>) -> bool {
        if set.contains(host) {
            return true;
        }

        if let Some(ip) = ip {
            let ip_str = ip.to_string();
            if set.contains(&ip_str) {
                return true;
            }
        }

        // Subdomain matching: if set contains "example.com", then "api.example.com" matches
        for entry in set {
            if let Some(suffix) = entry.strip_prefix("*.") {
                if host == suffix || host.ends_with(&format!(".{suffix}")) {
                    return true;
                }
            } else if host.ends_with(&format!(".{entry}")) {
                return true;
            }
        }

        false
    }
}

/// Returns true if the host name or resolved IP is a non-public target (SSRF prevention).
pub fn is_non_public_target(host: &str, ip: Option<IpAddr>) -> bool {
    let clean = host.trim().trim_end_matches('.');
    if clean.eq_ignore_ascii_case("localhost") || clean.ends_with(".localhost") {
        return true;
    }

    if let Ok(parsed_ip) = clean.parse::<IpAddr>() {
        return is_non_public_ip(parsed_ip);
    }

    if let Some(ip) = ip {
        return is_non_public_ip(ip);
    }

    false
}

/// Checks whether an IP address is non-public per RFC definitions:
/// loopback, private (RFC 1918), link-local, unspecified, multicast, broadcast,
/// CGNAT (RFC 6598), TEST-NET (RFC 5737), Benchmarking (RFC 2544), Reserved.
pub fn is_non_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_non_public_ipv4(v4),
        IpAddr::V6(v6) => is_non_public_ipv6(v6),
    }
}

fn is_non_public_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ipv4_in_cidr(ip, [0, 0, 0, 0], 8)       // "this network" (RFC 1122)
        || ipv4_in_cidr(ip, [100, 64, 0, 0], 10)   // CGNAT (RFC 6598)
        || ipv4_in_cidr(ip, [192, 0, 0, 0], 24)    // IETF Protocol Assignments (RFC 6890)
        || ipv4_in_cidr(ip, [192, 0, 2, 0], 24)    // TEST-NET-1 (RFC 5737)
        || ipv4_in_cidr(ip, [198, 18, 0, 0], 15)   // Benchmarking (RFC 2544)
        || ipv4_in_cidr(ip, [198, 51, 100, 0], 24) // TEST-NET-2 (RFC 5737)
        || ipv4_in_cidr(ip, [203, 0, 113, 0], 24)  // TEST-NET-3 (RFC 5737)
        || ipv4_in_cidr(ip, [240, 0, 0, 0], 4) // Reserved (RFC 6890)
}

fn is_non_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4() {
        return is_non_public_ipv4(v4) || ip.is_loopback();
    }
    // Unique local (fc00::/7), link-local (fe80::/10), loopback, unspecified, multicast
    let segments = ip.segments();
    let is_unique_local = (segments[0] & 0xfe00) == 0xfc00;
    let is_unicast_link_local = (segments[0] & 0xffc0) == 0xfe80;

    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || is_unique_local
        || is_unicast_link_local
}

/// Checks whether an IP address belongs to an RFC 1918 private unicast or CGNAT block.
pub fn is_private_network_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || ipv4_in_cidr(v4, [100, 64, 0, 0], 10),
        IpAddr::V6(v6) => match v6.to_ipv4() {
            Some(v4) => is_private_network_ip(IpAddr::V4(v4)),
            None => (v6.segments()[0] & 0xfe00) == 0xfc00,
        },
    }
}

fn ipv4_in_cidr(ip: Ipv4Addr, base: [u8; 4], prefix: u8) -> bool {
    let ip = u32::from(ip);
    let base = u32::from(Ipv4Addr::from(base));
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (ip & mask) == (base & mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_non_public_ipv4_classification() {
        assert!(is_non_public_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_non_public_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 1, 25))));
        assert!(is_non_public_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_non_public_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_non_public_ip(IpAddr::V4(Ipv4Addr::new(
            169, 254, 169, 254
        )))); // Link local
        assert!(is_non_public_ip(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)))); // CGNAT
        assert!(is_non_public_ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)))); // TEST-NET-1
        assert!(is_non_public_ip(IpAddr::V4(Ipv4Addr::new(240, 1, 2, 3)))); // Reserved

        // Public IPs
        assert!(!is_non_public_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_non_public_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_non_public_ip(IpAddr::V4(Ipv4Addr::new(
            140, 82, 121, 4
        ))));
    }

    #[test]
    fn test_non_public_ipv6_classification() {
        assert!(is_non_public_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_non_public_ip(IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
        // fc00::/7 unique local
        assert!(is_non_public_ip(IpAddr::V6(
            "fd12:3456:789a:1::1".parse().unwrap()
        )));
        // fe80::/10 link local
        assert!(is_non_public_ip(IpAddr::V6("fe80::1".parse().unwrap())));
    }

    #[test]
    fn test_connect_policy_denies_local_target_when_binding_disabled() {
        let policy = ConnectPolicy::default(); // allow_local_binding is false
        let decision =
            policy.evaluate_destination("127.0.0.1", 8080, Some(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(matches!(decision, ConnectDecision::Denied(_)));

        let decision = policy.evaluate_destination("localhost", 3000, None);
        assert!(matches!(decision, ConnectDecision::Denied(_)));
    }

    #[test]
    fn test_connect_policy_allows_local_target_when_binding_enabled() {
        let policy = ConnectPolicy::default().with_allow_local_binding(true);
        let decision =
            policy.evaluate_destination("127.0.0.1", 8080, Some(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert_eq!(decision, ConnectDecision::Allowed);
    }

    #[test]
    fn test_connect_policy_explicit_allowlist_local_target() {
        let mut policy = ConnectPolicy::default();
        policy.allow_domain("127.0.0.1");
        let decision =
            policy.evaluate_destination("127.0.0.1", 8080, Some(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert_eq!(decision, ConnectDecision::Allowed);
    }

    #[test]
    fn test_connect_policy_explicit_denylist_takes_precedence() {
        let mut policy = ConnectPolicy::default().with_allow_local_binding(true);
        policy.deny_domain("api.untrusted.com");
        let decision = policy.evaluate_destination("api.untrusted.com", 443, None);
        assert!(matches!(decision, ConnectDecision::Denied(_)));
    }

    #[test]
    fn test_connect_policy_review_domains() {
        let mut policy = ConnectPolicy::default();
        policy.review_domain("staging.internal");
        let decision = policy.evaluate_destination("sub.staging.internal", 443, None);
        assert!(matches!(decision, ConnectDecision::RequiresReview(_)));
    }
}
