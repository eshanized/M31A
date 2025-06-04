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
	"time"

	"github.com/eshanized/M31A/internal/types"
)

type WebFetch struct {
	sessionsDir     string
	allowPrivateIPs bool
}

func NewWebFetch(sessionsDir string, allowPrivateIPs bool) *WebFetch {
	return &WebFetch{
		sessionsDir:     sessionsDir,
		allowPrivateIPs: allowPrivateIPs,
	}
}

// isPrivateIP returns true if the IP is loopback, link-local, RFC1918,
// or cloud metadata. These should not be reachable by an external agent.
func isPrivateIP(ip net.IP) bool {
	if ip.IsLoopback() {
		return true
	}
	if ip.IsLinkLocalUnicast() || ip.IsLinkLocalMulticast() {
		return true
	}
	// RFC1918 private ranges
	if ip4 := ip.To4(); ip4 != nil {
		return ip4[0] == 10 ||
			(ip4[0] == 172 && ip4[1]&0xf0 == 16) ||
			(ip4[0] == 192 && ip4[1] == 168)
	}
	// fc00::/7 unique local
	if ip16 := ip.To16(); ip16 != nil {
		return ip16[0]&0xfe == 0xfc
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
			return fmt.Errorf("access to private IP %s is blocked (SSRF protection)", ip)
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
				return fmt.Errorf("host %s resolves to private IP %s (SSRF protection)", host, addr.IP)
			}
		}
	}

	return nil
}

func (t *WebFetch) Name() string {
	return "WebFetch"
}

func (t *WebFetch) Description() string {
	return "Fetch content from a URL and return it as markdown, text, or raw HTML."
}

func (t *WebFetch) RiskLevel() types.RiskLevel {
	return types.RiskSafe
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

	timeout := 30
	if tRaw, ok := input.Params["timeout"].(float64); ok {
		timeout = int(tRaw)
	}
	if timeout <= 0 || timeout > 120 {
		return types.ToolResult{}, fmt.Errorf("timeout must be between 1 and 120 seconds")
	}

	// SSRF protection: resolve and check hostname before connecting
	if err := t.resolveAndCheck(ctx, urlStr); err != nil {
		return types.ToolResult{}, err
	}

	// Create request
	req, err := http.NewRequestWithContext(ctx, "GET", urlStr, nil)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("failed to create request: %w", err)
	}

	// Browser user agent
	req.Header.Set("User-Agent", "M31A/1.0 (AI Coding Agent; +https://github.com/eshanized/M31A)")
	req.Header.Set("Accept", "text/html,application/xhtml+xml,application/xml,text/plain;q=0.9,*/*;q=0.8")
	req.Header.Set("Accept-Language", "en-US,en;q=0.5")

	// Execute request
	client := &http.Client{
		Timeout: time.Duration(timeout) * time.Second,
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			if len(via) >= 5 {
				return fmt.Errorf("stopped after 5 redirects")
			}
			// SSRF protection: check each redirect target
			if err := t.resolveAndCheck(ctx, req.URL.String()); err != nil {
				slog.Warn("WebFetch redirect blocked by SSRF protection",
					"url", req.URL.String(), "error", err)
				return err
			}
			return nil
		},
	}
	resp, err := client.Do(req)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	// Check status code
	if resp.StatusCode < 200 || resp.StatusCode >= 400 {
		return types.ToolResult{}, fmt.Errorf("HTTP %d: %s", resp.StatusCode, resp.Status)
	}

	// Read body with size limit (5MB)
	const maxSize = 5 * 1024 * 1024
	body, err := io.ReadAll(io.LimitReader(resp.Body, maxSize+1))
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("failed to read response body: %w", err)
	}
	if len(body) > maxSize {
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
func htmlToMarkdown(html string) string {
	// Strip script and style elements
	html = stripTags(html, "script", "style")

	// Handle common block elements with newlines
	html = replaceBlockTag(html, "p", "\n\n")
	html = replaceBlockTag(html, "div", "\n")
	html = replaceBlockTag(html, "br", "\n")
	html = replaceBlockTag(html, "li", "\n- ")
	html = replaceBlockTag(html, "h1", "\n# ")
	html = replaceBlockTag(html, "h2", "\n## ")
	html = replaceBlockTag(html, "h3", "\n### ")
	html = replaceBlockTag(html, "h4", "\n#### ")

	// Handle links: <a href="url">text</a> -> [text](url)
	html = convertLinks(html)

	// Handle bold and italic
	html = replaceInlineTag(html, "strong", "**")
	html = replaceInlineTag(html, "b", "**")
	html = replaceInlineTag(html, "em", "*")
	html = replaceInlineTag(html, "i", "*")
	html = replaceInlineTag(html, "code", "`")

	// Strip all remaining tags
	html = stripAllTags(html)

	// Decode common HTML entities
	html = strings.ReplaceAll(html, "&amp;", "&")
	html = strings.ReplaceAll(html, "&lt;", "<")
	html = strings.ReplaceAll(html, "&gt;", ">")
	html = strings.ReplaceAll(html, "&quot;", "\"")
	html = strings.ReplaceAll(html, "&#39;", "'")
	html = strings.ReplaceAll(html, "&nbsp;", " ")

	// Normalize whitespace
	html = normalizeWhitespace(html)

	return strings.TrimSpace(html)
}

// htmlToText extracts plain text from HTML.
func htmlToText(html string) string {
	html = stripTags(html, "script", "style")
	html = replaceBlockTag(html, "p", "\n\n")
	html = replaceBlockTag(html, "br", "\n")
	html = replaceBlockTag(html, "li", "\n")
	html = stripAllTags(html)

	// Decode entities
	html = strings.ReplaceAll(html, "&amp;", "&")
	html = strings.ReplaceAll(html, "&lt;", "<")
	html = strings.ReplaceAll(html, "&gt;", ">")
	html = strings.ReplaceAll(html, "&quot;", "\"")
	html = strings.ReplaceAll(html, "&#39;", "'")
	html = strings.ReplaceAll(html, "&nbsp;", " ")

	return normalizeWhitespace(html)
}

func stripTags(html string, tags ...string) string {
	for _, tag := range tags {
		for {
			start := strings.Index(html, "<"+tag)
			if start == -1 {
				break
			}
			end := strings.Index(html, "</"+tag+">")
			if end == -1 {
				break
			}
			end += len(tag) + 3 // len("</>") = 3
			html = html[:start] + html[end:]
		}
	}
	return html
}

func replaceBlockTag(html, tag, replacement string) string {
	lower := strings.ToLower(html)
	for {
		openTag := "<" + tag
		start := strings.Index(lower, openTag)
		if start == -1 {
			break
		}

		// Find end of opening tag
		tagEnd := strings.Index(html[start:], ">")
		if tagEnd == -1 {
			break
		}
		tagEnd += start + 1

		// Find closing tag
		closeTag := "</" + tag + ">"
		closeStart := strings.Index(lower[tagEnd:], closeTag)
		if closeStart == -1 {
			break
		}
		closeStart += tagEnd
		closeEnd := closeStart + len(closeTag)

		inner := html[tagEnd:closeStart]
		html = html[:start] + replacement + inner + html[closeEnd:]
		lower = strings.ToLower(html)
	}
	return html
}

func replaceInlineTag(html, tag, marker string) string {
	lower := strings.ToLower(html)
	for {
		openTag := "<" + tag
		start := strings.Index(lower, openTag)
		if start == -1 {
			break
		}

		tagEnd := strings.Index(html[start:], ">")
		if tagEnd == -1 {
			break
		}
		tagEnd += start + 1

		closeTag := "</" + tag + ">"
		closeStart := strings.Index(lower[tagEnd:], closeTag)
		if closeStart == -1 {
			break
		}
		closeStart += tagEnd
		closeEnd := closeStart + len(closeTag)

		inner := html[tagEnd:closeStart]
		html = html[:start] + marker + inner + marker + html[closeEnd:]
		lower = strings.ToLower(html)
	}
	return html
}

func convertLinks(html string) string {
	lower := strings.ToLower(html)
	for {
		start := strings.Index(lower, "<a ")
		if start == -1 {
			break
		}

		// Find href
		hrefStart := strings.Index(lower[start:], "href=")
		if hrefStart == -1 {
			// No href, skip
			tagEnd := strings.Index(html[start:], ">")
			if tagEnd == -1 {
				break
			}
			html = html[:start] + html[start+tagEnd+1:]
			lower = strings.ToLower(html)
			continue
		}
		hrefStart += start + 6 // len("href=") = 5 + 1 for space

		// Extract URL (handle quotes)
		quoteChar := html[hrefStart]
		urlStart := hrefStart + 1
		urlEnd := strings.Index(html[urlStart:], string(quoteChar))
		if urlEnd == -1 {
			break
		}
		urlEnd += urlStart
		url := html[urlStart:urlEnd]

		// Find end of opening tag
		tagEnd := strings.Index(html[start:], ">")
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
		html = html[:start] + "[" + text + "](" + url + ")" + html[closeEnd:]
		lower = strings.ToLower(html)
	}
	return html
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
	// Collapse multiple newlines
	for strings.Contains(s, "\n\n\n") {
		s = strings.ReplaceAll(s, "\n\n\n", "\n\n")
	}
	// Collapse multiple spaces
	var b strings.Builder
	inSpace := false
	for _, ch := range s {
		if ch == ' ' || ch == '\t' {
			if !inSpace {
				b.WriteRune(' ')
				inSpace = true
			}
		} else {
			b.WriteRune(ch)
			inSpace = false
		}
	}
	return strings.TrimSpace(b.String())
}
