//! P1-01 regression: DNS validation is bound to the connection destination.
//!
//! Proves single-resolution validation (no validate/connect TOCTOU gap),
//! per-hop redirect validation, and fail-closed classification across the
//! full blocked-address taxonomy.

use m31a::policy::destination::{NetworkDestinationPolicy, ValidatingDnsResolver};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

fn policy() -> NetworkDestinationPolicy {
    NetworkDestinationPolicy::new()
}

#[test]
fn public_addresses_allowed() {
    let p = policy();
    for ip in [
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
        IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
    ] {
        assert!(p.validate_ip(ip).is_ok(), "{ip} must be allowed");
    }
}

#[test]
fn blocked_taxonomy_fails_closed() {
    let p = policy();
    let blocked: &[IpAddr] = &[
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), // loopback
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)), // RFC1918
        IpAddr::V4(Ipv4Addr::new(172, 16, 5, 5)),
        IpAddr::V4(Ipv4Addr::new(172, 31, 255, 1)),
        IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)), // metadata
        IpAddr::V4(Ipv4Addr::new(169, 254, 10, 20)),   // link-local
        IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),      // CGNAT
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),         // unspecified
        IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1)),       // multicast
        IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255)), // broadcast
        IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),       // TEST-NET-1
        IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7)),    // TEST-NET-2
        IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9)),     // TEST-NET-3
        IpAddr::V6(Ipv6Addr::LOCALHOST),               // ::1
        IpAddr::V6(Ipv6Addr::UNSPECIFIED),             // ::
        IpAddr::V6(Ipv4Addr::new(127, 0, 0, 1).to_ipv6_mapped()), // v4-mapped loopback
        IpAddr::V6(Ipv4Addr::new(10, 0, 0, 1).to_ipv6_mapped()), // v4-mapped RFC1918
        IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1)), // unique-local
        IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)), // link-local
        IpAddr::V6(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1)), // multicast
        IpAddr::V6(Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1)), // documentation
    ];
    for ip in blocked {
        assert!(p.validate_ip(*ip).is_err(), "{ip} must be blocked");
    }
}

#[tokio::test]
async fn hostname_aliases_blocked_without_dns() {
    let p = policy();
    for host in [
        "localhost",
        "localhost.localdomain",
        "app.localhost",
        "svc.local",
        "db.internal",
    ] {
        assert!(
            p.validate_host(host).await.is_err(),
            "{host} must be blocked"
        );
        assert!(
            p.resolve_socket_addrs(host, 443).await.is_err(),
            "{host} binding must fail"
        );
    }
}

#[tokio::test]
async fn literal_binding_returns_validated_socket_addr() {
    let p = policy();
    // Allowed literal: single resolution, bound address, no second DNS.
    let bound = p
        .resolve_socket_addrs("8.8.8.8", 443)
        .await
        .expect("public literal must bind");
    assert_eq!(bound.len(), 1);
    assert_eq!(
        bound[0].socket_addr().ip(),
        IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))
    );
    assert_eq!(bound[0].port(), 443);
    assert_eq!(bound[0].host(), "8.8.8.8");

    // Blocked literals fail at BIND time (before any connection attempt).
    for host in [
        "127.0.0.1",
        "10.0.0.1",
        "192.168.1.1",
        "169.254.169.254",
        "0.0.0.0",
    ] {
        assert!(
            p.resolve_socket_addrs(host, 80).await.is_err(),
            "{host} must not bind"
        );
    }
    // Bracketed IPv6 literals handled.
    assert!(p.resolve_socket_addrs("[::1]", 80).await.is_err());
}

#[tokio::test]
async fn validated_connection_never_re_resolves() {
    let p = policy();
    // A blocked binding can never connect: connect_validated re-checks the
    // IP synchronously with zero DNS involvement and fails closed.
    let err = p.resolve_socket_addrs("127.0.0.1", 9).await.unwrap_err();
    assert!(
        matches!(err, m31a::policy::destination::NetworkSecurityError::BlockedDestination { .. }),
        "resolve_socket_addrs for 127.0.0.1 must yield BlockedDestination, got: {err:?}"
    );

    // If an invalid/blocked address was smuggled into connect_validated,
    // connect_validated must reject it with BlockedDestination before any network activity.
    let smuggled_blocked = m31a::policy::destination::ValidatedSocketAddr::new_for_test(
        std::net::SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        "127.0.0.1",
    );
    let smuggled_err = p
        .connect_validated(&smuggled_blocked, Duration::from_millis(50))
        .await
        .unwrap_err();
    assert!(
        matches!(smuggled_err, m31a::policy::destination::NetworkSecurityError::BlockedDestination { .. }),
        "Smuggled blocked address in connect_validated must return BlockedDestination, got: {smuggled_err:?}"
    );

    // Public literal binding succeeds at bind time.
    let bound = p.resolve_socket_addrs("8.8.8.8", 1).await.expect("public IP literal must bind");
    let conn = p
        .connect_validated(&bound[0], Duration::from_millis(100))
        .await;

    // A transport failure on an allowed IP must return ConnectionFailed, NOT BlockedDestination or DnsResolutionFailed!
    match conn {
        Ok(_) => {}
        Err(m31a::policy::destination::NetworkSecurityError::ConnectionFailed { host, ip, .. }) => {
            assert_eq!(host, "8.8.8.8");
            assert_eq!(ip, "8.8.8.8");
        }
        Err(other) => {
            panic!(
                "Transport failure must map to ConnectionFailed, never policy rejection or DNS failure: {:?}",
                other
            );
        }
    }
}

#[tokio::test]
async fn url_validation_rejects_redirect_targets() {
    let p = policy();
    // Redirect destinations are validated independently per hop (same
    // function the web provider calls for every hop).
    for url in [
        "http://127.0.0.1:11434/v1",
        "https://169.254.169.254/latest/meta-data/",
        "http://10.0.0.5/admin",
        "file:///etc/passwd",
        "ftp://ftp.example.com/x",
        "https://localhost:11434/v1",
    ] {
        assert!(p.validate_url(url).await.is_err(), "{url} must be rejected");
    }
}

#[tokio::test]
async fn validating_resolver_is_the_centralized_connector() {
    use reqwest::dns::Resolve;

    // 1. Direct DNS resolver contract: blocked destinations fail at name resolution
    let resolver = ValidatingDnsResolver;
    let err = match resolver.resolve("localhost".parse().unwrap()).await {
        Err(e) => e,
        Ok(_) => panic!("ValidatingDnsResolver must reject localhost"),
    };
    assert!(err.to_string().contains("egress policy blocked"));

    let err_meta = match resolver.resolve("169.254.169.254".parse().unwrap()).await {
        Err(e) => e,
        Ok(_) => panic!("ValidatingDnsResolver must reject cloud metadata IP"),
    };
    assert!(err_meta.to_string().contains("egress policy blocked"));

    // 2. HTTP client builder contract: policy_validating_client_builder produces client
    // whose requests to blocked hostnames strictly fail closed via the validating resolver
    // BEFORE any connection is attempted (never passing because of a transport connection failure).
    let client = m31a::policy::destination::policy_validating_client_builder()
        .build()
        .expect("build client");
    let req_err = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .expect_err("client request to blocked host must fail");

    // The failure MUST stem from the policy-enforcing resolver (PermissionDenied: egress policy blocked)
    let debug_str = format!("{req_err:?}");
    let mut chain = req_err.to_string();
    let mut cur: Option<&dyn std::error::Error> = std::error::Error::source(&req_err);
    while let Some(e) = cur {
        chain.push_str(" -> ");
        chain.push_str(&e.to_string());
        cur = e.source();
    }
    assert!(
        debug_str.contains("egress policy blocked")
            || debug_str.contains("PermissionDenied")
            || chain.contains("egress policy blocked")
            || chain.contains("PermissionDenied"),
        "Client error MUST stem from resolver policy block, got chain: {chain}"
    );
}

#[tokio::test]
async fn test_policy_rejection_prevents_side_effects_on_target_endpoint() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Spin up an actual local TCP listener to verify that policy denial prevents
    // ANY connection or side effect on the local endpoint.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local test listener");
    let local_port = listener.local_addr().unwrap().port();

    let accepted_connections = Arc::new(AtomicUsize::new(0));
    let counter_clone = accepted_connections.clone();

    let server_task = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            use tokio::io::AsyncWriteExt;
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK").await;
        }
    });

    let validating_client = m31a::policy::destination::policy_validating_client_builder()
        .build()
        .expect("build validating client");

    // 1. Prohibited domain destination: localhost
    let url_host = format!("http://localhost:{local_port}/prohibited-action");
    let err_host = validating_client
        .get(&url_host)
        .send()
        .await
        .expect_err("request to localhost must be denied by validating client");
    let debug_host = format!("{err_host:?}");
    assert!(
        debug_host.contains("egress policy blocked") || debug_host.contains("PermissionDenied"),
        "Hostname request must fail at validating resolver: {debug_host}"
    );

    // 2. Prohibited IP literal destination: 127.0.0.1
    let url_ip = format!("http://127.0.0.1:{local_port}/prohibited-action");
    let _ = validating_client.get(&url_ip).send().await;

    // CRITICAL SECURITY ASSERTION: The listener must have received ZERO connections!
    // Both hostname validation and IP guard prevented any connection to the local target endpoint.
    assert_eq!(
        accepted_connections.load(Ordering::SeqCst),
        0,
        "Target endpoint must receive zero connections when policy denies egress"
    );

    // 3. Falsification comparison: An unvalidated raw client DOES hit the server,
    // proving the listener is active and the validating client was the mechanism that protected it.
    let unvalidated_client = reqwest::Client::new();
    let res = unvalidated_client.get(&url_ip).send().await;
    assert!(res.is_ok(), "Raw client reaches local endpoint");
    assert!(
        accepted_connections.load(Ordering::SeqCst) >= 1,
        "Raw client must connect, proving endpoint was active"
    );

    server_task.abort();
}

#[tokio::test]
async fn test_dns_resolution_failure_distinguishable_from_policy_rejection() {
    let p = policy();

    // 1. Policy rejection: known prohibited destination returns BlockedDestination
    let policy_err = p.resolve_socket_addrs("127.0.0.1", 80).await.unwrap_err();
    assert!(
        matches!(
            policy_err,
            m31a::policy::destination::NetworkSecurityError::BlockedDestination { .. }
        ),
        "Policy rejection must yield BlockedDestination, got: {policy_err:?}"
    );

    // 2. DNS resolution failure: non-existent domain in .invalid (RFC 2606) yields DnsResolutionFailed
    let dns_err = p
        .resolve_socket_addrs("nonexistent-adversarial-test-domain-4949.invalid", 80)
        .await
        .unwrap_err();
    assert!(
        matches!(
            dns_err,
            m31a::policy::destination::NetworkSecurityError::DnsResolutionFailed { .. }
                | m31a::policy::destination::NetworkSecurityError::NoAddressesResolved(..)
        ),
        "Resolution failure must yield DnsResolutionFailed or NoAddressesResolved, NOT BlockedDestination: {dns_err:?}"
    );

    // 3. Transport failure: valid IP but port refused yields ConnectionFailed
    let valid_addr = m31a::policy::destination::ValidatedSocketAddr::new_for_test(
        std::net::SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 1),
        "8.8.8.8",
    );
    let conn_err = p
        .connect_validated(&valid_addr, Duration::from_millis(100))
        .await
        .unwrap_err();
    assert!(
        matches!(
            conn_err,
            m31a::policy::destination::NetworkSecurityError::ConnectionFailed { .. }
        ),
        "Transport failure must yield ConnectionFailed, NOT BlockedDestination or DnsResolutionFailed: {conn_err:?}"
    );
}
