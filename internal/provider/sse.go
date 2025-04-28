package provider

import (
	"bufio"
	"context"
	"io"
	"net/http"
	"strings"
	"time"
)

type SSEParser struct {
	scanner *bufio.Scanner
	resp    *http.Response
}

func NewSSEParser(resp *http.Response) *SSEParser {
	scanner := bufio.NewScanner(resp.Body)
	scanner.Buffer(make([]byte, 0, 65536), 65536)
	return &SSEParser{
		scanner: scanner,
		resp:    resp,
	}
}

func (p *SSEParser) Next() (eventType string, data string, err error) {
	var lines []string

	for p.scanner.Scan() {
		line := p.scanner.Text()

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
			if payload == "[DONE]" {
				return "", "", io.EOF
			}
			dataParts = append(dataParts, payload)
		} else if strings.HasPrefix(line, "event: ") {
			eventType = strings.TrimPrefix(line, "event: ")
		}
	}

	data = strings.Join(dataParts, "")
	return eventType, data, nil
}

func (p *SSEParser) NextWithContext(ctx context.Context) (eventType string, data string, err error) {
	done := make(chan struct{})
	var result struct {
		eventType string
		data      string
		err       error
	}

	go func() {
		result.eventType, result.data, result.err = p.Next()
		close(done)
	}()

	select {
	case <-done:
		return result.eventType, result.data, result.err
	case <-ctx.Done():
		p.resp.Body.Close()
		return "", "", ctx.Err()
	}
}

func (p *SSEParser) Close() error {
	return p.resp.Body.Close()
}

// DefaultStreamTimeout is the maximum time to wait for a single SSE event.
const DefaultStreamTimeout = 5 * time.Minute
