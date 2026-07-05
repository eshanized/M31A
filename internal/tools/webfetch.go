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

const (
	maxRetries     = 3
	baseRetryDelay = 500 * time.Millisecond
)

type WebFetch struct {
	sessionsDir     string
	allowPrivateIPs bool
	client          *http.Client
	dnsCache        *DNSCache
	maxRetries      int
	baseRetryDelay  time.Duration
}

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
				return err
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
func htmlToMarkdown(rawHTML string) string {
	// Strip script and style elements first
	rawHTML = stripTags(rawHTML, "script", "style")

	// Compute lowercase once for all case-insensitive tag matching
	lower := strings.ToLower(rawHTML)

	// Handle tables: convert <table> to markdown tables
	rawHTML, lower = convertTables(rawHTML, lower)

	// Handle common block elements with newlines
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "p", "\n\n")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "div", "\n")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "br", "\n")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "hr", "\n---\n")

	// Handle headings with proper markdown
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "h1", "\n# ")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "h2", "\n## ")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "h3", "\n### ")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "h4", "\n#### ")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "h5", "\n##### ")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "h6", "\n###### ")

	// Handle blockquotes
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "blockquote", "\n> ")

	// Handle unordered lists
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "ul", "\n")
	rawHTML, _ = replaceBlockTag(rawHTML, lower, "li", "\n- ")

	// Handle ordered lists (convert <li> inside <ol> to numbered)
	rawHTML = convertOrderedList(rawHTML)

	// Handle links: <a href="url">text</a> -> [text](url)
	rawHTML = convertLinks(rawHTML, strings.ToLower(rawHTML))
	lower = strings.ToLower(rawHTML)

	// Handle images: <img src="url" alt="text"> -> ![text](url)
	rawHTML = convertImages(rawHTML, lower)
	lower = strings.ToLower(rawHTML)

	// Handle inline formatting
	rawHTML = replaceInlineTag(rawHTML, lower, "strong", "**")
	lower = strings.ToLower(rawHTML)
	rawHTML = replaceInlineTag(rawHTML, lower, "b", "**")
	lower = strings.ToLower(rawHTML)
	rawHTML = replaceInlineTag(rawHTML, lower, "em", "*")
	lower = strings.ToLower(rawHTML)
	rawHTML = replaceInlineTag(rawHTML, lower, "i", "*")
	lower = strings.ToLower(rawHTML)
	rawHTML = replaceInlineTag(rawHTML, lower, "code", "`")
	lower = strings.ToLower(rawHTML)
	rawHTML = replaceInlineTag(rawHTML, lower, "kbd", "`")
	lower = strings.ToLower(rawHTML)
	rawHTML = replaceInlineTag(rawHTML, lower, "samp", "`")
	lower = strings.ToLower(rawHTML)
	rawHTML = replaceInlineTag(rawHTML, lower, "pre", "\n```\n")

	// Handle <abbr title="...">text</abbr> -> text (title)
	rawHTML = convertAbbreviations(rawHTML, strings.ToLower(rawHTML))

	// Strip all remaining tags
	rawHTML = stripAllTags(rawHTML)

	// Decode common HTML entities
	rawHTML = decodeHTMLEntities(rawHTML)

	// Normalize whitespace
	rawHTML = normalizeWhitespace(rawHTML)

	return strings.TrimSpace(rawHTML)
}

// convertTables converts HTML tables to markdown table format.
func convertTables(rawHTML, lower string) (string, string) {
	type tableMatch struct {
		start int
		end   int
		html  string
	}

	var tables []tableMatch
	searchPos := 0

	for {
		idx := strings.Index(lower[searchPos:], "<table")
		if idx == -1 {
			break
		}
		start := searchPos + idx

		// Find matching </table>
		endIdx := strings.Index(lower[start:], "</table>")
		if endIdx == -1 {
			break
		}
		end := start + endIdx + len("</table>")

		tables = append(tables, tableMatch{
			start: start,
			end:   end,
			html:  rawHTML[start:end],
		})
		searchPos = end
	}

	if len(tables) == 0 {
		return rawHTML, lower
	}

	// Process tables in reverse order to preserve positions
	for i := len(tables) - 1; i >= 0; i-- {
		t := tables[i]
		md := htmlTableToMarkdown(t.html)
		rawHTML = rawHTML[:t.start] + "\n" + md + "\n" + rawHTML[t.end:]
	}

	return rawHTML, strings.ToLower(rawHTML)
}

// htmlTableToMarkdown converts a single HTML table to markdown.
func htmlTableToMarkdown(tableHTML string) string {
	var rows [][]string
	var headerIdx int

	// Find all <tr> sections
	lower := strings.ToLower(tableHTML)
	searchPos := 0
	for {
		trIdx := strings.Index(lower[searchPos:], "<tr")
		if trIdx == -1 {
			break
		}
		trStart := searchPos + trIdx
		trEndIdx := strings.Index(lower[trStart:], "</tr>")
		if trEndIdx == -1 {
			break
		}
		trEnd := trStart + trEndIdx + len("</tr>")
		trHTML := tableHTML[trStart:trEnd]

		// Check if this is a header row
		if strings.Contains(strings.ToLower(trHTML), "<th>") {
			headerIdx = len(rows)
		}

		// Extract cell contents
		var cells []string
		cellLower := strings.ToLower(trHTML)
		cellSearchPos := 0
		for {
			thIdx := strings.Index(cellLower[cellSearchPos:], "<t") // matches <th> and <td>
			if thIdx == -1 {
				break
			}
			cellStart := cellSearchPos + thIdx
			// Find >
			gtIdx := strings.IndexByte(cellLower[cellStart:], '>')
			if gtIdx == -1 {
				break
			}
			cellContentStart := cellStart + gtIdx + 1
			// Find closing tag
			closeTagIdx := strings.Index(cellLower[cellContentStart:], "</t")
			if closeTagIdx == -1 {
				break
			}
			cellContent := strings.TrimSpace(tableHTML[cellContentStart : cellContentStart+closeTagIdx])
			// Strip any inner tags
			cellContent = stripAllTags(cellContent)
			cellContent = decodeHTMLEntities(cellContent)
			cellContent = strings.TrimSpace(cellContent)
			// Escape pipes in cell content
			cellContent = strings.ReplaceAll(cellContent, "|", "\\|")
			cells = append(cells, cellContent)
			cellSearchPos = cellContentStart + closeTagIdx + 4
		}
		if len(cells) > 0 {
			rows = append(rows, cells)
		}
		searchPos = trEnd
	}

	if len(rows) == 0 {
		return tableHTML
	}

	// Determine max columns
	maxCols := 0
	for _, row := range rows {
		if len(row) > maxCols {
			maxCols = len(row)
		}
	}

	var sb strings.Builder
	for i, row := range rows {
		// Pad row to max columns
		for len(row) < maxCols {
			row = append(row, "")
		}
		sb.WriteString("| ")
		sb.WriteString(strings.Join(row, " | "))
		sb.WriteString(" |\n")

		// Add separator after header
		if i == headerIdx || (i == 0 && len(rows) > 1) {
			separators := make([]string, maxCols)
			for j := range separators {
				separators[j] = "---"
			}
			sb.WriteString("| ")
			sb.WriteString(strings.Join(separators, " | "))
			sb.WriteString(" |\n")
		}
	}

	return sb.String()
}

// convertOrderedList converts HTML ordered lists to numbered markdown lists.
func convertOrderedList(rawHTML string) string {
	// Find <ol>...</ol> blocks and convert <li> to numbered items
	lower := strings.ToLower(rawHTML)
	result := rawHTML
	searchPos := 0

	for {
		olIdx := strings.Index(lower[searchPos:], "<ol")
		if olIdx == -1 {
			break
		}
		olStart := searchPos + olIdx
		olEndIdx := strings.Index(lower[olStart:], "</ol>")
		if olEndIdx == -1 {
			break
		}
		olEnd := olStart + olEndIdx + len("</ol>")
		olHTML := rawHTML[olStart:olEnd]

		// Replace <li> with numbered items
		var sb strings.Builder
		counter := 1
		liLower := strings.ToLower(olHTML)
		liSearchPos := 0
		liPrevEnd := 0
		for {
			liIdx := strings.Index(liLower[liSearchPos:], "<li")
			if liIdx == -1 {
				// Copy remaining content
				sb.WriteString(olHTML[liPrevEnd:])
				break
			}
			liStart := liSearchPos + liIdx
			// Copy content before this <li>
			sb.WriteString(olHTML[liPrevEnd:liStart])

			// Find >
			gtIdx := strings.IndexByte(liLower[liStart:], '>')
			if gtIdx == -1 {
				sb.WriteString(olHTML[liPrevEnd:])
				break
			}
			contentStart := liStart + gtIdx + 1
			// Find </li>
			closeIdx := strings.Index(liLower[contentStart:], "</li>")
			if closeIdx == -1 {
				sb.WriteString(olHTML[liPrevEnd:])
				break
			}
			content := strings.TrimSpace(olHTML[contentStart : contentStart+closeIdx])
			fmt.Fprintf(&sb, "\n%d. %s", counter, content)
			counter++
			liPrevEnd = contentStart + closeIdx + len("</li>")
			liSearchPos = liPrevEnd
		}

		result = result[:olStart] + sb.String() + result[olEnd:]
		lower = strings.ToLower(result)
		searchPos = olStart + sb.Len()
	}

	return result
}

// convertImages converts <img> tags to markdown image syntax.
func convertImages(rawHTML, lower string) string {
	type imgMatch struct {
		start int
		end   int
		alt   string
		src   string
	}
	var matches []imgMatch

	searchPos := 0
	for {
		idx := strings.Index(lower[searchPos:], "<img")
		if idx == -1 {
			break
		}
		start := searchPos + idx
		// Find self-closing />
		endIdx := strings.Index(lower[start:], "/>")
		if endIdx == -1 {
			// Try >
			endIdx = strings.Index(lower[start:], ">")
			if endIdx == -1 {
				break
			}
		}
		end := start + endIdx + 2
		tagHTML := rawHTML[start:end]
		tagLower := lower[start:end]

		// Extract src
		src := ""
		if srcIdx := strings.Index(tagLower, "src="); srcIdx >= 0 {
			srcStart := srcIdx + 5
			if srcStart < len(tagLower) {
				quote := tagLower[srcStart]
				if quote == '"' || quote == '\'' {
					srcEnd := strings.Index(tagLower[srcStart+1:], string(quote))
					if srcEnd >= 0 {
						src = tagHTML[srcStart+1 : srcStart+1+srcEnd]
					}
				}
			}
		}

		// Extract alt
		alt := ""
		if altIdx := strings.Index(tagLower, "alt="); altIdx >= 0 {
			altStart := altIdx + 5
			if altStart < len(tagLower) {
				quote := tagLower[altStart]
				if quote == '"' || quote == '\'' {
					altEnd := strings.Index(tagLower[altStart+1:], string(quote))
					if altEnd >= 0 {
						alt = tagHTML[altStart+1 : altStart+1+altEnd]
					}
				}
			}
		}

		if src != "" {
			matches = append(matches, imgMatch{start: start, end: end, alt: alt, src: src})
		}
		searchPos = end
	}

	if len(matches) == 0 {
		return rawHTML
	}

	// Apply in reverse order
	result := rawHTML
	for i := len(matches) - 1; i >= 0; i-- {
		m := matches[i]
		md := fmt.Sprintf("![%s](%s)", m.alt, m.src)
		result = result[:m.start] + md + result[m.end:]
	}
	return result
}

// convertAbbreviations removes <abbr> tags, keeping just the text content.
func convertAbbreviations(rawHTML, lower string) string {
	// Simple approach: just strip <abbr ...> and </abbr> tags
	rawHTML = replaceInlineTag(rawHTML, lower, "abbr", "")
	return rawHTML
}

// htmlToText extracts plain text from HTML.
func htmlToText(rawHTML string) string {
	rawHTML = stripTags(rawHTML, "script", "style")
	lower := strings.ToLower(rawHTML)
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "p", "\n\n")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "br", "\n")
	rawHTML, lower = replaceBlockTag(rawHTML, lower, "li", "\n- ")
	rawHTML, _ = replaceBlockTag(rawHTML, lower, "blockquote", "\n> ")
	rawHTML = stripAllTags(rawHTML)

	// Decode entities
	rawHTML = decodeHTMLEntities(rawHTML)

	return normalizeWhitespace(rawHTML)
}

// stripTags removes all occurrences of the given HTML tags and their content.
// Uses strings.Builder for efficient string construction instead of O(N²) concatenation.
func stripTags(rawHTML string, tags ...string) string {
	for _, tag := range tags {
		openTag := "<" + tag
		closeTag := "</" + tag + ">"
		var b strings.Builder
		b.Grow(len(rawHTML))
		pos := 0
		for {
			start := strings.Index(rawHTML[pos:], openTag)
			if start == -1 {
				break
			}
			start += pos

			tagEnd := strings.IndexByte(rawHTML[start:], '>')
			if tagEnd == -1 {
				break
			}
			tagEnd += start + 1

			// Check for self-closing tag (/> at end)
			if strings.HasSuffix(rawHTML[start:tagEnd], "/") {
				b.WriteString(rawHTML[pos:start])
				pos = tagEnd
				continue
			}

			// Find closing tag
			closeStart := strings.Index(rawHTML[tagEnd:], closeTag)
			if closeStart == -1 {
				break
			}
			closeStart += tagEnd
			closeEnd := closeStart + len(closeTag)
			b.WriteString(rawHTML[pos:start])
			pos = closeEnd
		}
		b.WriteString(rawHTML[pos:])
		rawHTML = b.String()
	}
	return rawHTML
}

// replaceBlockTag replaces block-level HTML tags with the given replacement string.
// Returns both the modified HTML and its pre-computed lowercase, avoiding the
// caller needing to recompute strings.ToLower after every call.
func replaceBlockTag(rawHTML, lower, tag, replacement string) (string, string) {
	openTag := "<" + tag
	closeTag := "</" + tag + ">"
	var result strings.Builder
	result.Grow(len(rawHTML))
	pos := 0
	for {
		start := strings.Index(lower[pos:], openTag)
		if start == -1 {
			break
		}
		start += pos

		tagEnd := strings.IndexByte(rawHTML[start:], '>')
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

		result.WriteString(rawHTML[pos:start])
		result.WriteString(replacement)
		result.WriteString(rawHTML[tagEnd:closeStart])
		pos = closeEnd
	}
	result.WriteString(rawHTML[pos:])
	newHTML := result.String()
	return newHTML, strings.ToLower(newHTML)
}

// replaceInlineTag replaces inline HTML tags with the given marker around inner content.
// Collects all match positions first, then applies replacements in reverse order
// so earlier positions remain valid. Only one ToLower recompute is needed after
// the caller uses the result.
func replaceInlineTag(rawHTML, lower, tag, marker string) string {
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

		tagEnd := strings.IndexByte(rawHTML[start:], '>')
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
		return rawHTML
	}

	// Build result in forward order
	var b strings.Builder
	b.Grow(len(rawHTML) + len(matches)*(len(marker)*2))
	prevEnd := 0
	for _, m := range matches {
		b.WriteString(rawHTML[prevEnd:m.openStart])
		b.WriteString(marker)
		// Find the '>' of the opening tag to get inner content start
		openGT := strings.IndexByte(rawHTML[m.openStart:], '>')
		if openGT == -1 {
			continue
		}
		innerStart := m.openStart + openGT + 1 // position after '>'
		b.WriteString(rawHTML[innerStart:m.closeStart])
		b.WriteString(marker)
		prevEnd = m.closeEnd
	}
	b.WriteString(rawHTML[prevEnd:])
	return b.String()
}

// convertLinks converts <a href="url">text</a> to [text](url).
// Collects all link matches first, then applies replacements in reverse order
// so position shifts don't invalidate earlier indices.
func convertLinks(rawHTML, lower string) string {
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
			tagEnd := strings.IndexByte(rawHTML[start:], '>')
			if tagEnd == -1 {
				break
			}
			searchPos = start + tagEnd + 1
			continue
		}
		hrefStart := start + hrefIdx + 5 // len("href=") = 5

		if hrefStart >= len(rawHTML) {
			break
		}

		// Extract URL (handle quoted and unquoted)
		var linkURL string
		quoteChar := rawHTML[hrefStart]
		if quoteChar == '"' || quoteChar == '\'' {
			urlStart := hrefStart + 1
			if urlStart >= len(rawHTML) {
				break
			}
			urlEnd := strings.Index(rawHTML[urlStart:], string(quoteChar))
			if urlEnd == -1 {
				break
			}
			linkURL = rawHTML[urlStart : urlStart+urlEnd]
		} else {
			urlEnd := hrefStart
			for urlEnd < len(rawHTML) && rawHTML[urlEnd] != ' ' && rawHTML[urlEnd] != '>' && rawHTML[urlEnd] != '\t' && rawHTML[urlEnd] != '\n' {
				urlEnd++
			}
			linkURL = rawHTML[hrefStart:urlEnd]
		}

		// Find end of opening tag
		openGT := strings.IndexByte(rawHTML[start:], '>')
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

		text := rawHTML[tagEndPos:closeStart]
		matches = append(matches, linkMatch{
			start:    start,
			closeEnd: closeEnd,
			url:      linkURL,
			text:     text,
		})
		searchPos = closeEnd
	}

	if len(matches) == 0 {
		return rawHTML
	}

	// Build the result by writing non-matched segments in forward order
	result := make([]byte, 0, len(rawHTML)+len(matches)*4)
	result = append(result, rawHTML[:matches[0].start]...)
	for i, m := range matches {
		result = append(result, '[')
		result = append(result, m.text...)
		result = append(result, ']')
		result = append(result, '(')
		result = append(result, m.url...)
		result = append(result, ')')
		if i+1 < len(matches) {
			result = append(result, rawHTML[m.closeEnd:matches[i+1].start]...)
		}
	}
	result = append(result, rawHTML[matches[len(matches)-1].closeEnd:]...)
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

func stripAllTags(rawHTML string) string {
	var b strings.Builder
	inTag := false
	for _, ch := range rawHTML {
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

// Close releases idle connections held by the HTTP client's transport.
// Call this during shutdown to prevent connection leaks.
func (wf *WebFetch) Close() {
	wf.client.CloseIdleConnections()
}
