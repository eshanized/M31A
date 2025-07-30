package provider

import (
	"bufio"
	"fmt"
	"io"
	"net/http"
	"strings"
	"sync"
	"time"
)

type SSEParser struct {
	scanner   *bufio.Scanner
	resp      *http.Response
	closeOnce sync.Once
}

func NewSSEParser(resp *http.Response) *SSEParser {
	scanner := bufio.NewScanner(resp.Body)
	scanner.Buffer(make([]byte, 0, sseMaxLineLength), sseMaxLineLength)
	return &SSEParser{
		scanner: scanner,
		resp:    resp,
	}
}

func (p *SSEParser) Next() (eventType string, data string, err error) {
	var lines []string

	for p.scanner.Scan() {
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
		}
	}

	data = strings.Join(dataParts, "")
	if data == "" && len(dataParts) == 0 {
		return "", "", fmt.Errorf("stream truncated before completion: %w", io.ErrUnexpectedEOF)
	}
	return eventType, data, nil
}

// Close releases the underlying response body. Idempotent — safe to call
// multiple times.
func (p *SSEParser) Close() error {
	var closeErr error
	p.closeOnce.Do(func() {
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
