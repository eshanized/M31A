package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

type Dispatcher struct {
	mu          sync.RWMutex
	tools       map[string]types.Tool
	permissions map[string]bool
	requestCh   chan PermissionRequest
	responseCh  chan PermissionResponse
}

func NewDispatcher() *Dispatcher {
	return &Dispatcher{
		tools:       make(map[string]types.Tool),
		permissions: make(map[string]bool),
		requestCh:   make(chan PermissionRequest, 1),
		responseCh:  make(chan PermissionResponse),
	}
}

func (d *Dispatcher) Register(tool types.Tool) {
	d.mu.Lock()
	defer d.mu.Unlock()
	name := tool.Name()
	if _, ok := d.tools[name]; ok {
		panic(fmt.Sprintf("tool already registered: %s", name))
	}
	d.tools[name] = tool
}

func (d *Dispatcher) Execute(ctx context.Context, call types.ToolCall) (types.ToolResult, error) {
	start := time.Now()

	d.mu.RLock()
	tool, ok := d.tools[call.Name]
	d.mu.RUnlock()

	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: unknown tool: %s", m31errors.ErrToolExecution, call.Name)
	}

	var input types.ToolInput
	if err := json.Unmarshal(call.Input, &input); err != nil {
		input = types.ToolInput{Name: call.Name, Params: map[string]any{}}
	}
	input.Name = call.Name

	risk := tool.RiskLevel()
	if risk == types.RiskDangerous || risk == types.RiskDestructive {
		d.mu.RLock()
		allowed, remembered := d.permissions[call.Name]
		d.mu.RUnlock()

		if !remembered || !allowed {
			req := PermissionRequest{
				ToolName:    call.Name,
				Command:     extractCommandString(call.Name, call.Input),
				RiskLevel:   risk,
				TimeoutSecs: 300,
			}

			select {
			case d.requestCh <- req:
			default:
				return types.ToolResult{}, m31errors.ErrPermissionDenied
			}

			var resp PermissionResponse
			select {
			case resp = <-d.responseCh:
			case <-ctx.Done():
				return types.ToolResult{}, ctx.Err()
			}

			if !resp.Allowed {
				return types.ToolResult{}, m31errors.ErrPermissionDenied
			}

			if resp.Remember {
				d.mu.Lock()
				d.permissions[call.Name] = resp.Allowed
				d.mu.Unlock()
			}
		}
	}

	result, err := tool.Execute(ctx, input)
	elapsed := time.Since(start).Milliseconds()

	res := types.ToolResult{
		ToolCallID: call.ID,
		Output:     result.Output,
		DurationMs: elapsed,
		Truncated:  result.Truncated,
	}
	if err != nil {
		res.Error = err.Error()
	}

	return res, nil
}

func (d *Dispatcher) ApprovePermission(allowed bool, remember bool) {
	d.responseCh <- PermissionResponse{Allowed: allowed, Remember: remember}
}

func (d *Dispatcher) List() []string {
	d.mu.RLock()
	defer d.mu.RUnlock()
	names := make([]string, 0, len(d.tools))
	for name := range d.tools {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func (d *Dispatcher) GetTool(name string) (types.Tool, bool) {
	d.mu.RLock()
	defer d.mu.RUnlock()
	t, ok := d.tools[name]
	return t, ok
}

// extractCommandString extracts a human-readable command string from tool input
// for display in the permission prompt.
func extractCommandString(toolName string, input json.RawMessage) string {
	if len(input) == 0 {
		return ""
	}

	// Normalize tool name to canonical form (first char uppercase)
	if len(toolName) > 0 {
		toolName = strings.ToUpper(toolName[:1]) + toolName[1:]
	}

	// Try unmarshaling as ToolInput first (has Name and Params)
	var toolInput struct {
		Name   string         `json:"name"`
		Params map[string]any `json:"params"`
	}
	if err := json.Unmarshal(input, &toolInput); err == nil && len(toolInput.Params) > 0 {
		switch toolName {
		case "Bash":
			if cmd, ok := toolInput.Params["command"].(string); ok {
				return cmd
			}
		case "FileRead":
			if path, ok := toolInput.Params["path"].(string); ok {
				return fmt.Sprintf("read %s", path)
			}
		case "FileWrite":
			if path, ok := toolInput.Params["path"].(string); ok {
				return fmt.Sprintf("write %s", path)
			}
		case "Glob":
			if pattern, ok := toolInput.Params["pattern"].(string); ok {
				return fmt.Sprintf("glob %s", pattern)
			}
		case "Grep":
			if pattern, ok := toolInput.Params["pattern"].(string); ok {
				return fmt.Sprintf("grep %s", pattern)
			}
		}
	}

	// Fallback: try as raw map
	var params map[string]any
	if err := json.Unmarshal(input, &params); err == nil {
		switch toolName {
		case "Bash":
			if cmd, ok := params["command"].(string); ok {
				return cmd
			}
		case "FileRead":
			if path, ok := params["path"].(string); ok {
				return fmt.Sprintf("read %s", path)
			}
		case "FileWrite":
			if path, ok := params["path"].(string); ok {
				return fmt.Sprintf("write %s", path)
			}
		case "Glob":
			if pattern, ok := params["pattern"].(string); ok {
				return fmt.Sprintf("glob %s", pattern)
			}
		case "Grep":
			if pattern, ok := params["pattern"].(string); ok {
				return fmt.Sprintf("grep %s", pattern)
			}
		}
	}

	return string(input)
}

func (d *Dispatcher) RequestCh() chan PermissionRequest {
	return d.requestCh
}

func DefaultDispatcher(workDir, backupDir string) *Dispatcher {
	d := NewDispatcher()
	d.Register(NewBash(workDir))
	d.Register(NewFileRead(workDir))
	d.Register(NewFileWrite(workDir, backupDir))
	d.Register(NewGlob(workDir))
	d.Register(NewGrep(workDir))
	return d
}
