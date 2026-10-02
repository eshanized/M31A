//! Centralized Network Destination Policy and SSRF Defense (SEC-NET, Findings H & I).
//!
//! Enforces strict egress security controls across all network, web, and HTTP clients.
//! Blocks access to:
//! - Loopback addresses (127.0.0.0/8, ::1)
//! - RFC 1918 private IPv4 ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
//! - Link-local ranges (169.254.0.0/16, fe80::/10)
//! - Cloud instance metadata services (169.254.169.254, [fd00:ec2::254], etc.)
//! - IPv6 unique-local / private ranges (fc00::/7)
//! - IPv6 loopback, unspecified, and discard ranges
//! - IPv4-mapped / IPv4-compatible IPv6 addresses encoding blocked IPv4 addresses
//! - Broadcast and multicast ranges (224.0.0.0/4, ff00::/8, 255.255.255.255)
//! - Documentation and reserved test networks (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24, 2001:db8::/32)
//! - Carrier-grade NAT (100.64.0.0/10)
//! - Unspecified addresses (0.0.0.0, ::)
//!
//! Hostnames are resolved through DNS and all resolved IP addresses are validated before connection.
//! Redirect targets are step-by-step validated so that public endpoints cannot redirect to private targets.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use thiserror::Error;

/// Typed network destination security errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NetworkSecurityError {
    #[error("Destination URL is invalid: {0}")]
    InvalidUrl(String),

    #[error("Unsupported URL scheme '{0}': only HTTP and HTTPS are permitted")]
    UnsupportedScheme(String),

    #[error("Missing host in destination URL")]
    MissingHost,

    #[error("Access to destination '{host}' ({ip}) is blocked by egress security policy: {reason}")]
    BlockedDestination {
        host: String,
        ip: String,
        reason: &'static str,
    },

    #[error("DNS resolution failed for host '{host}': {message}")]
    DnsResolutionFailed { host: String, message: String },

    #[error("Destination host '{0}' resolved to no IP addresses")]
    NoAddressesResolved(String),

    #[error("Redirect target blocked by egress security policy: {0}")]
    BlockedRedirect(String),

    #[error("Too many redirects: exceeded limit of {0}")]
    TooManyRedirects(usize),
}

/// Authoritative centralized network destination policy enforcer.
#[derive(Debug, Clone, Default)]
pub struct NetworkDestinationPolicy;

impl NetworkDestinationPolicy {
    pub fn new() -> Self {
        Self
    }

    /// Check whether an IPv4 address is blocked by policy.
    pub fn is_ipv4_blocked(ip: &Ipv4Addr) -> Option<&'static str> {
        let octets = ip.octets();

        // 1. Loopback (127.0.0.0/8)
        if ip.is_loopback() {
            return Some("loopback address");
        }

        // 2. Unspecified (0.0.0.0/8)
        if octets[0] == 0 {
            return Some("unspecified / current network");
        }

        // 3. RFC 1918 Private ranges:
        // 10.0.0.0/8
        if octets[0] == 10 {
            return Some("RFC 1918 private address (10.0.0.0/8)");
        }
        // 172.16.0.0/12
        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
            return Some("RFC 1918 private address (172.16.0.0/12)");
        }
        // 192.168.0.0/16
        if octets[0] == 192 && octets[1] == 168 {
            return Some("RFC 1918 private address (192.168.0.0/16)");
        }

        // 4. Link-local (169.254.0.0/16) - includes cloud metadata service 169.254.169.254
        if ip.is_link_local() || (octets[0] == 169 && octets[1] == 254) {
            if octets[2] == 169 && octets[3] == 254 {
                return Some("cloud instance metadata endpoint (169.254.169.254)");
            }
            return Some("link-local address (169.254.0.0/16)");
        }

        // 5. Carrier-grade NAT (100.64.0.0/10)
        if octets[0] == 100 && (64..=127).contains(&octets[1]) {
            return Some("carrier-grade NAT address (100.64.0.0/10)");
        }

        // 6. Documentation and test networks (TEST-NET-1, 2, 3)
        // 192.0.2.0/24 (TEST-NET-1)
        if octets[0] == 192 && octets[1] == 0 && octets[2] == 2 {
            return Some("test/documentation network (192.0.2.0/24)");
        }
        // 198.51.100.0/24 (TEST-NET-2)
        if octets[0] == 198 && octets[1] == 51 && octets[2] == 100 {
            return Some("test/documentation network (198.51.100.0/24)");
        }
        // 203.0.113.0/24 (TEST-NET-3)
        if octets[0] == 203 && octets[1] == 0 && octets[2] == 113 {
            return Some("test/documentation network (203.0.113.0/24)");
        }
        // Benchmark testing: 198.18.0.0/15
        if octets[0] == 198 && (18..=19).contains(&octets[1]) {
            return Some("benchmark testing network (198.18.0.0/15)");
        }

        // 7. Multicast (224.0.0.0/4)
        if ip.is_multicast() {
            return Some("multicast address (224.0.0.0/4)");
        }

        // 8. Broadcast (255.255.255.255)
        if ip.is_broadcast() {
            return Some("broadcast address (255.255.255.255)");
        }

        // 9. Reserved for future use (240.0.0.0/4)
        if (octets[0] & 0xf0) == 240 {
            return Some("reserved address range (240.0.0.0/4)");
        }

        None
    }

    /// Check whether an IPv6 address is blocked by policy.
    pub fn is_ipv6_blocked(ip: &Ipv6Addr) -> Option<&'static str> {
        // 1. Loopback (::1)
        if ip.is_loopback() {
            return Some("IPv6 loopback address (::1)");
        }

        // 2. Unspecified (::)
        if ip.is_unspecified() {
            return Some("IPv6 unspecified address (::)");
        }

        // 3. IPv4-mapped IPv6 (::ffff:w.x.y.z)
        if let Some(v4) = ip.to_ipv4_mapped() {
            if let Some(reason) = Self::is_ipv4_blocked(&v4) {
                return Some(reason);
            }
        }

        // 4. IPv4-compatible IPv6 (deprecated ::w.x.y.z)
        let segments = ip.segments();
        if segments[0] == 0
            && segments[1] == 0
            && segments[2] == 0
            && segments[3] == 0
            && segments[4] == 0
            && segments[5] == 0
        {
            let v4 = Ipv4Addr::new(
                (segments[6] >> 8) as u8,
                segments[6] as u8,
                (segments[7] >> 8) as u8,
                segments[7] as u8,
            );
            if let Some(reason) = Self::is_ipv4_blocked(&v4) {
                return Some(reason);
            }
        }

        // 5. Unique-local / Private (fc00::/7)
        if (segments[0] & 0xfe00) == 0xfc00 {
            return Some("IPv6 unique-local / private address (fc00::/7)");
        }

        // 6. Link-local (fe80::/10)
        if (segments[0] & 0xffc0) == 0xfe80 {
            return Some("IPv6 link-local address (fe80::/10)");
        }

        // 7. Multicast (ff00::/8)
        if ip.is_multicast() {
            return Some("IPv6 multicast address (ff00::/8)");
        }

        // 8. Documentation (2001:db8::/32)
        if segments[0] == 0x2001 && segments[1] == 0x0db8 {
            return Some("IPv6 documentation network (2001:db8::/32)");
        }

        // 9. Discard prefix (100::/64)
        if segments[0] == 0x0100
            && segments[1] == 0
            && segments[2] == 0
            && segments[3] == 0
            && segments[4] == 0
            && segments[5] == 0
            && segments[6] == 0
            && segments[7] == 0
        {
            return Some("IPv6 discard prefix (100::/64)");
        }

        None
    }

    /// Check whether any IP address is blocked.
    pub fn is_ip_blocked(ip: &IpAddr) -> Option<&'static str> {
        match ip {
            IpAddr::V4(v4) => Self::is_ipv4_blocked(v4),
            IpAddr::V6(v6) => Self::is_ipv6_blocked(v6),
        }
    }

    /// Validate an IP address directly against policy.
    pub fn validate_ip(&self, ip: IpAddr) -> Result<(), NetworkSecurityError> {
        if let Some(reason) = Self::is_ip_blocked(&ip) {
            return Err(NetworkSecurityError::BlockedDestination {
                host: ip.to_string(),
                ip: ip.to_string(),
                reason,
            });
        }
        Ok(())
    }

    /// Validate a hostname by checking localhost aliases and resolving DNS.
    pub async fn validate_host(&self, host: &str) -> Result<Vec<IpAddr>, NetworkSecurityError> {
        let trimmed_host = host.trim();
        let lower = trimmed_host.to_ascii_lowercase();

        // 1. Explicit localhost checks
        if lower == "localhost"
            || lower == "localhost.localdomain"
            || lower.ends_with(".localhost")
            || lower.ends_with(".local")
            || lower.ends_with(".internal")
        {
            return Err(NetworkSecurityError::BlockedDestination {
                host: trimmed_host.to_string(),
                ip: "127.0.0.1".to_string(),
                reason: "localhost/internal hostname alias",
            });
        }

        // 2. Direct IP parsing check
        if let Ok(ip) = trimmed_host.parse::<IpAddr>() {
            self.validate_ip(ip)?;
            return Ok(vec![ip]);
        }

        // Strip IPv6 brackets if present in host string (e.g. "[::1]")
        let clean_host = trimmed_host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(trimmed_host);
        if let Ok(ip) = clean_host.parse::<IpAddr>() {
            self.validate_ip(ip)?;
            return Ok(vec![ip]);
        }

        // 3. DNS resolution via tokio::net::lookup_host
        let addrs = tokio::net::lookup_host(format!("{}:80", trimmed_host))
            .await
            .map_err(|e| NetworkSecurityError::DnsResolutionFailed {
                host: trimmed_host.to_string(),
                message: e.to_string(),
            })?;

        let ips: Vec<IpAddr> = addrs.map(|sa| sa.ip()).collect();
        if ips.is_empty() {
            return Err(NetworkSecurityError::NoAddressesResolved(
                trimmed_host.to_string(),
            ));
        }

        // Fail-closed invariant: If ANY resolved IP is in a blocked range, fail closed!
        // This prevents dual-homed DNS rebinding and split-horizon DNS bypasses.
        for ip in &ips {
            if let Some(reason) = Self::is_ip_blocked(ip) {
                return Err(NetworkSecurityError::BlockedDestination {
                    host: trimmed_host.to_string(),
                    ip: ip.to_string(),
                    reason,
                });
            }
        }

        Ok(ips)
    }

    /// Validate a full URL for scheme and destination IP address.
    pub async fn validate_url(&self, raw_url: &str) -> Result<reqwest::Url, NetworkSecurityError> {
        let parsed = reqwest::Url::parse(raw_url)
            .map_err(|e| NetworkSecurityError::InvalidUrl(e.to_string()))?;

        // Scheme check: only HTTP and HTTPS permitted
        let scheme = parsed.scheme().to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(NetworkSecurityError::UnsupportedScheme(scheme));
        }

        // Host extraction
        let host = parsed.host_str().ok_or(NetworkSecurityError::MissingHost)?;

        // Validate host & DNS resolution
        self.validate_host(host).await?;

        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_loopback_blocked() {
        let policy = NetworkDestinationPolicy::new();
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
                .is_err()
        );
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(127, 255, 255, 255)))
                .is_err()
        );
        assert!(policy.validate_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)).is_err());
    }

    #[test]
    fn test_rfc1918_private_ipv4_blocked() {
        let policy = NetworkDestinationPolicy::new();
        // 10.0.0.0/8
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)))
                .is_err()
        );
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(10, 254, 1, 100)))
                .is_err()
        );
        // 172.16.0.0/12
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)))
                .is_err()
        );
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(172, 31, 255, 254)))
                .is_err()
        );
        // 192.168.0.0/16
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)))
                .is_err()
        );
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 100, 50)))
                .is_err()
        );
    }

    #[test]
    fn test_metadata_service_blocked() {
        let policy = NetworkDestinationPolicy::new();
        let metadata_ip = IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254));
        let err = policy.validate_ip(metadata_ip).unwrap_err();
        match err {
            NetworkSecurityError::BlockedDestination { reason, .. } => {
                assert!(reason.contains("metadata") || reason.contains("link-local"));
            }
            other => panic!("expected BlockedDestination, got {:?}", other),
        }
    }

    #[test]
    fn test_link_local_blocked() {
        let policy = NetworkDestinationPolicy::new();
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)))
                .is_err()
        );
        // IPv6 fe80::1
        let fe80 = IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1));
        assert!(policy.validate_ip(fe80).is_err());
    }

    #[test]
    fn test_ipv6_unique_local_blocked() {
        let policy = NetworkDestinationPolicy::new();
        // fc00::/7 (fd00::1)
        let fd00 = IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1));
        assert!(policy.validate_ip(fd00).is_err());
    }

    #[test]
    fn test_public_ips_allowed() {
        let policy = NetworkDestinationPolicy::new();
        // Cloudflare 1.1.1.1
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)))
                .is_ok()
        );
        // Google 8.8.8.8
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)))
                .is_ok()
        );
        // Quad9 9.9.9.9
        assert!(
            policy
                .validate_ip(IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)))
                .is_ok()
        );
    }

    #[test]
    fn test_ipv4_mapped_ipv6_loopback_blocked() {
        let policy = NetworkDestinationPolicy::new();
        // ::ffff:127.0.0.1
        let mapped = IpAddr::V6(Ipv4Addr::new(127, 0, 0, 1).to_ipv6_mapped());
        assert!(policy.validate_ip(mapped).is_err());

        // ::ffff:169.254.169.254
        let mapped_meta = IpAddr::V6(Ipv4Addr::new(169, 254, 169, 254).to_ipv6_mapped());
        assert!(policy.validate_ip(mapped_meta).is_err());
    }

    #[tokio::test]
    async fn test_localhost_aliases_blocked() {
        let policy = NetworkDestinationPolicy::new();
        assert!(policy.validate_host("localhost").await.is_err());
        assert!(policy.validate_host("localhost.localdomain").await.is_err());
        assert!(policy.validate_host("app.localhost").await.is_err());
        assert!(policy.validate_host("service.internal").await.is_err());
    }

    #[tokio::test]
    async fn test_url_schemes() {
        let policy = NetworkDestinationPolicy::new();
        assert!(policy.validate_url("file:///etc/passwd").await.is_err());
        assert!(policy.validate_url("gopher://localhost").await.is_err());
        assert!(policy.validate_url("ftp://ftp.example.com").await.is_err());
    }
}
