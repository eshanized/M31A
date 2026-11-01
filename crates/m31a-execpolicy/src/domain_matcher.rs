//! Domain and Network Security Matcher
//!
//! Evaluates candidate outbound network destinations (host, IP, port, protocol) against
//! wildcard domain allowlists and cloud metadata SSRF blacklists.
//! Evaluates candidate outbound network destinations (host, IP, port, protocol) against
//! wildcard domain allowlists and cloud metadata SSRF blacklists.

use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPattern {
    Exact(String),
    Subdomain(String),        // e.g. *.example.com (subdomains only)
    ApexAndSubdomain(String), // e.g. **.example.com (apex + subdomains)
    Wildcard,                 // *
}

impl DomainPattern {
    pub fn parse(pattern: &str) -> Self {
        let trimmed = pattern.trim().to_lowercase();
        if trimmed == "*" {
            DomainPattern::Wildcard
        } else if let Some(suffix) = trimmed.strip_prefix("**.") {
            DomainPattern::ApexAndSubdomain(suffix.to_string())
        } else if let Some(suffix) = trimmed.strip_prefix("*.") {
            DomainPattern::Subdomain(suffix.to_string())
        } else {
            DomainPattern::Exact(trimmed)
        }
    }

    pub fn matches(&self, candidate: &str) -> bool {
        let cand = candidate.trim().to_lowercase();

        match self {
            DomainPattern::Wildcard => true,
            DomainPattern::Exact(expected) => expected == &cand,
            DomainPattern::Subdomain(suffix) => {
                if cand == suffix.as_str() {
                    false
                } else if let Some(prefix) = cand.strip_suffix(suffix.as_str()) {
                    prefix.ends_with('.')
                } else {
                    false
                }
            }
            DomainPattern::ApexAndSubdomain(suffix) => {
                if cand == suffix.as_str() {
                    true
                } else if let Some(prefix) = cand.strip_suffix(suffix.as_str()) {
                    prefix.ends_with('.')
                } else {
                    false
                }
            }
        }
    }
}

pub struct NetworkSecurityFilter;

impl NetworkSecurityFilter {
    /// Detects cloud metadata endpoints (SSRF risk for credentials exfiltration)
    pub fn is_cloud_metadata_service(host: &str) -> bool {
        let clean = host.trim().trim_start_matches('[').trim_end_matches(']');
        if clean == "169.254.169.254"
            || clean == "fd00:ec2::254"
            || clean == "metadata.google.internal"
        {
            return true;
        }
        if let Ok(ip) = clean.parse::<IpAddr>() {
            match ip {
                IpAddr::V4(v4) => {
                    // Link-local 169.254.0.0/16
                    v4.is_link_local()
                }
                IpAddr::V6(v6) => {
                    // IPv6 link-local fe80::/10
                    (v6.segments()[0] & 0xffc0) == 0xfe80
                }
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_pattern_matching() {
        let apex_sub = DomainPattern::parse("**.nvidia.com");
        assert!(apex_sub.matches("nvidia.com"));
        assert!(apex_sub.matches("api.nvidia.com"));
        assert!(apex_sub.matches("integrate.api.nvidia.com"));
        assert!(!apex_sub.matches("othernvidia.com"));
        assert!(!apex_sub.matches("fake-nvidia.com"));

        let sub_only = DomainPattern::parse("*.github.com");
        assert!(!sub_only.matches("github.com"));
        assert!(sub_only.matches("api.github.com"));
        assert!(sub_only.matches("gist.github.com"));

        let exact = DomainPattern::parse("crates.io");
        assert!(exact.matches("crates.io"));
        assert!(!exact.matches("static.crates.io"));
    }

    #[test]
    fn test_cloud_metadata_ssrf_blocking() {
        assert!(NetworkSecurityFilter::is_cloud_metadata_service(
            "169.254.169.254"
        ));
        assert!(NetworkSecurityFilter::is_cloud_metadata_service(
            "metadata.google.internal"
        ));
        assert!(NetworkSecurityFilter::is_cloud_metadata_service(
            "169.254.1.1"
        ));
        assert!(!NetworkSecurityFilter::is_cloud_metadata_service("8.8.8.8"));
        assert!(!NetworkSecurityFilter::is_cloud_metadata_service(
            "integrate.api.nvidia.com"
        ));
    }
}
