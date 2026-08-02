package search

import (
	"net"
	"testing"
)

func TestIsPrivateIP(t *testing.T) {
	tests := []struct {
		name     string
		ipStr    string
		expected bool
	}{
		{"loopback 127.0.0.1", "127.0.0.1", true},
		{"loopback ::1", "::1", true},
		{"private 10.0.0.1", "10.0.0.1", true},
		{"private 10.255.255.255", "10.255.255.255", true},
		{"private 172.16.0.1", "172.16.0.1", true},
		{"private 172.31.255.255", "172.31.255.255", true},
		{"private 192.168.0.1", "192.168.0.1", true},
		{"private 192.168.255.255", "192.168.255.255", true},
		{"link-local 169.254.0.1", "169.254.0.1", true},
		{"link-local 169.254.255.255", "169.254.255.255", true},
		{"public 8.8.8.8", "8.8.8.8", false},
		{"public 1.1.1.1", "1.1.1.1", false},
		{"public 203.0.113.1", "203.0.113.1", false},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			ip := net.ParseIP(tc.ipStr)
			if ip == nil {
				t.Fatalf("failed to parse IP: %s", tc.ipStr)
			}
			result := IsPrivateIP(ip)
			if result != tc.expected {
				t.Errorf("IsPrivateIP(%s) = %v, want %v", tc.ipStr, result, tc.expected)
			}
		})
	}
}

func TestIsReservedIP(t *testing.T) {
	tests := []struct {
		name     string
		ipStr    string
		expected bool
	}{
		{"0.0.0.0", "0.0.0.0", true},
		{"0.255.255.255", "0.255.255.255", true},
		{"100.64.0.1", "100.64.0.1", true},
		{"100.127.255.255", "100.127.255.255", true},
		{"192.0.0.1", "192.0.0.1", true},
		{"192.0.0.255", "192.0.0.255", true},
		{"192.0.2.1", "192.0.2.1", true},
		{"192.0.2.255", "192.0.2.255", true},
		{"198.18.0.1", "198.18.0.1", true},
		{"198.19.255.255", "198.19.255.255", true},
		{"198.51.100.1", "198.51.100.1", true},
		{"203.0.113.1", "203.0.113.1", true},
		{"240.0.0.1", "240.0.0.1", true},
		{"255.255.255.255", "255.255.255.255", true},
		{"8.8.8.8", "8.8.8.8", false},
		{"1.1.1.1", "1.1.1.1", false},
		{"10.0.0.1", "10.0.0.1", false}, // Private, not reserved
		{"127.0.0.1", "127.0.0.1", false}, // Loopback, not reserved
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			ip := net.ParseIP(tc.ipStr)
			if ip == nil {
				t.Fatalf("failed to parse IP: %s", tc.ipStr)
			}
			result := IsReservedIP(ip)
			if result != tc.expected {
				t.Errorf("IsReservedIP(%s) = %v, want %v", tc.ipStr, result, tc.expected)
			}
		})
	}
}