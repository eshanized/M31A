package tools

import (
	"context"
	"fmt"
	"html"
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

// Compile-time interface check
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
	// dnsInsertsSinceEvict counts Store calls; eviction runs when it exceeds
	// dnsEvictThreshold. Prevents unbounded memory growth of dnsCache (BUG-08).
	dnsInsertsSinceEvict atomic.Int32
}

const dnsEvictThreshold int32 = 64

func NewWebFetch(sessionsDir string, allowPrivateIPs bool) *WebFetch {
	wf := &WebFetch{
		sessionsDir:     sessionsDir,
		allowPrivateIPs: allowPrivateIPs,
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
						_ = conn.Close()
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

	// Periodic eviction: prevents unbounded growth when many unique hosts
	// are fetched over a long session. Threshold-gated so it rarely runs.
	if t.dnsInsertsSinceEvict.Add(1) >= dnsEvictThreshold {
		t.dnsInsertsSinceEvict.Store(0)
		t.dnsCache.Range(func(key, value any) bool {
			e := value.(*dnsCacheEntry)
			if now.After(e.expires) {
				t.dnsCache.Delete(key)
			}
			return true
		})
	}

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

	// Handle common block elements with newlines — each call returns the
	// updated lowercase string, avoiding redundant ToLower calls.
	html, lower = replaceBlockTag(html, lower, "p", "\n\n")
	html, lower = replaceBlockTag(html, lower, "div", "\n")
	html, lower = replaceBlockTag(html, lower, "br", "\n")
	html, lower = replaceBlockTag(html, lower, "li", "\n- ")
	html, lower = replaceBlockTag(html, lower, "h1", "\n# ")
	html, lower = replaceBlockTag(html, lower, "h2", "\n## ")
	html, lower = replaceBlockTag(html, lower, "h3", "\n### ")
	html, lower = replaceBlockTag(html, lower, "h4", "\n#### ")

	// Handle links: <a href="url">text</a> -> [text](url)
	// convertLinks collects all matches before mutating, so only one
	// ToLower recompute is needed after the batch.
	html = convertLinks(html, lower)
	lower = strings.ToLower(html)

	// Handle bold and italic — each collects matches first, so one
	// ToLower recompute is needed per function.
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
	html, lower = replaceBlockTag(html, lower, "p", "\n\n")
	html, lower = replaceBlockTag(html, lower, "br", "\n")
	html, _ = replaceBlockTag(html, lower, "li", "\n")
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

// replaceBlockTag replaces block-level HTML tags with the given replacement string.
// Returns both the modified HTML and its pre-computed lowercase, avoiding the
// caller needing to recompute strings.ToLower after every call.
func replaceBlockTag(html, lower, tag, replacement string) (string, string) {
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
	newHTML := result.String()
	return newHTML, strings.ToLower(newHTML)
}

// replaceInlineTag replaces inline HTML tags with the given marker around inner content.
// Collects all match positions first, then applies replacements in reverse order
// so earlier positions remain valid. Only one ToLower recompute is needed after
// the caller uses the result.
func replaceInlineTag(html, lower, tag, marker string) string {
	openTag := "<" + tag
	closeTag := "</" + tag + ">"

	type tagMatch struct {
		openStart  int // position of '<' of opening tag
		closeStart int // position of '<' of closing tag
		closeEnd   int // position after '>' of closing tag
	}
	var matches []tagMatch

	searchPos := 0
	for {
		start := strings.Index(lower[searchPos:], openTag)
		if start == -1 {
			break
		}
		start += searchPos

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

		matches = append(matches, tagMatch{
			openStart:  start,
			closeStart: closeStart,
			closeEnd:   closeEnd,
		})
		searchPos = closeEnd
	}

	if len(matches) == 0 {
		return html
	}

	// Build result in forward order
	var b strings.Builder
	b.Grow(len(html) + len(matches)*(len(marker)*2))
	prevEnd := 0
	for _, m := range matches {
		b.WriteString(html[prevEnd:m.openStart])
		b.WriteString(marker)
		// Find the '>' of the opening tag to get inner content start
		openGT := strings.IndexByte(html[m.openStart:], '>')
		if openGT == -1 {
			continue
		}
		innerStart := m.openStart + openGT + 1 // position after '>'
		b.WriteString(html[innerStart:m.closeStart])
		b.WriteString(marker)
		prevEnd = m.closeEnd
	}
	b.WriteString(html[prevEnd:])
	return b.String()
}

// convertLinks converts <a href="url">text</a> to [text](url).
// Collects all link matches first, then applies replacements in reverse order
// so position shifts don't invalidate earlier indices.
func convertLinks(html, lower string) string {
	type linkMatch struct {
		start    int
		closeEnd int
		url      string
		text     string
	}
	var matches []linkMatch

	searchPos := 0
	for {
		start := strings.Index(lower[searchPos:], "<a ")
		if start == -1 {
			break
		}
		start += searchPos

		// Find href
		hrefIdx := strings.Index(lower[start:], "href=")
		if hrefIdx == -1 {
			// No href, skip this tag
			tagEnd := strings.IndexByte(html[start:], '>')
			if tagEnd == -1 {
				break
			}
			searchPos = start + tagEnd + 1
			continue
		}
		hrefStart := start + hrefIdx + 5 // len("href=") = 5

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
			url = html[urlStart : urlStart+urlEnd]
		} else {
			urlEnd := hrefStart
			for urlEnd < len(html) && html[urlEnd] != ' ' && html[urlEnd] != '>' && html[urlEnd] != '\t' && html[urlEnd] != '\n' {
				urlEnd++
			}
			url = html[hrefStart:urlEnd]
		}

		// Find end of opening tag
		openGT := strings.IndexByte(html[start:], '>')
		if openGT == -1 {
			break
		}
		tagEndPos := start + openGT + 1

		// Find closing tag
		closeStart := strings.Index(lower[tagEndPos:], "</a>")
		if closeStart == -1 {
			break
		}
		closeStart += tagEndPos
		closeEnd := closeStart + 4 // len("</a>")

		text := html[tagEndPos:closeStart]
		matches = append(matches, linkMatch{
			start:    start,
			closeEnd: closeEnd,
			url:      url,
			text:     text,
		})
		searchPos = closeEnd
	}

	if len(matches) == 0 {
		return html
	}

	// Build the result by writing non-matched segments in forward order
	result := make([]byte, 0, len(html)+len(matches)*4)
	result = append(result, html[:matches[0].start]...)
	for i, m := range matches {
		result = append(result, '[')
		result = append(result, m.text...)
		result = append(result, ']')
		result = append(result, '(')
		result = append(result, m.url...)
		result = append(result, ')')
		if i+1 < len(matches) {
			result = append(result, html[m.closeEnd:matches[i+1].start]...)
		}
	}
	result = append(result, html[matches[len(matches)-1].closeEnd:]...)
	return string(result)
}

// decodeHTMLEntities replaces HTML entities with their character equivalents.
// Uses the standard library's html.UnescapeString for comprehensive coverage
// of named, numeric, and hex entities. Non-breaking spaces are normalised to
// regular spaces for downstream text processing.
func decodeHTMLEntities(s string) string {
	s = html.UnescapeString(s)
	return strings.ReplaceAll(s, "\u00a0", " ")
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
		switch ch {
		case '\n':
			prevNewlines++
			inSpace = false
			if prevNewlines <= 2 {
				b.WriteRune(ch)
			}
		case ' ', '\t':
			prevNewlines = 0
			if !inSpace {
				b.WriteRune(' ')
				inSpace = true
			}
		default:
			prevNewlines = 0
			inSpace = false
			b.WriteRune(ch)
		}
	}
	return strings.TrimSpace(b.String())
}
