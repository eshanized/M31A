package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

var _ types.Tool = (*HTTPCheck)(nil)

type HTTPCheck struct {
	client *http.Client
}

// newSSRFProtectedTransport creates an HTTP transport with SSRF protection.
// It resolves DNS, checks for private/reserved IPs, and pins the first IP
// for the connection to prevent DNS rebinding attacks.
func newSSRFProtectedTransport() *http.Transport {
	return &http.Transport{
		DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
			host, port, err := net.SplitHostPort(addr)
			if err != nil {
				return nil, fmt.Errorf("failed to parse address %s: %w", addr, err)
			}

			// Resolve DNS
			resolver := &net.Resolver{PreferGo: true}
			ips, err := resolver.LookupIPAddr(ctx, host)
			if err != nil {
				return nil, fmt.Errorf("DNS resolution failed for %s: %w", host, err)
			}

			if len(ips) == 0 {
				return nil, fmt.Errorf("no IP addresses resolved for %s", host)
			}

			// Check for private/reserved IPs
			for _, ip := range ips {
				if isPrivateIP(ip.IP) || isReservedIP(ip.IP) {
					return nil, fmt.Errorf("access to private/reserved IP %s is blocked: %w", ip.IP, errors.ErrPrivateIPBlocked)
				}
			}

			// Pin the first IP for connection
			pinnedAddr := net.JoinHostPort(ips[0].IP.String(), port)
			dialer := &net.Dialer{Timeout: 10 * time.Second}
			return dialer.DialContext(ctx, network, pinnedAddr)
		},
		TLSHandshakeTimeout: 5 * time.Second,
	}
}

// NewHTTPCheck creates a new HTTPCheck tool instance with SSRF protection.
func NewHTTPCheck() *HTTPCheck {
	return &HTTPCheck{
		client: &http.Client{
			Timeout:   15 * time.Second,
			Transport: newSSRFProtectedTransport(),
			CheckRedirect: func(req *http.Request, via []*http.Request) error {
				if len(via) >= 10 {
					return fmt.Errorf("too many redirects")
				}
				return nil
			},
		},
	}
}

func (h *HTTPCheck) Name() string               { return "HTTPCheck" }
func (h *HTTPCheck) RiskLevel() types.RiskLevel { return types.RiskSafe }

func (h *HTTPCheck) Description() string {
	return `Make HTTP requests to URLs and validate responses.
Use this to verify that web pages are serving correct content after the dev server starts.

Parameters:
- url (required): The URL to check (e.g., "http://localhost:3000")
- method (optional): HTTP method (GET, POST, PUT, PATCH, DELETE, HEAD), defaults to "GET"
- headers (optional): Request headers as KEY=VALUE array
- body (optional): Request body string (for POST/PUT/PATCH)
- expected_status (optional): Expected HTTP status code, defaults to 200
- expected_content (optional): List of strings that must appear in the response body
- not_expected_content (optional): List of strings that must NOT appear in the response body
- json_path (optional): JSONPath expression to validate in response (e.g., "$.data.id")
- json_value (optional): Expected value at json_path (used with json_path)
- max_body_bytes (optional): Max response body to read, defaults to 1MB`
}

func (h *HTTPCheck) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"url": {
				"type": "string",
				"description": "URL to check"
			},
			"method": {
				"type": "string",
				"description": "HTTP method (default: GET)",
				"enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"]
			},
			"headers": {
				"type": "array",
				"items": {"type": "string"},
				"description": "Request headers as KEY=VALUE strings"
			},
			"body": {
				"type": "string",
				"description": "Request body (for POST, PUT, PATCH)"
			},
			"expected_status": {
				"type": "integer",
				"description": "Expected HTTP status code (default: 200)"
			},
			"expected_content": {
				"type": "array",
				"items": {"type": "string"},
				"description": "Strings that must appear in the response body"
			},
			"not_expected_content": {
				"type": "array",
				"items": {"type": "string"},
				"description": "Strings that must NOT appear in the response body"
			},
			"json_path": {
				"type": "string",
				"description": "JSONPath-like expression to validate (e.g., '$.data.id', '$.results[0].name')"
			},
			"json_value": {
				"type": "string",
				"description": "Expected value at json_path (string comparison)"
			},
			"max_body_bytes": {
				"type": "integer",
				"description": "Max response body bytes to read (default: 1048576)"
			}
		},
		"required": ["url"]
	}`
}

func (h *HTTPCheck) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	url, _ := input.Params["url"].(string)
	if url == "" {
		return types.ToolResult{
			Error:      "url is required",
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	method := "GET"
	if m, ok := input.Params["method"].(string); ok && m != "" {
		method = strings.ToUpper(m)
	}

	expectedStatus := 200
	if s, ok := input.Params["expected_status"].(float64); ok {
		expectedStatus = int(s)
	}

	maxBody := int64(1048576)
	if m, ok := input.Params["max_body_bytes"].(float64); ok && m > 0 {
		maxBody = int64(m)
	}

	// Build request body
	var bodyReader io.Reader
	if body, ok := input.Params["body"].(string); ok && body != "" {
		bodyReader = strings.NewReader(body)
	}

	req, err := http.NewRequestWithContext(ctx, method, url, bodyReader)
	if err != nil {
		return types.ToolResult{
			Error:      fmt.Sprintf("failed to create request: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	// Set headers
	if headers, ok := input.Params["headers"].([]any); ok {
		for _, h := range headers {
			if s, ok := h.(string); ok {
				if idx := strings.IndexByte(s, '='); idx > 0 {
					req.Header.Set(s[:idx], s[idx+1:])
				}
			}
		}
	}

	// Set Content-Type for body if not already set
	if bodyReader != nil && req.Header.Get("Content-Type") == "" {
		if _, ok := input.Params["body"].(string); ok {
			req.Header.Set("Content-Type", "application/json")
		}
	}

	resp, err := h.client.Do(req)
	if err != nil {
		return types.ToolResult{
			Output:     fmt.Sprintf("Request to %s failed: %v", url, err),
			Error:      fmt.Sprintf("HTTP request failed: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}
	defer func() { _ = resp.Body.Close() }()

	body, err := io.ReadAll(io.LimitReader(resp.Body, maxBody))
	if err != nil {
		return types.ToolResult{
			Error:      fmt.Sprintf("failed to read response body: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	var issues []string
	bodyStr := string(body)

	if resp.StatusCode != expectedStatus {
		issues = append(issues, fmt.Sprintf("status %d (expected %d)", resp.StatusCode, expectedStatus))
	}

	if expected, ok := input.Params["expected_content"].([]any); ok {
		for _, e := range expected {
			s, _ := e.(string)
			if s != "" && !strings.Contains(bodyStr, s) {
				issues = append(issues, fmt.Sprintf("missing expected content: %q", s))
			}
		}
	}

	if notExpected, ok := input.Params["not_expected_content"].([]any); ok {
		for _, ne := range notExpected {
			s, _ := ne.(string)
			if s != "" && strings.Contains(bodyStr, s) {
				issues = append(issues, fmt.Sprintf("found unexpected content: %q", s))
			}
		}
	}

	// JSON path validation
	if jsonPath, ok := input.Params["json_path"].(string); ok && jsonPath != "" {
		jsonValue, _ := input.Params["json_value"].(string)
		if err := validateJSONPath(bodyStr, jsonPath, jsonValue); err != nil {
			issues = append(issues, fmt.Sprintf("json_path validation failed: %v", err))
		}
	}

	duration := time.Since(start).Milliseconds()

	if len(issues) > 0 {
		return types.ToolResult{
			Output: fmt.Sprintf("FAIL %s %s → %d (%d bytes)\nIssues:\n  - %s",
				method, url, resp.StatusCode, len(body), strings.Join(issues, "\n  - ")),
			Error:      fmt.Sprintf("HTTP check failed: %s", strings.Join(issues, "; ")),
			DurationMs: duration,
		}, nil
	}

	return types.ToolResult{
		Output: fmt.Sprintf("PASS %s %s → %d (%d bytes, %dms)",
			method, url, resp.StatusCode, len(body), duration),
		DurationMs: duration,
	}, nil
}

// validateJSONPath validates a simple JSONPath-like expression against JSON content.
// Supports: $.key, $.key.nested, $.array[0], $.array[0].key
func validateJSONPath(bodyStr, path, expectedValue string) error {
	var data any
	if err := json.Unmarshal([]byte(bodyStr), &data); err != nil {
		return fmt.Errorf("response is not valid JSON: %w", err)
	}

	// Parse simple JSONPath: split by . and handle array indices
	parts := strings.Split(strings.TrimPrefix(path, "$."), ".")
	current := data

	for _, part := range parts {
		// Handle array index: key[0]
		if idxStart := strings.IndexByte(part, '['); idxStart >= 0 {
			key := part[:idxStart]
			idxEnd := strings.IndexByte(part, ']')
			if idxEnd < 0 {
				return fmt.Errorf("invalid JSONPath syntax: %s", part)
			}
			idxStr := part[idxStart+1 : idxEnd]
			idx := 0
			if _, err := fmt.Sscanf(idxStr, "%d", &idx); err != nil {
				return fmt.Errorf("invalid array index: %s", idxStr)
			}

			// Navigate to the key first if non-empty
			if key != "" {
				obj, ok := current.(map[string]any)
				if !ok {
					return fmt.Errorf("expected object at key %q", key)
				}
				current = obj[key]
			}

			arr, ok := current.([]any)
			if !ok {
				return fmt.Errorf("expected array, got %T", current)
			}
			if idx < 0 || idx >= len(arr) {
				return fmt.Errorf("array index %d out of bounds (length %d)", idx, len(arr))
			}
			current = arr[idx]
		} else {
			obj, ok := current.(map[string]any)
			if !ok {
				return fmt.Errorf("expected object at key %q, got %T", part, current)
			}
			current = obj[part]
		}
	}

	// Compare with expected value
	actualValue := fmt.Sprintf("%v", current)
	if expectedValue != "" && actualValue != expectedValue {
		return fmt.Errorf("expected %q, got %q", expectedValue, actualValue)
	}

	return nil
}
