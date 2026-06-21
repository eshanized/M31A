package tools

import (
	"context"
	"fmt"
	"io"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

var _ types.Tool = (*HTTPCheck)(nil)

type HTTPCheck struct {
	client *http.Client
}

func NewHTTPCheck() *HTTPCheck {
	return &HTTPCheck{
		client: &http.Client{
			Timeout: 15 * time.Second,
			Transport: &http.Transport{
				DialContext: (&net.Dialer{
					Timeout: 5 * time.Second,
				}).DialContext,
				TLSHandshakeTimeout: 5 * time.Second,
			},
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
- method (optional): HTTP method, defaults to "GET"
- expected_status (optional): Expected HTTP status code, defaults to 200
- expected_content (optional): List of strings that must appear in the response body
- not_expected_content (optional): List of strings that must NOT appear in the response body
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
				"enum": ["GET", "POST", "HEAD"]
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

	req, err := http.NewRequestWithContext(ctx, method, url, nil)
	if err != nil {
		return types.ToolResult{
			Error:      fmt.Sprintf("failed to create request: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	resp, err := h.client.Do(req)
	if err != nil {
		return types.ToolResult{
			Output:     fmt.Sprintf("Request to %s failed: %v", url, err),
			Error:      fmt.Sprintf("HTTP request failed: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}
	defer resp.Body.Close()

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
