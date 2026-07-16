package search

import "net"

// IsPrivateIP checks if an IP is private (RFC 1918), loopback, or link-local.
func IsPrivateIP(ip net.IP) bool {
	return ip.IsLoopback() || ip.IsLinkLocalUnicast() || ip.IsLinkLocalMulticast() || ip.IsPrivate()
}

// IsReservedIP checks if an IP is reserved (RFC 5735).
func IsReservedIP(ip net.IP) bool {
	if ip4 := ip.To4(); ip4 != nil {
		// 0.0.0.0/8 - "This network"
		if ip4[0] == 0 {
			return true
		}
		// 100.64.0.0/10 - CGNAT
		if ip4[0] == 100 && ip4[1] >= 64 && ip4[1] <= 127 {
			return true
		}
		// 169.254.0.0/16 - Link-local (already covered by IsLinkLocalUnicast)
		// 192.0.0.0/24 - IETF protocol assignments
		if ip4[0] == 192 && ip4[1] == 0 && ip4[2] == 0 {
			return true
		}
		// 192.0.2.0/24 - TEST-NET-1
		if ip4[0] == 192 && ip4[1] == 0 && ip4[2] == 2 {
			return true
		}
		// 198.18.0.0/15 - Benchmark testing
		if ip4[0] == 198 && (ip4[1] == 18 || ip4[1] == 19) {
			return true
		}
		// 198.51.100.0/24 - TEST-NET-2
		if ip4[0] == 198 && ip4[1] == 51 && ip4[2] == 100 {
			return true
		}
		// 203.0.113.0/24 - TEST-NET-3
		if ip4[0] == 203 && ip4[1] == 0 && ip4[2] == 113 {
			return true
		}
		// 240.0.0.0/4 - Future use
		if ip4[0] >= 240 {
			return true
		}
		// 255.255.255.255/32 - Limited broadcast
		if ip4[0] == 255 && ip4[1] == 255 && ip4[2] == 255 && ip4[3] == 255 {
			return true
		}
	}
	return false
}
