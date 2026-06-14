package provider

import (
	"bufio"
	"context"
	"io"
	"log/slog"
	"net/http"
	"strings"
	"sync"
	"time"
)

type SSEParser struct {
	scanner   *bufio.Scanner
	resp      *http.Response
	closeOnce sync.Once
	ctx       context.Context
	cancel    context.CancelFunc
	watchdog  *time.Timer
}

func NewSSEParser(resp *http.Response) *SSEParser {
	return NewSSEParserWithContext(resp, context.Background())
}

func NewSSEParserWithContext(resp *http.Response, ctx context.Context) *SSEParser {
	ctx, cancel := context.WithCancel(ctx)
	scanner := bufio.NewScanner(resp.Body)
	scanner.Buffer(make([]byte, 0, sseMaxLineLength), sseMaxLineLength)

	// C-2: Start a watchdog that closes the body if no data arrives
	// within DefaultStreamTimeout. This prevents indefinite blocking.
	watchdog := time.AfterFunc(DefaultStreamTimeout, func() {
		_ = resp.Body.Close()
	})

	return &SSEParser{
		scanner:  scanner,
		resp:     resp,
		ctx:      ctx,
		cancel:   cancel,
		watchdog: watchdog,
	}
}

func (p *SSEParser) Next() (eventType string, data string, err error) {
	var lines []string

	for p.scanner.Scan() {
		// C-2: Reset watchdog on each successful read to prevent timeout
		// while data is still flowing.
		if p.watchdog != nil {
			p.watchdog.Reset(DefaultStreamTimeout)
		}

		// Check context cancellation between lines
		if p.ctx != nil {
			select {
			case <-p.ctx.Done():
				return "", "", p.ctx.Err()
			default:
			}
		}

		line := p.scanner.Text()
		// H-18 fix: trim \r from SSE lines to prevent JSON parse failures
		// on providers that send \r\n line endings.
		line = strings.TrimRight(line, "\r")

		if line == "" {
			if len(lines) > 0 {
				break
			}
			continue
		}

		lines = append(lines, line)
	}

	// Check for scanner errors immediately after the scan loop ends
	if scanErr := p.scanner.Err(); scanErr != nil {
		return "", "", scanErr
	}

	if len(lines) == 0 {
		return "", "", io.EOF
	}

	var dataParts []string
	for _, line := range lines {
		if strings.HasPrefix(line, "data: ") {
			payload := strings.TrimPrefix(line, "data: ")
			if strings.TrimSpace(payload) == "[DONE]" {
				return "", "", io.EOF
			}
			dataParts = append(dataParts, payload)
		} else if strings.HasPrefix(line, "event: ") {
			eventType = strings.TrimPrefix(line, "event: ")
		} else if strings.HasPrefix(line, "id: ") || strings.HasPrefix(line, "retry: ") {
			// SSE spec fields not used by current LLM providers; logged for diagnostics.
			slog.Debug("SSE parser ignoring field", "line", line)
		}
	}

	data = strings.Join(dataParts, "\n")
	if strings.TrimSpace(data) == "" {
		// Empty data line (keep-alive or empty event) — skip to next SSE event
		// instead of returning an error or empty string that would fail JSON parsing.
		return "", "", nil
	}
	return eventType, data, nil
}

// Close releases the underlying response body. Idempotent — safe to call
// multiple times.
func (p *SSEParser) Close() error {
	var closeErr error
	p.closeOnce.Do(func() {
		if p.watchdog != nil {
			p.watchdog.Stop()
		}
		if p.cancel != nil {
			p.cancel()
		}
		if p.resp != nil && p.resp.Body != nil {
			closeErr = p.resp.Body.Close()
		}
	})
	return closeErr
}

// DefaultStreamTimeout is the maximum time to wait for a single SSE event.
const DefaultStreamTimeout = 5 * time.Minute

// sseMaxLineLength is the maximum size of a single SSE event line (1MB).
const sseMaxLineLength = 1024 * 1024
