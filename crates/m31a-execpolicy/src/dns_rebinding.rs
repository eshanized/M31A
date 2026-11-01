//! Anti-DNS-Rebinding Resolver and Dynamic IP Validator.
//!
//! Protects the local network proxy against DNS rebinding attacks and SSRF exploits
//! by verifying that domain names and resolved IP addresses do not map to local loopback,
//! cloud metadata services (e.g. 169.254.169.254), or non-public private subnets.

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

/// Risk classifications for unsafe DNS resolutions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsRebindingRisk {
    Loopback(IpAddr),
    CloudMetadata(IpAddr),
    PrivateSubnet(IpAddr),
    LinkLocal(IpAddr),
    MulticastOrBroadcast(IpAddr),
    ResolutionFailed(String),
}

impl std::fmt::Display for DnsRebindingRisk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loopback(ip) => write!(f, "Resolved to loopback address: {}", ip),
            Self::CloudMetadata(ip) => write!(f, "Resolved to cloud metadata address: {}", ip),
            Self::PrivateSubnet(ip) => write!(f, "Resolved to private RFC subnet: {}", ip),
            Self::LinkLocal(ip) => write!(f, "Resolved to link-local address: {}", ip),
            Self::MulticastOrBroadcast(ip) => write!(f, "Resolved to multicast/broadcast: {}", ip),
            Self::ResolutionFailed(err) => write!(f, "DNS resolution failed: {}", err),
        }
    }
}

impl std::error::Error for DnsRebindingRisk {}

/// Validates whether an IP address is a safe, routable public destination.
pub fn validate_ip_safety(ip: IpAddr) -> Result<(), DnsRebindingRisk> {
    match ip {
        IpAddr::V4(ipv4) => validate_ipv4_safety(ipv4),
        IpAddr::V6(ipv6) => validate_ipv6_safety(ipv6),
    }
}

fn validate_ipv4_safety(ip: Ipv4Addr) -> Result<(), DnsRebindingRisk> {
    let octets = ip.octets();

    // Loopback 127.0.0.0/8
    if ip.is_loopback() || octets[0] == 127 {
        return Err(DnsRebindingRisk::Loopback(IpAddr::V4(ip)));
    }

    // Cloud metadata: 169.254.169.254
    if octets[0] == 169 && octets[1] == 254 && octets[2] == 169 && octets[3] == 254 {
        return Err(DnsRebindingRisk::CloudMetadata(IpAddr::V4(ip)));
    }

    // Link-local: 169.254.0.0/16
    if ip.is_link_local() || (octets[0] == 169 && octets[1] == 254) {
        return Err(DnsRebindingRisk::LinkLocal(IpAddr::V4(ip)));
    }

    // Private subnets:
    // 10.0.0.0/8
    if octets[0] == 10 {
        return Err(DnsRebindingRisk::PrivateSubnet(IpAddr::V4(ip)));
    }
    // 172.16.0.0/12
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return Err(DnsRebindingRisk::PrivateSubnet(IpAddr::V4(ip)));
    }
    // 192.168.0.0/16
    if octets[0] == 192 && octets[1] == 168 {
        return Err(DnsRebindingRisk::PrivateSubnet(IpAddr::V4(ip)));
    }

    // Broadcast: 255.255.255.255
    if ip.is_broadcast() {
        return Err(DnsRebindingRisk::MulticastOrBroadcast(IpAddr::V4(ip)));
    }

    // Multicast: 224.0.0.0/4
    if ip.is_multicast() || octets[0] >= 224 {
        return Err(DnsRebindingRisk::MulticastOrBroadcast(IpAddr::V4(ip)));
    }

    // Unspecified: 0.0.0.0
    if ip.is_unspecified() {
        return Err(DnsRebindingRisk::PrivateSubnet(IpAddr::V4(ip)));
    }

    Ok(())
}

fn validate_ipv6_safety(ip: Ipv6Addr) -> Result<(), DnsRebindingRisk> {
    if ip.is_loopback() || ip == Ipv6Addr::LOCALHOST {
        return Err(DnsRebindingRisk::Loopback(IpAddr::V6(ip)));
    }

    if ip.is_multicast() {
        return Err(DnsRebindingRisk::MulticastOrBroadcast(IpAddr::V6(ip)));
    }

    if ip.is_unspecified() {
        return Err(DnsRebindingRisk::PrivateSubnet(IpAddr::V6(ip)));
    }

    let segments = ip.segments();

    // Link-local: fe80::/10
    if (segments[0] & 0xffc0) == 0xfe80 {
        return Err(DnsRebindingRisk::LinkLocal(IpAddr::V6(ip)));
    }

    // Unique Local (ULA): fc00::/7 (fc00 - fdff)
    if (segments[0] & 0xfe00) == 0xfc00 {
        return Err(DnsRebindingRisk::PrivateSubnet(IpAddr::V6(ip)));
    }

    // IPv4-mapped IPv6: ::ffff:127.0.0.1
    if let Some(v4) = ip.to_ipv4_mapped() {
        return validate_ipv4_safety(v4);
    }

    Ok(())
}

/// Anti-rebinding resolver ensuring all addresses resolved for a host are safe.
#[derive(Debug, Clone, Default)]
pub struct AntiRebindingDnsResolver {
    allow_loopback_for_testing: bool,
}

impl AntiRebindingDnsResolver {
    pub fn new() -> Self {
        Self {
            allow_loopback_for_testing: false,
        }
    }

    pub fn with_allow_loopback(allow: bool) -> Self {
        Self {
            allow_loopback_for_testing: allow,
        }
    }

    /// Resolve a hostname and validate that none of the resolved IPs are dangerous.
    pub fn resolve_and_validate(&self, host: &str) -> Result<Vec<IpAddr>, DnsRebindingRisk> {
        // If host is already an IP literal
        if let Ok(ip) = host.parse::<IpAddr>() {
            if self.allow_loopback_for_testing && ip.is_loopback() {
                return Ok(vec![ip]);
            }
            validate_ip_safety(ip)?;
            return Ok(vec![ip]);
        }

        // Perform DNS resolution on host:80
        let target = format!("{}:80", host);
        let addrs: Vec<IpAddr> = match target.to_socket_addrs() {
            Ok(iter) => iter.map(|sa| sa.ip()).collect(),
            Err(e) => return Err(DnsRebindingRisk::ResolutionFailed(e.to_string())),
        };

        if addrs.is_empty() {
            return Err(DnsRebindingRisk::ResolutionFailed(
                "No IP addresses resolved".to_string(),
            ));
        }

        for &ip in &addrs {
            if self.allow_loopback_for_testing && ip.is_loopback() {
                continue;
            }
            validate_ip_safety(ip)?;
        }

        Ok(addrs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_public_ips() {
        assert!(validate_ip_safety("8.8.8.8".parse().unwrap()).is_ok());
        assert!(validate_ip_safety("1.1.1.1".parse().unwrap()).is_ok());
        assert!(validate_ip_safety("2606:4700:4700::1111".parse().unwrap()).is_ok());
    }

    #[test]
    fn test_blocks_loopback() {
        let v4: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(validate_ip_safety(v4), Err(DnsRebindingRisk::Loopback(v4)));

        let v6: IpAddr = "::1".parse().unwrap();
        assert_eq!(validate_ip_safety(v6), Err(DnsRebindingRisk::Loopback(v6)));
    }

    #[test]
    fn test_blocks_cloud_metadata() {
        let meta_v4: IpAddr = "169.254.169.254".parse().unwrap();
        assert_eq!(
            validate_ip_safety(meta_v4),
            Err(DnsRebindingRisk::CloudMetadata(meta_v4))
        );
    }

    #[test]
    fn test_blocks_private_subnets() {
        let sub10: IpAddr = "10.0.1.5".parse().unwrap();
        assert_eq!(
            validate_ip_safety(sub10),
            Err(DnsRebindingRisk::PrivateSubnet(sub10))
        );

        let sub172: IpAddr = "172.20.0.1".parse().unwrap();
        assert_eq!(
            validate_ip_safety(sub172),
            Err(DnsRebindingRisk::PrivateSubnet(sub172))
        );

        let sub192: IpAddr = "192.168.1.1".parse().unwrap();
        assert_eq!(
            validate_ip_safety(sub192),
            Err(DnsRebindingRisk::PrivateSubnet(sub192))
        );

        let ula: IpAddr = "fd00::1".parse().unwrap();
        assert_eq!(
            validate_ip_safety(ula),
            Err(DnsRebindingRisk::PrivateSubnet(ula))
        );
    }

    #[test]
    fn test_resolver_ip_literal() {
        let resolver = AntiRebindingDnsResolver::new();
        assert!(resolver.resolve_and_validate("1.1.1.1").is_ok());
        assert!(resolver.resolve_and_validate("127.0.0.1").is_err());
    }
}
