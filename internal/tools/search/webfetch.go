package search

import (
	"context"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"
	"sync/atomic"
	"time"

	"github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
)

var _ types.Tool = (*WebFetch)(nil)

// Constants from original constants.go (copied to avoid circular imports)
const (
	MaxRedirects       = 20
	DefaultTimeoutSecs = 30
	MaxTimeoutSecs     = 120
	DNSCacheTTL        = 5 * time.Minute
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
func GetVersion() string {
	if v := Version.Load(); v != nil {
		if vs, ok := v.(string); ok && vs != "" {
			return vs
		}
	}
	return "dev"
}

// WebFetch fetches URLs with automatic HTML-to-markdown/text conversion.
type WebFetch struct {
	AllowPrivateIPs bool
	httpClient      *http.Client
	timeout         time.Duration
	dnsCache        *DNSCache
}

// NewWebFetch creates a new WebFetch tool instance.
func NewWebFetch(workDir string, timeoutSecs int, dnsCache *DNSCache) *WebFetch {
	if timeoutSecs <= 0 || timeoutSecs > MaxTimeoutSecs {
		timeoutSecs = DefaultTimeoutSecs
	}
	return &WebFetch{
		httpClient: &http.Client{
			Timeout: time.Duration(timeoutSecs) * time.Second,
			CheckRedirect: func(req *http.Request, via []*http.Request) error {
				if len(via) >= MaxRedirects {
					return http.ErrUseLastResponse
				}
				return nil
			},
		},
		timeout:         time.Duration(timeoutSecs) * time.Second,
		AllowPrivateIPs: true,
		dnsCache:        dnsCache,
	}
}

func (t *WebFetch) Name() string {
	return "WebFetch"
}

func (t *WebFetch) Description() string {
	return "Fetch content from a URL with automatic format conversion (HTML to markdown/text). Supports HTTP/HTTPS, redirects, custom headers, and timeout configuration."
}

func (t *WebFetch) RiskLevel() types.RiskLevel {
	return types.RiskMedium
}

func (t *WebFetch) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"url": {"type": "string", "description": "The URL to fetch"},
			"method": {"type": "string", "description": "HTTP method (GET, POST, etc.)", "default": "GET"},
			"headers": {"type": "object", "description": "Custom HTTP headers"},
			"body": {"type": "string", "description": "Request body for POST/PUT"},
			"format": {"type": "string", "description": "Output format: markdown, text, html", "default": "markdown", "enum": ["markdown", "text", "html"]},
			"timeout": {"type": "integer", "description": "Request timeout in seconds", "default": 30}
		},
		"required": ["url"]
	}`
}

func (t *WebFetch) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	urlRaw, ok := input.Params["url"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: url", errors.ErrToolExecution)
	}
	urlStr, ok := urlRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter url must be a string", errors.ErrToolExecution)
	}

	parsedURL, err := url.Parse(urlStr)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: invalid URL: %w", errors.ErrToolExecution, err)
	}
	if parsedURL.Scheme != "http" && parsedURL.Scheme != "https" {
		return types.ToolResult{}, fmt.Errorf("%w: only http/https URLs are supported", errors.ErrToolExecution)
	}

	// Block private IPs (SSRF protection)
	host := parsedURL.Hostname()
	if !t.AllowPrivateIPs && IsPrivateIPFromHost(host) {
		return types.ToolResult{}, fmt.Errorf("%w: access to private IP blocked", errors.ErrPrivateIPBlocked)
	}

	method := "GET"
	if mRaw, ok := input.Params["method"]; ok {
		if mStr, ok := mRaw.(string); ok {
			method = strings.ToUpper(mStr)
		}
	}

	format := "markdown"
	if fRaw, ok := input.Params["format"]; ok {
		if fStr, ok := fRaw.(string); ok {
			format = strings.ToLower(fStr)
		}
	}
	if format != "markdown" && format != "text" && format != "html" {
		return types.ToolResult{}, fmt.Errorf("%w: format must be markdown, text, or html", errors.ErrToolExecution)
	}

	timeout := t.timeout
	if tRaw, ok := input.Params["timeout"]; ok {
		if tInt, ok := tRaw.(float64); ok && tInt > 0 && tInt <= MaxTimeoutSecs {
			timeout = time.Duration(tInt) * time.Second
		}
	}

	var body io.Reader
	if bRaw, ok := input.Params["body"]; ok {
		if bStr, ok := bRaw.(string); ok {
			body = strings.NewReader(bStr)
		}
	}

	maxRedirects := MaxRedirects
	if mrRaw, ok := input.Params["max_redirects"]; ok {
		if mrInt, ok := mrRaw.(float64); ok && mrInt >= 0 && mrInt <= MaxRedirects {
			maxRedirects = int(mrInt)
		}
	}

	req, err := http.NewRequestWithContext(ctx, method, urlStr, body)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: failed to create request: %w", errors.ErrToolExecution, err)
	}

	if hRaw, ok := input.Params["headers"]; ok {
		if hMap, ok := hRaw.(map[string]any); ok {
			for k, v := range hMap {
				if vStr, ok := v.(string); ok {
					req.Header.Set(k, vStr)
				}
			}
		}
	}

	req.Header.Set("User-Agent", fmt.Sprintf("M31A/%s", GetVersion()))

	client := t.httpClient
	if timeout != t.timeout {
		client = &http.Client{
			Timeout: timeout,
			CheckRedirect: func(req *http.Request, via []*http.Request) error {
				if len(via) >= maxRedirects {
					return http.ErrUseLastResponse
				}
				return nil
			},
			Transport: t.httpClient.Transport,
		}
	}

	resp, err := client.Do(req)
	if err != nil {
		if ctx.Err() != nil {
			return types.ToolResult{}, ctx.Err()
		}
		return types.ToolResult{}, fmt.Errorf("%w: request failed: %w", errors.ErrToolExecution, err)
	}
	defer func() { _ = resp.Body.Close() }()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: failed to read response body: %w", errors.ErrToolExecution, err)
	}
	if len(respBody) > types.MaxFileSize {
		return types.ToolResult{}, fmt.Errorf("%w: response exceeds 5MB limit", errors.ErrToolExecution)
	}

	contentType := resp.Header.Get("Content-Type")
	isHTML := strings.Contains(contentType, "html")

	var result string
	switch {
	case isHTML && format == "markdown":
		result = HtmlToMarkdown(string(respBody))
	case isHTML && format == "text":
		result = HtmlToText(string(respBody))
	case isHTML && format == "html":
		result = string(respBody)
	default:
		result = string(respBody)
	}

	return types.ToolResult{
		Output:     fmt.Sprintf("Fetched %s (%s)\nContent-Type: %s\nSize: %d bytes\n\n%s", urlStr, resp.Status, contentType, len(result), result),
		DurationMs: time.Since(start).Milliseconds(),
	}, nil
}

func IsPrivateIPFromHost(host string) bool {
	ips, err := net.LookupIP(host)
	if err != nil {
		return false
	}
	for _, ip := range ips {
		if ip.IsLoopback() || ip.IsLinkLocalUnicast() || ip.IsLinkLocalMulticast() || ip.IsPrivate() {
			return true
		}
	}
	return false
}

// resolveAndCheck resolves the host from a URL and checks if it is a private IP.
func (t *WebFetch) ResolveAndCheck(ctx context.Context, urlStr string) error {
	parsedURL, err := url.Parse(urlStr)
	if err != nil {
		return err
	}
	host := parsedURL.Hostname()
	if host == "" {
		return fmt.Errorf("URL has no host")
	}
	if !t.AllowPrivateIPs && IsPrivateIPFromHost(host) {
		return fmt.Errorf("private IP not allowed: %s", host)
	}
	return nil
}

// resolveAndCache resolves the DNS for a hostname and caches the result.
func (t *WebFetch) ResolveAndCache(ctx context.Context, host string) ([]net.IP, error) {
	if t.dnsCache != nil {
		if entry, ok := t.dnsCache.cache.Load(host); ok {
			entry := entry.(*dnsCacheEntry)
			if time.Now().Before(entry.expires) {
				ips := make([]net.IP, len(entry.addrs))
				for i, addr := range entry.addrs {
					ips[i] = addr.IP
				}
				return ips, nil
			}
		}
	}
	ctx2, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	addrs, err := t.lookupIPWithContext(ctx2, host)
	if err != nil {
		return nil, err
	}
	if t.dnsCache != nil {

		t.dnsCache.cache.Store(host, &dnsCacheEntry{
			addrs:   toIPAddr(addrs),
			expires: time.Now().Add(5 * time.Minute),
		})
	}
	return addrs, nil
}

func (t *WebFetch) lookupIPWithContext(ctx context.Context, host string) ([]net.IP, error) {
	type result struct {
		ips []net.IP
		err error
	}
	ch := make(chan result, 1)
	go func() {
		ips, err := net.LookupIP(host)
		ch <- result{ips: ips, err: err}
	}()
	select {
	case r := <-ch:
		return r.ips, r.err
	case <-ctx.Done():
		return nil, ctx.Err()
	}
}

func toIPAddr(ips []net.IP) []net.IPAddr {
	addrs := make([]net.IPAddr, len(ips))
	for i, ip := range ips {
		addrs[i] = net.IPAddr{IP: ip}
	}
	return addrs
}
