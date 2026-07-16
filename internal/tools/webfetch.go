package tools

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"net"
	"net/http"
	"net/url"
	"strings"
	"sync/atomic"
	"time"

	"github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

var _ types.Tool = (*WebFetch)(nil)

// Version is the application version used in User-Agent headers.
// Set via SetVersion() from cmd/m31a/main.go.
var Version atomic.Value

// SetVersion sets the application version for User-Agent headers.
func SetVersion(v string) {
	if v != "" {
		Version.Store(v)
	}
}

// getVersion returns the current version string, defaulting to "dev" if unset.
func getVersion() string {
	if v, ok := Version.Load().(string); ok {
		return v
	}
	return "dev"
}

type WebFetch struct {
	sessionsDir     string
	allowPrivateIPs bool
	client          *http.Client
	dnsCache        *DNSCache
	maxRetries      int
	baseRetryDelay  time.Duration
}

// NewWebFetch creates a new WebFetch tool instance.
func NewWebFetch(sessionsDir string, allowPrivateIPs bool, maxRetries int, retryDelayMs int) *WebFetch {
	retries := 3
	if maxRetries > 0 {
		retries = maxRetries
	}
	delay := 500 * time.Millisecond
	if retryDelayMs > 0 {
		delay = time.Duration(retryDelayMs) * time.Millisecond
	}
	wf := &WebFetch{
		sessionsDir:     sessionsDir,
		allowPrivateIPs: allowPrivateIPs,
		dnsCache:        NewDNSCache(DNSCacheTTL, 64),
		maxRetries:      retries,
		baseRetryDelay:  delay,
	}
	wf.client = &http.Client{
		Transport: &http.Transport{
			MaxIdleConns:          100,
			MaxIdleConnsPerHost:   10,
			IdleConnTimeout:       90 * time.Second,
			TLSHandshakeTimeout:   10 * time.Second,
			ResponseHeaderTimeout: 30 * time.Second,
			ExpectContinueTimeout: 1 * time.Second,
			DisableKeepAlives:     false,
			DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
				host, port, err := net.SplitHostPort(addr)
				if err != nil {
					return nil, fmt.Errorf("invalid address: %w", err)
				}

				// Use pinned DNS cache to prevent TOCTOU rebinding attacks
				addrs, err := wf.resolveAndCache(ctx, host)
				if err != nil {
					return nil, err
				}

				// Check ALL resolved IPs against private range (not just the first)
				if !wf.allowPrivateIPs {
					for _, addr := range addrs {
						if isPrivateIP(addr.IP) || isReservedIP(addr.IP) {
							return nil, fmt.Errorf("access to private/reserved IP %s is blocked: %w", addr.IP, errors.ErrPrivateIPBlocked)
						}
					}
				}

				if len(addrs) == 0 {
					return nil, fmt.Errorf("no IP addresses resolved for %s", host)
				}

				// Pin the first IP for connection
				pinnedAddr := net.JoinHostPort(addrs[0].IP.String(), port)

				// Connect with the pinned IP
				dialer := &net.Dialer{Timeout: time.Duration(DefaultTimeoutSecs) * time.Second}
				conn, err := dialer.DialContext(ctx, network, pinnedAddr)
				if err != nil {
					return nil, err
				}

				// Re-check after connect (paranoid check)
				if tcpConn, ok := conn.(*net.TCPConn); ok {
					remoteAddr, addrOk := tcpConn.RemoteAddr().(*net.TCPAddr)
					if addrOk && !wf.allowPrivateIPs && (isPrivateIP(remoteAddr.IP) || isReservedIP(remoteAddr.IP)) {
						_ = conn.Close()
						return nil, fmt.Errorf("connected to private/reserved IP %s is blocked: %w", remoteAddr.IP, errors.ErrPrivateIPBlocked)
					}
				}

				return conn, nil
			},
		},
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			if len(via) >= MaxRedirects {
				return fmt.Errorf("stopped after %d redirects", MaxRedirects)
			}
			// SSRF protection: resolve and cache DNS for redirect target,
			// then check for private IPs. Using resolveAndCache ensures
			// the same pinned IP is used for the redirect connection.
			host := req.URL.Hostname()
			if host == "" {
				return nil
			}
			addrs, err := wf.resolveAndCache(req.Context(), host)
			if err != nil {
				slog.Warn("WebFetch redirect DNS resolution failed",
					"url", req.URL.String(), "error", err)
				return fmt.Errorf("resolve redirect host: %w", err)
			}
			if !wf.allowPrivateIPs {
				for _, addr := range addrs {
					if isPrivateIP(addr.IP) || isReservedIP(addr.IP) {
						slog.Warn("WebFetch redirect blocked by SSRF protection",
							"url", req.URL.String(), "ip", addr.IP)
						return fmt.Errorf("redirect to private/reserved IP %s is blocked: %w", addr.IP, errors.ErrPrivateIPBlocked)
					}
				}
			}
			return nil
		},
	}
	return wf
}

// isPrivateIP returns true if the IP is loopback, link-local, RFC1918,
// IPv6 ULA, IPv6 link-local, or cloud metadata. These should not be
// reachable by an external agent.
// Uses net.IP.IsPrivate() (Go 1.17+) for comprehensive coverage
// including IPv4-mapped IPv6 addresses and IPv6 ULA ranges.
func isPrivateIP(ip net.IP) bool {
	if ip.IsLoopback() {
		return true
	}
	if ip.IsLinkLocalUnicast() || ip.IsLinkLocalMulticast() {
		return true
	}
	if ip.IsUnspecified() {
		return true
	}
	// Use Go's built-in IsPrivate for RFC1918, RFC4193 (ULA), and IPv4-mapped IPv6
	if ip.IsPrivate() {
		return true
	}
	// Check for link-local range 169.254.0.0/16 (cloud metadata and other services)
	if ip4 := ip.To4(); ip4 != nil {
		if ip4[0] == 169 && ip4[1] == 254 {
			return true
		}
	} else {
		// IPv6 cloud metadata: AWS uses fd00:ec2::254
		if ip6 := ip.To16(); ip6 != nil {
			if ip6[0] == 0xfd && ip6[1] == 0x00 && ip6[2] == 0x0e && ip6[3] == 0xc2 {
				return true
			}
		}
	}
	return false
}

// isReservedIP checks if an IP is in a reserved range that should never
// be the target of an outbound connection.
func isReservedIP(ip net.IP) bool {
	// Check for multicast addresses
	if ip.IsMulticast() {
		return true
	}
	// Check for broadcast address (255.255.255.255)
	if ip4 := ip.To4(); ip4 != nil {
		if ip4[0] == 255 && ip4[1] == 255 && ip4[2] == 255 && ip4[3] == 255 {
			return true
		}
	}
	return false
}

// resolveAndCheck resolves the host in a URL and rejects private IPs
// unless allowPrivateIPs is true.
func (t *WebFetch) resolveAndCheck(ctx context.Context, urlStr string) error {
	parsed, err := url.Parse(urlStr)
	if err != nil {
		return fmt.Errorf("invalid URL: %w", err)
	}

	host := parsed.Hostname()
	if host == "" {
		return fmt.Errorf("URL has no host")
	}

	// Check for literal IP first
	if ip := net.ParseIP(host); ip != nil {
		if !t.allowPrivateIPs && isPrivateIP(ip) {
			return fmt.Errorf("access to private IP %s is blocked: %w", ip, errors.ErrPrivateIPBlocked)
		}
		return nil
	}

	// Resolve hostname
	resolver := &net.Resolver{PreferGo: true}
	addrs, err := resolver.LookupIPAddr(ctx, host)
	if err != nil {
		return fmt.Errorf("DNS resolution failed for %s: %w", host, err)
	}

	if !t.allowPrivateIPs {
		for _, addr := range addrs {
			if isPrivateIP(addr.IP) {
				return fmt.Errorf("host %s resolves to private IP %s: %w", host, addr.IP, errors.ErrPrivateIPBlocked)
			}
		}
	}

	return nil
}

// resolveAndCache resolves DNS for a hostname using the shared DNS cache.
func (t *WebFetch) resolveAndCache(ctx context.Context, host string) ([]net.IPAddr, error) {
	return t.dnsCache.Resolve(ctx, host)
}

func (t *WebFetch) Name() string {
	return "WebFetch"
}

func (t *WebFetch) Description() string {
	return "Fetch content from a URL and return it as markdown, text, or raw HTML. Includes automatic retry with exponential backoff for transient failures."
}

func (t *WebFetch) RiskLevel() types.RiskLevel {
	return types.RiskMedium
}

// ParameterSchema returns the JSON Schema for WebFetch tool parameters.
func (t *WebFetch) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"url": {"type": "string", "description": "URL to fetch"},
			"format": {"type": "string", "enum": ["text", "markdown", "html"], "description": "Output format", "default": "markdown"},
			"timeout": {"type": "integer", "description": "Request timeout in seconds (default 30, max 120)"},
			"retries": {"type": "integer", "description": "Max retry attempts for transient failures (default 3, max 5)"}
		},
		"required": ["url"]
	}`
}

func (t *WebFetch) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	urlRaw, ok := input.Params["url"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: url")
	}
	urlStr, ok := urlRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter url must be a string")
	}

	// Validate URL
	parsed, err := url.Parse(urlStr)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("invalid URL: %w", err)
	}
	if parsed.Scheme != "http" && parsed.Scheme != "https" {
		return types.ToolResult{}, fmt.Errorf("only http and https schemes are supported")
	}

	format := "markdown"
	if f, ok := input.Params["format"].(string); ok {
		switch f {
		case "markdown", "text", "html":
			format = f
		default:
			return types.ToolResult{}, fmt.Errorf("invalid format: %s (must be markdown, text, or html)", f)
		}
	}

	timeout := DefaultTimeoutSecs
	if tRaw, ok := input.Params["timeout"].(float64); ok {
		timeout = int(tRaw)
	}
	if timeout <= 0 || timeout > MaxTimeoutSecs {
		return types.ToolResult{}, fmt.Errorf("timeout must be between 1 and %d seconds", MaxTimeoutSecs)
	}

	retries := t.maxRetries
	if rRaw, ok := input.Params["retries"].(float64); ok {
		retries = int(rRaw)
		if retries < 0 {
			retries = 0
		}
		if retries > 5 {
			retries = 5
		}
	}

	// SSRF protection is handled by the dialer's resolver in the HTTP client.
	// No separate resolveAndCheck call needed here — the dialer resolves DNS,
	// checks for private IPs, and pins the IP for the connection.

	var lastErr error
	for attempt := 0; attempt <= retries; attempt++ {
		if attempt > 0 {
			// Exponential backoff with jitter
			delay := t.baseRetryDelay * time.Duration(1<<(attempt-1))
			select {
			case <-time.After(delay):
			case <-ctx.Done():
				return types.ToolResult{}, ctx.Err()
			}
		}

		result, err := t.doFetch(ctx, urlStr, format, timeout)
		if err == nil {
			elapsed := time.Since(start).Milliseconds()
			result.DurationMs = elapsed
			return result, nil
		}
		lastErr = err

		// Don't retry on client errors (4xx) or permanent errors
		if !isRetryableError(err) {
			break
		}
	}

	return types.ToolResult{}, fmt.Errorf("request failed after %d retries: %w", retries, lastErr)
}

// doFetch performs a single HTTP fetch attempt.
func (t *WebFetch) doFetch(ctx context.Context, urlStr, format string, timeout int) (types.ToolResult, error) {
	// Create request
	req, err := http.NewRequestWithContext(ctx, "GET", urlStr, nil)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("failed to create request: %w", err)
	}

	// Browser user agent
	req.Header.Set("User-Agent", fmt.Sprintf("M31A/%s (AI Coding Agent; +https://github.com/eshanized/M31A)", getVersion()))
	req.Header.Set("Accept", "text/html,application/xhtml+xml,application/xml,text/plain;q=0.9,*/*;q=0.8")
	req.Header.Set("Accept-Language", "en-US,en;q=0.5")

	// Execute request — use shared client with per-request timeout via context
	reqCtx, reqCancel := context.WithTimeout(ctx, time.Duration(timeout)*time.Second)
	defer reqCancel()
	req = req.WithContext(reqCtx)
	resp, err := t.client.Do(req)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close() //nolint:errcheck

	// Check status code
	if resp.StatusCode < 200 || resp.StatusCode >= 400 {
		return types.ToolResult{}, fmt.Errorf("HTTP %d: %s", resp.StatusCode, resp.Status)
	}

	// Read body with size limit (5MB)
	if resp.ContentLength > types.MaxFileSize {
		return types.ToolResult{}, fmt.Errorf("response too large: %d bytes exceeds %d limit", resp.ContentLength, types.MaxFileSize)
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, types.MaxFileSize+1))
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("failed to read response body: %w", err)
	}
	if len(body) > types.MaxFileSize {
		return types.ToolResult{}, fmt.Errorf("response exceeds 5MB limit")
	}

	contentType := resp.Header.Get("Content-Type")
	isHTML := strings.Contains(contentType, "html")

	var result string
	switch {
	case isHTML && format == "markdown":
		result = htmlToMarkdown(string(body))
	case isHTML && format == "text":
		result = htmlToText(string(body))
	case isHTML && format == "html":
		result = string(body)
	default:
		result = string(body)
	}

	return types.ToolResult{
		Output: fmt.Sprintf("Fetched %s (%s)\nContent-Type: %s\nSize: %d bytes\n\n%s",
			urlStr, resp.Status, contentType, len(result), result),
	}, nil
}

// isRetryableError returns true if the error is transient and worth retrying.
func isRetryableError(err error) bool {
	errStr := err.Error()
	// Retry on network errors, timeouts, and 5xx status codes
	if strings.Contains(errStr, "request failed") && !strings.Contains(errStr, "HTTP 4") {
		return true
	}
	if strings.Contains(errStr, "connection refused") ||
		strings.Contains(errStr, "connection reset") ||
		strings.Contains(errStr, "broken pipe") ||
		strings.Contains(errStr, "EOF") ||
		strings.Contains(errStr, "i/o timeout") {
		return true
	}
	return false
}

// htmlToMarkdown converts HTML to a simplified markdown representation.
// This is a pure-Go implementation without external dependencies.
// Handles tables, lists, blockquotes, headings, links, bold, italic, and code.
