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
	"sync"
	"sync/atomic"
	"time"

	"github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

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

// dnsCacheEntry caches DNS resolution results for a hostname to prevent
// DNS rebinding (TOCTOU) attacks. The 5-minute TTL ensures stale entries
// are refreshed while still pinning IPs for the duration of a request.
type dnsCacheEntry struct {
	addrs   []net.IPAddr
	expires time.Time
}

type WebFetch struct {
	sessionsDir     string
	allowPrivateIPs bool
	client          *http.Client
	dnsCache        sync.Map // map[string]*dnsCacheEntry; key=hostname
}

func NewWebFetch(sessionsDir string, allowPrivateIPs bool) *WebFetch {
	wf := &WebFetch{
		sessionsDir:     sessionsDir,
		allowPrivateIPs: allowPrivateIPs,
	}
	wf.client = &http.Client{
		Transport: &http.Transport{
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
						if isPrivateIP(addr.IP) {
							return nil, fmt.Errorf("access to private IP %s is blocked: %w", addr.IP, errors.ErrPrivateIPBlocked)
						}
					}
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
					remoteAddr := tcpConn.RemoteAddr().(*net.TCPAddr)
					if !wf.allowPrivateIPs && isPrivateIP(remoteAddr.IP) {
						conn.Close()
						return nil, fmt.Errorf("connected to private IP %s is blocked: %w", remoteAddr.IP, errors.ErrPrivateIPBlocked)
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
				return err
			}
			if !wf.allowPrivateIPs {
				for _, addr := range addrs {
					if isPrivateIP(addr.IP) {
						slog.Warn("WebFetch redirect blocked by SSRF protection",
							"url", req.URL.String(), "ip", addr.IP)
						return fmt.Errorf("redirect to private IP %s is blocked: %w", addr.IP, errors.ErrPrivateIPBlocked)
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
	// Explicit cloud metadata endpoint check (defense in depth)
	if ip4 := ip.To4(); ip4 != nil {
		if ip4[0] == 169 && ip4[1] == 254 && ip4[2] == 169 && ip4[3] == 254 {
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

// resolveAndCache resolves DNS for a hostname using a sync.Map cache with
// 5-minute TTL. Caching pins IPs for the request lifecycle, preventing DNS
// rebinding (TOCTOU) attacks where an attacker changes DNS between resolution
// and connection.
func (t *WebFetch) resolveAndCache(ctx context.Context, host string) ([]net.IPAddr, error) {
	// Check for literal IP — no caching needed
	if ip := net.ParseIP(host); ip != nil {
		return []net.IPAddr{{IP: ip}}, nil
	}

	now := time.Now()
	ttl := DNSCacheTTL

	// Check cache
	if cached, ok := t.dnsCache.Load(host); ok {
		entry := cached.(*dnsCacheEntry)
		if now.Before(entry.expires) {
			return entry.addrs, nil
		}
		// Expired — fall through to re-resolve
	}

	// Resolve
	resolver := &net.Resolver{PreferGo: true}
	addrs, err := resolver.LookupIPAddr(ctx, host)
	if err != nil {
		return nil, fmt.Errorf("DNS resolution failed for %s: %w", host, err)
	}
	if len(addrs) == 0 {
		return nil, fmt.Errorf("no IP addresses found for %s", host)
	}

	// Store in cache
	entry := &dnsCacheEntry{
		addrs:   addrs,
		expires: now.Add(ttl),
	}
	t.dnsCache.Store(host, entry)

	return addrs, nil
}

func (t *WebFetch) Name() string {
	return "WebFetch"
}

func (t *WebFetch) Description() string {
	return "Fetch content from a URL and return it as markdown, text, or raw HTML."
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
			"format": {"type": "string", "enum": ["text", "markdown", "html"], "description": "Output format", "default": "markdown"}
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

	// SSRF protection is handled by the dialer's resolver in the HTTP client.
	// No separate resolveAndCheck call needed here — the dialer resolves DNS,
	// checks for private IPs, and pins the IP for the connection.

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
	defer resp.Body.Close()

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

	elapsed := time.Since(start).Milliseconds()
	return types.ToolResult{
		Output: fmt.Sprintf("Fetched %s (%s)\nContent-Type: %s\nSize: %d bytes\n\n%s",
			urlStr, resp.Status, contentType, len(result), result),
		DurationMs: elapsed,
	}, nil
}

// htmlToMarkdown converts HTML to a simplified markdown representation.
// This is a pure-Go implementation without external dependencies.
// Computes lowercase HTML once and reuses it across all tag operations,
// avoiding repeated O(N) ToLower allocations.
func htmlToMarkdown(html string) string {
	// Strip script and style elements first (before lowercase computation
	// since stripTags works on raw HTML)
	html = stripTags(html, "script", "style")

	// Compute lowercase once for all case-insensitive tag matching
	lower := strings.ToLower(html)

	// Handle common block elements with newlines — pass pre-computed lowercase
	html = replaceBlockTag(html, lower, "p", "\n\n")
	// Recompute lower after each replacement since string positions shift
	lower = strings.ToLower(html)
	html = replaceBlockTag(html, lower, "div", "\n")
	lower = strings.ToLower(html)
	html = replaceBlockTag(html, lower, "br", "\n")
	lower = strings.ToLower(html)
	html = replaceBlockTag(html, lower, "li", "\n- ")
	lower = strings.ToLower(html)
	html = replaceBlockTag(html, lower, "h1", "\n# ")
	lower = strings.ToLower(html)
	html = replaceBlockTag(html, lower, "h2", "\n## ")
	lower = strings.ToLower(html)
	html = replaceBlockTag(html, lower, "h3", "\n### ")
	lower = strings.ToLower(html)
	html = replaceBlockTag(html, lower, "h4", "\n#### ")

	// Handle links: <a href="url">text</a> -> [text](url)
	lower = strings.ToLower(html)
	html = convertLinks(html, lower)

	// Handle bold and italic
	lower = strings.ToLower(html)
	html = replaceInlineTag(html, lower, "strong", "**")
	lower = strings.ToLower(html)
	html = replaceInlineTag(html, lower, "b", "**")
	lower = strings.ToLower(html)
	html = replaceInlineTag(html, lower, "em", "*")
	lower = strings.ToLower(html)
	html = replaceInlineTag(html, lower, "i", "*")
	lower = strings.ToLower(html)
	html = replaceInlineTag(html, lower, "code", "`")

	// Strip all remaining tags
	html = stripAllTags(html)

	// Decode common HTML entities
	html = decodeHTMLEntities(html)

	// Normalize whitespace
	html = normalizeWhitespace(html)

	return strings.TrimSpace(html)
}

// htmlToText extracts plain text from HTML.
func htmlToText(html string) string {
	html = stripTags(html, "script", "style")
	lower := strings.ToLower(html)
	html = replaceBlockTag(html, lower, "p", "\n\n")
	lower = strings.ToLower(html)
	html = replaceBlockTag(html, lower, "br", "\n")
	lower = strings.ToLower(html)
	html = replaceBlockTag(html, lower, "li", "\n")
	html = stripAllTags(html)

	// Decode entities
	html = decodeHTMLEntities(html)

	return normalizeWhitespace(html)
}

// stripTags removes all occurrences of the given HTML tags and their content.
// Uses strings.Builder for efficient string construction instead of O(N²) concatenation.
// Limitation: does not correctly handle nested same-type tags (e.g., nested
// <script> blocks). This is acceptable for script/style stripping where such
// nesting is extremely rare in practice.
func stripTags(html string, tags ...string) string {
	for _, tag := range tags {
		openTag := "<" + tag
		closeTag := "</" + tag + ">"
		var b strings.Builder
		b.Grow(len(html))
		pos := 0
		for {
			start := strings.Index(html[pos:], openTag)
			if start == -1 {
				break
			}
			start += pos

			tagEnd := strings.IndexByte(html[start:], '>')
			if tagEnd == -1 {
				break
			}
			tagEnd += start + 1

			// Check for self-closing tag (/> at end)
			if strings.HasSuffix(html[start:tagEnd], "/") {
				b.WriteString(html[pos:start])
				pos = tagEnd
				continue
			}

			// Find closing tag
			closeStart := strings.Index(html[tagEnd:], closeTag)
			if closeStart == -1 {
				break
			}
			closeStart += tagEnd
			closeEnd := closeStart + len(closeTag)
			b.WriteString(html[pos:start])
			pos = closeEnd
		}
		b.WriteString(html[pos:])
		html = b.String()
	}
	return html
}

func replaceBlockTag(html, lower, tag, replacement string) string {
	openTag := "<" + tag
	closeTag := "</" + tag + ">"
	var result strings.Builder
	result.Grow(len(html))
	pos := 0
	for {
		start := strings.Index(lower[pos:], openTag)
		if start == -1 {
			break
		}
		start += pos

		tagEnd := strings.IndexByte(html[start:], '>')
		if tagEnd == -1 {
			break
		}
		tagEnd += start + 1

		closeStart := strings.Index(lower[tagEnd:], closeTag)
		if closeStart == -1 {
			break
		}
		closeStart += tagEnd
		closeEnd := closeStart + len(closeTag)

		result.WriteString(html[pos:start])
		result.WriteString(replacement)
		result.WriteString(html[tagEnd:closeStart])
		pos = closeEnd
	}
	result.WriteString(html[pos:])
	return result.String()
}

func replaceInlineTag(html, lower, tag, marker string) string {
	openTag := "<" + tag
	closeTag := "</" + tag + ">"
	for {
		start := strings.Index(lower, openTag)
		if start == -1 {
			break
		}

		tagEnd := strings.IndexByte(html[start:], '>')
		if tagEnd == -1 {
			break
		}
		tagEnd += start + 1

		closeStart := strings.Index(lower[tagEnd:], closeTag)
		if closeStart == -1 {
			break
		}
		closeStart += tagEnd
		closeEnd := closeStart + len(closeTag)

		inner := html[tagEnd:closeStart]
		html = html[:start] + marker + inner + marker + html[closeEnd:]
		// Recompute lower for next iteration since string positions shift
		lower = strings.ToLower(html)
	}
	return html
}

func convertLinks(html, lower string) string {
	for {
		start := strings.Index(lower, "<a ")
		if start == -1 {
			break
		}

		// Find href
		hrefStart := strings.Index(lower[start:], "href=")
		if hrefStart == -1 {
			// No href, skip
			tagEnd := strings.IndexByte(html[start:], '>')
			if tagEnd == -1 {
				break
			}
			html = html[:start] + html[start+tagEnd+1:]
			lower = strings.ToLower(html)
			continue
		}
		hrefStart += start + 5 // len("href=") = 5

		// Bounds check before accessing the character after href=
		if hrefStart >= len(html) {
			break
		}

		// Extract URL (handle quoted and unquoted)
		var url string
		quoteChar := html[hrefStart]
		if quoteChar == '"' || quoteChar == '\'' {
			urlStart := hrefStart + 1
			if urlStart >= len(html) {
				break
			}
			urlEnd := strings.Index(html[urlStart:], string(quoteChar))
			if urlEnd == -1 {
				break
			}
			urlEnd += urlStart
			url = html[urlStart:urlEnd]
		} else {
			// Unquoted href — read until whitespace or >
			urlEnd := hrefStart
			for urlEnd < len(html) && html[urlEnd] != ' ' && html[urlEnd] != '>' && html[urlEnd] != '\t' && html[urlEnd] != '\n' {
				urlEnd++
			}
			url = html[hrefStart:urlEnd]
		}

		// Find end of opening tag
		tagEnd := strings.IndexByte(html[start:], '>')
		if tagEnd == -1 {
			break
		}
		tagEnd += start + 1

		// Find closing tag
		closeTag := "</a>"
		closeStart := strings.Index(lower[tagEnd:], closeTag)
		if closeStart == -1 {
			break
		}
		closeStart += tagEnd
		closeEnd := closeStart + len(closeTag)

		text := html[tagEnd:closeStart]
		replacement := "[" + text + "](" + url + ")"
		html = html[:start] + replacement + html[closeEnd:]
		// Recompute lower for next iteration since string positions shift
		lower = strings.ToLower(html)
	}
	return html
}

// decodeHTMLEntities replaces common HTML entities with their character equivalents.
func decodeHTMLEntities(s string) string {
	s = strings.ReplaceAll(s, "&amp;", "&")
	s = strings.ReplaceAll(s, "&lt;", "<")
	s = strings.ReplaceAll(s, "&gt;", ">")
	s = strings.ReplaceAll(s, "&quot;", "\"")
	s = strings.ReplaceAll(s, "&#39;", "'")
	s = strings.ReplaceAll(s, "&nbsp;", " ")
	return s
}

func stripAllTags(html string) string {
	var b strings.Builder
	inTag := false
	for _, ch := range html {
		if ch == '<' {
			inTag = true
			continue
		}
		if ch == '>' {
			inTag = false
			continue
		}
		if !inTag {
			b.WriteRune(ch)
		}
	}
	return b.String()
}

func normalizeWhitespace(s string) string {
	var b strings.Builder
	b.Grow(len(s))
	prevNewlines := 0
	inSpace := false
	for _, ch := range s {
		if ch == '\n' {
			prevNewlines++
			inSpace = false
			if prevNewlines <= 2 {
				b.WriteRune(ch)
			}
		} else if ch == ' ' || ch == '\t' {
			prevNewlines = 0
			if !inSpace {
				b.WriteRune(' ')
				inSpace = true
			}
		} else {
			prevNewlines = 0
			inSpace = false
			b.WriteRune(ch)
		}
	}
	return strings.TrimSpace(b.String())
}
