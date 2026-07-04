package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*WebSearch)(nil)

type WebSearch struct {
	client          *http.Client
	baseURL         string
	maxResults      int
	allowPrivateIPs bool
	dnsCache        *DNSCache
}

func NewWebSearch(baseURL string) *WebSearch {
	if baseURL == "" {
		baseURL = DefaultSearchBaseURL
	}
	baseURL = strings.TrimRight(baseURL, "/")

	ws := &WebSearch{
		baseURL:    baseURL,
		maxResults: DefaultMaxSearchResults,
		dnsCache:   NewDNSCache(DNSCacheTTL, 64),
	}
	ws.client = &http.Client{
		Timeout: 30 * time.Second,
		Transport: &http.Transport{
			MaxIdleConns:        10,
			IdleConnTimeout:     60 * time.Second,
			TLSHandshakeTimeout: 10 * time.Second,
			DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
				host, port, err := net.SplitHostPort(addr)
				if err != nil {
					return nil, fmt.Errorf("invalid address: %w", err)
				}
				ips, err := ws.resolveAndCache(ctx, host)
				if err != nil {
					return nil, fmt.Errorf("DNS resolution failed for %s: %w", host, err)
				}
				if len(ips) == 0 {
					return nil, fmt.Errorf("no IP addresses resolved for %s", host)
				}
				if !ws.allowPrivateIPs {
					for _, ip := range ips {
						if isPrivateIP(ip.IP) || isReservedIP(ip.IP) {
							return nil, fmt.Errorf("blocked: %s is a private or reserved IP", ip.IP)
						}
					}
				}
				pinnedAddr := net.JoinHostPort(ips[0].IP.String(), port)
				dialer := &net.Dialer{Timeout: 10 * time.Second}
				return dialer.DialContext(ctx, network, pinnedAddr)
			},
		},
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			if len(via) >= 5 {
				return fmt.Errorf("too many redirects")
			}
			if ws.allowPrivateIPs {
				return nil
			}
			host := req.URL.Hostname()
			ips, err := ws.resolveAndCache(req.Context(), host)
			if err != nil {
				return fmt.Errorf("redirect DNS resolution failed: %w", err)
			}
			if len(ips) == 0 {
				return fmt.Errorf("no IP addresses resolved for redirect to %s", req.URL.Host)
			}
			for _, ip := range ips {
				if isPrivateIP(ip.IP) || isReservedIP(ip.IP) {
					return fmt.Errorf("redirect to private/reserved IP %s is blocked: %w", ip.IP, errors.ErrPrivateIPBlocked)
				}
			}
			return nil
		},
	}
	return ws
}

// resolveAndCache resolves DNS for a hostname using the shared DNS cache.
func (t *WebSearch) resolveAndCache(ctx context.Context, host string) ([]net.IPAddr, error) {
	return t.dnsCache.Resolve(ctx, host)
}

func (t *WebSearch) Name() string               { return "WebSearch" }
func (t *WebSearch) RiskLevel() types.RiskLevel { return types.RiskMedium }

func (t *WebSearch) Description() string {
	return `Search the web and return structured results (title, URL, snippet).
Use this to discover information, documentation, or solutions to problems.
Powered by SearXNG (privacy-respecting meta-search engine).`
}

func (t *WebSearch) ParameterSchema() string {
	return `{
  "type": "object",
  "properties": {
    "query":       {"type": "string", "description": "Search query (max 500 chars)"},
    "max_results": {"type": "integer", "description": "Max results to return (1-10, default 5)"},
    "engines":     {"type": "string", "description": "Comma-separated search engines (e.g. 'google,bing,duckduckgo')"}
  },
  "required": ["query"]
}`
}

type searxngResponse struct {
	Results []searxngResult `json:"results"`
}

type searxngResult struct {
	Title   string `json:"title"`
	URL     string `json:"url"`
	Content string `json:"content"`
	Engine  string `json:"engine"`
}

func (t *WebSearch) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	queryRaw, ok := input.Params["query"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: query")
	}
	query, ok := queryRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter query must be a string")
	}
	query = strings.TrimSpace(query)
	if query == "" {
		return types.ToolResult{}, fmt.Errorf("query cannot be empty")
	}
	if len(query) > MaxSearchQueryLength {
		return types.ToolResult{}, fmt.Errorf("query too long (max %d chars, got %d)", MaxSearchQueryLength, len(query))
	}

	maxResults := t.maxResults
	if raw, ok := input.Params["max_results"]; ok {
		if f, ok := raw.(float64); ok {
			maxResults = int(f)
		}
	}
	if maxResults < 1 {
		maxResults = 1
	}
	if maxResults > MaxSearchResults {
		maxResults = MaxSearchResults
	}

	searchURL, err := t.buildURL(query, input.Params)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("failed to build search URL: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, searchURL, nil)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("User-Agent", fmt.Sprintf("M31A/%s (AI Coding Agent; +https://github.com/eshanized/M31A)", getVersion()))
	req.Header.Set("Accept", "application/json")

	resp, err := t.client.Do(req)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("search request failed: %w", err)
	}
	defer resp.Body.Close() //nolint:errcheck

	if resp.StatusCode < 200 || resp.StatusCode >= 400 {
		return types.ToolResult{}, fmt.Errorf("search returned HTTP %d: %s", resp.StatusCode, resp.Status)
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, types.MaxFileSize+1))
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("failed to read search response: %w", err)
	}
	if len(body) > types.MaxFileSize {
		return types.ToolResult{}, fmt.Errorf("search response exceeds 5MB limit")
	}

	var sResp searxngResponse
	if err := json.Unmarshal(body, &sResp); err != nil {
		return types.ToolResult{}, fmt.Errorf("failed to parse search response JSON: %w", err)
	}

	if len(sResp.Results) == 0 {
		return types.ToolResult{
			Output:     fmt.Sprintf("No results found for %q", query),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	results := sResp.Results
	truncated := false
	if len(results) > maxResults {
		results = results[:maxResults]
		truncated = true
	}

	var b strings.Builder
	fmt.Fprintf(&b, "Found %d results for %q:\n", len(results), query)
	for i, r := range results {
		fmt.Fprintf(&b, "\n%d. [%s](%s)\n", i+1, r.Title, r.URL)
		if r.Content != "" {
			fmt.Fprintf(&b, "   %s\n", r.Content)
		}
	}
	if truncated {
		fmt.Fprintf(&b, "\n[... more results available (limit: %d)]", maxResults)
	}

	return types.ToolResult{
		Output:     b.String(),
		DurationMs: time.Since(start).Milliseconds(),
		Truncated:  truncated,
	}, nil
}

func (t *WebSearch) buildURL(query string, params map[string]any) (string, error) {
	q := url.Values{}
	q.Set("q", query)
	q.Set("format", "json")
	q.Set("categories", "general")

	if engines, ok := params["engines"].(string); ok && engines != "" {
		q.Set("engines", engines)
	}

	return t.baseURL + "/search?" + q.Encode(), nil
}

// Close releases idle connections held by the HTTP client's transport.
// Call this during shutdown to prevent connection leaks.
func (t *WebSearch) Close() {
	t.client.CloseIdleConnections()
}
