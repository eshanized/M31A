package lsp

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"sync"
	"time"
)

// LSPClient communicates with a language server via JSON-RPC 2.0 over stdio.
type LSPClient struct {
	cmd        *exec.Cmd
	stdin      io.WriteCloser
	stdout     *bufio.Reader
	stderr     *bytes.Buffer
	mu         sync.Mutex
	nextID     int64
	workDir    string
	capabilities LSPCapabilities
	lastUsed   time.Time
	closed     bool
}

// Location represents a location in a source file (LSP standard).
type Location struct {
	URI   string `json:"uri"`
	Range Range  `json:"range"`
}

// Range represents a range in a source file (LSP standard).
type Range struct {
	Start Position `json:"start"`
	End   Position `json:"end"`
}

// Position represents a position in a source file (LSP standard, 0-indexed).
type Position struct {
	Line      int `json:"line"`
	Character int `json:"character"`
}

// CallHierarchyItem represents an item in the call hierarchy (LSP standard).
type CallHierarchyItem struct {
	Name           string     `json:"name"`
	Kind           string     `json:"kind"`
	URI            string     `json:"uri"`
	Range          Range      `json:"range"`
	SelectionRange Range      `json:"selectionRange"`
	Detail         string     `json:"detail,omitempty"`
}

// TypeHierarchyItem represents an item in the type hierarchy (LSP standard).
type TypeHierarchyItem struct {
	Name           string `json:"name"`
	Kind           string `json:"kind"`
	URI            string `json:"uri"`
	Range          Range  `json:"range"`
	SelectionRange Range  `json:"selectionRange"`
}

// HoverInfo represents hover information (LSP standard).
type HoverInfo struct {
	Contents string `json:"contents"`
}

// jsonrpcRequest represents a JSON-RPC 2.0 request.
type jsonrpcRequest struct {
	JSONRPC string      `json:"jsonrpc"`
	Method  string      `json:"method"`
	Params  interface{} `json:"params,omitempty"`
	ID      int64       `json:"id"`
}

// jsonrpcNotification represents a JSON-RPC 2.0 notification (no ID).
type jsonrpcNotification struct {
	JSONRPC string      `json:"jsonrpc"`
	Method  string      `json:"method"`
	Params  interface{} `json:"params,omitempty"`
}

// jsonrpcResponse represents a JSON-RPC 2.0 response.
type jsonrpcResponse struct {
	JSONRPC string          `json:"jsonrpc"`
	Result  json.RawMessage `json:"result,omitempty"`
	Error   *jsonrpcError   `json:"error,omitempty"`
	ID      int64           `json:"id"`
}

type jsonrpcError struct {
	Code    int         `json:"code"`
	Message string      `json:"message"`
	Data    interface{} `json:"data,omitempty"`
}

// NewLSPClient starts a language server subprocess and performs the initialize handshake.
func NewLSPClient(command []string, workDir string) (*LSPClient, error) {
	if len(command) == 0 {
		return nil, errors.New("empty command")
	}

	cmd := exec.Command(command[0], command[1:]...)
	cmd.Dir = workDir

	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, fmt.Errorf("stdin pipe: %w", err)
	}

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("stdout pipe: %w", err)
	}

	stderr := &bytes.Buffer{}
	cmd.Stderr = stderr

	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("start command %v: %w", command, err)
	}

	client := &LSPClient{
		cmd:      cmd,
		stdin:    stdin,
		stdout:   bufio.NewReader(stdout),
		stderr:   stderr,
		workDir:  workDir,
		lastUsed: time.Now(),
		nextID:   1,
	}

	// Send initialize request
	initParams := map[string]interface{}{
		"processId": os.Getpid(),
		"rootUri":   "file://" + workDir,
		"capabilities": map[string]interface{}{
			"textDocument": map[string]interface{}{
				"definition":       map[string]interface{}{},
				"references":       map[string]interface{}{},
				"callHierarchy":    map[string]interface{}{},
				"typeHierarchy":    map[string]interface{}{},
				"hover":            map[string]interface{}{},
			},
		},
	}

	resp, err := client.sendRequest("initialize", initParams)
	if err != nil {
		client.Close()
		return nil, fmt.Errorf("initialize request: %w", err)
	}

	// Parse server capabilities from initialize response
	var initResult struct {
		Capabilities json.RawMessage `json:"capabilities"`
	}
	if err := json.Unmarshal(resp, &initResult); err != nil {
		client.Close()
		return nil, fmt.Errorf("parse initialize response: %w", err)
	}
	client.capabilities = NegotiateCapabilities(initResult.Capabilities)

	// Send initialized notification
	if err := client.sendNotification("initialized", map[string]interface{}{}); err != nil {
		client.Close()
		return nil, fmt.Errorf("initialized notification: %w", err)
	}

	return client, nil
}

// sendRequest sends a JSON-RPC 2.0 request and waits for the response.
func (c *LSPClient) sendRequest(method string, params interface{}) (json.RawMessage, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.closed {
		return nil, errors.New("client closed")
	}

	id := c.nextID
	c.nextID++

	req := jsonrpcRequest{
		JSONRPC: "2.0",
		Method:  method,
		Params:  params,
		ID:      id,
	}

	data, err := json.Marshal(req)
	if err != nil {
		return nil, fmt.Errorf("marshal request: %w", err)
	}

	if _, err := c.stdin.Write(append(data, '\n')); err != nil {
		return nil, fmt.Errorf("write request: %w", err)
	}

	return c.recvResponse(id)
}

// sendNotification sends a JSON-RPC 2.0 notification (no response expected).
func (c *LSPClient) sendNotification(method string, params interface{}) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.closed {
		return errors.New("client closed")
	}

	notif := jsonrpcNotification{
		JSONRPC: "2.0",
		Method:  method,
		Params:  params,
	}

	data, err := json.Marshal(notif)
	if err != nil {
		return fmt.Errorf("marshal notification: %w", err)
	}

	if _, err := c.stdin.Write(append(data, '\n')); err != nil {
		return fmt.Errorf("write notification: %w", err)
	}

	return nil
}

// recvResponse reads responses until it finds one matching the expected ID.
func (c *LSPClient) recvResponse(expectedID int64) (json.RawMessage, error) {
	for {
		line, err := c.stdout.ReadBytes('\n')
		if err != nil {
			return nil, fmt.Errorf("read response: %w", err)
		}

		var resp jsonrpcResponse
		if err := json.Unmarshal(line, &resp); err != nil {
			// Not a valid JSON-RPC response, skip
			continue
		}

		if resp.ID == expectedID {
			if resp.Error != nil {
				return nil, fmt.Errorf("LSP error %d: %s", resp.Error.Code, resp.Error.Message)
			}
			return resp.Result, nil
		}
		// Ignore responses for other IDs (shouldn't happen in single-threaded use)
	}
}

// Touch updates the last used time.
func (c *LSPClient) Touch() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.lastUsed = time.Now()
}

// LastUsed returns the last time the client was used.
func (c *LSPClient) LastUsed() time.Time {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.lastUsed
}

// Capabilities returns the negotiated server capabilities.
func (c *LSPClient) Capabilities() LSPCapabilities {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.capabilities
}

// Definition sends a textDocument/definition request.
func (c *LSPClient) Definition(file string, line, col int) ([]Location, error) {
	params := map[string]interface{}{
		"textDocument": map[string]interface{}{
			"uri": "file://" + file,
		},
		"position": map[string]interface{}{
			"line":      line,
			"character": col,
		},
	}

	resp, err := c.sendRequest("textDocument/definition", params)
	if err != nil {
		return nil, err
	}

	// Response can be a single Location or an array of Locations
	var locations []Location
	if err := json.Unmarshal(resp, &locations); err != nil {
		// Try single location
		var single Location
		if err := json.Unmarshal(resp, &single); err != nil {
			return nil, fmt.Errorf("parse definition response: %w", err)
		}
		locations = []Location{single}
	}
	return locations, nil
}

// References sends a textDocument/references request.
func (c *LSPClient) References(file string, line, col int, includeDeclaration bool) ([]Location, error) {
	params := map[string]interface{}{
		"textDocument": map[string]interface{}{
			"uri": "file://" + file,
		},
		"position": map[string]interface{}{
			"line":      line,
			"character": col,
		},
		"context": map[string]interface{}{
			"includeDeclaration": includeDeclaration,
		},
	}

	resp, err := c.sendRequest("textDocument/references", params)
	if err != nil {
		return nil, err
	}

	var locations []Location
	if err := json.Unmarshal(resp, &locations); err != nil {
		return nil, fmt.Errorf("parse references response: %w", err)
	}
	return locations, nil
}

// CallHierarchy prepares call hierarchy and retrieves incoming calls.
func (c *LSPClient) CallHierarchy(file string, line, col int) ([]CallHierarchyItem, error) {
	// First, prepare call hierarchy
	prepareParams := map[string]interface{}{
		"textDocument": map[string]interface{}{
			"uri": "file://" + file,
		},
		"position": map[string]interface{}{
			"line":      line,
			"character": col,
		},
	}

	resp, err := c.sendRequest("textDocument/prepareCallHierarchy", prepareParams)
	if err != nil {
		return nil, err
	}

	var items []CallHierarchyItem
	if err := json.Unmarshal(resp, &items); err != nil {
		return nil, fmt.Errorf("parse prepareCallHierarchy response: %w", err)
	}

	if len(items) == 0 {
		return nil, nil
	}

	// For each item, get incoming calls
	var allItems []CallHierarchyItem
	for _, item := range items {
		incomingParams := map[string]interface{}{
			"item": item,
		}
		incomingResp, err := c.sendRequest("callHierarchy/incomingCalls", incomingParams)
		if err != nil {
			continue // Skip this item on error
		}

		var incoming []struct {
			From CallHierarchyItem `json:"from"`
		}
		if err := json.Unmarshal(incomingResp, &incoming); err != nil {
			continue
		}

		for _, inc := range incoming {
			allItems = append(allItems, inc.From)
		}
	}

	return allItems, nil
}

// TypeHierarchy prepares type hierarchy and retrieves supertypes.
func (c *LSPClient) TypeHierarchy(file string, line, col int) ([]TypeHierarchyItem, error) {
	// First, prepare type hierarchy
	prepareParams := map[string]interface{}{
		"textDocument": map[string]interface{}{
			"uri": "file://" + file,
		},
		"position": map[string]interface{}{
			"line":      line,
			"character": col,
		},
	}

	resp, err := c.sendRequest("textDocument/prepareTypeHierarchy", prepareParams)
	if err != nil {
		return nil, err
	}

	var items []TypeHierarchyItem
	if err := json.Unmarshal(resp, &items); err != nil {
		return nil, fmt.Errorf("parse prepareTypeHierarchy response: %w", err)
	}

	if len(items) == 0 {
		return nil, nil
	}

	// For each item, get supertypes
	var allItems []TypeHierarchyItem
	for _, item := range items {
		supertypesParams := map[string]interface{}{
			"item": item,
		}
		supertypesResp, err := c.sendRequest("typeHierarchy/supertypes", supertypesParams)
		if err != nil {
			continue
		}

		var supertypes []TypeHierarchyItem
		if err := json.Unmarshal(supertypesResp, &supertypes); err != nil {
			continue
		}
		allItems = append(allItems, supertypes...)
	}

	return allItems, nil
}

// Hover sends a textDocument/hover request.
func (c *LSPClient) Hover(file string, line, col int) (*HoverInfo, error) {
	params := map[string]interface{}{
		"textDocument": map[string]interface{}{
			"uri": "file://" + file,
		},
		"position": map[string]interface{}{
			"line":      line,
			"character": col,
		},
	}

	resp, err := c.sendRequest("textDocument/hover", params)
	if err != nil {
		return nil, err
	}

	if len(resp) == 0 || string(resp) == "null" {
		return nil, nil
	}

	// Hover response can have contents as string or MarkupContent
	var hover struct {
		Contents interface{} `json:"contents"`
	}
	if err := json.Unmarshal(resp, &hover); err != nil {
		return nil, fmt.Errorf("parse hover response: %w", err)
	}

	// Extract string content
	var content string
	switch v := hover.Contents.(type) {
	case string:
		content = v
	case map[string]interface{}:
		if val, ok := v["value"].(string); ok {
			content = val
		}
	case []interface{}:
		for _, item := range v {
			if s, ok := item.(string); ok {
				content += s + "\n"
			} else if m, ok := item.(map[string]interface{}); ok {
				if val, ok := m["value"].(string); ok {
					content += val + "\n"
				}
			}
		}
	}

	return &HoverInfo{Contents: content}, nil
}

// Close sends shutdown request, exit notification, and kills the process.
func (c *LSPClient) Close() error {
	c.mu.Lock()
	if c.closed {
		c.mu.Unlock()
		return nil
	}
	c.closed = true
	c.mu.Unlock()

	// Send shutdown request
	_ = c.sendNotification("shutdown", nil)

	// Send exit notification
	_ = c.sendNotification("exit", nil)

	// Close stdin to signal EOF
	if c.stdin != nil {
		_ = c.stdin.Close()
	}

	// Wait for process to exit with timeout
	done := make(chan error, 1)
	go func() {
		done <- c.cmd.Wait()
	}()

	select {
	case err := <-done:
		return err
	case <-time.After(2 * time.Second):
		// Force kill if not exited
		if c.cmd.Process != nil {
			_ = c.cmd.Process.Kill()
		}
		return <-done
	}
}

// IsClosed returns whether the client has been closed.
func (c *LSPClient) IsClosed() bool {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.closed
}

// Stderr returns captured stderr output.
func (c *LSPClient) Stderr() string {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.stderr.String()
}