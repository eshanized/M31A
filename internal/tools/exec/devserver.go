package exec

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/exec"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/shell"
	"github.com/eshanized/M31A/internal/types"
)

var _ types.Tool = (*DevServer)(nil)

const (
	maxLogLines    = 500
	maxLogBytes    = 256 * 1024 // 256KB per server
	logTrimBytes   = 64 * 1024  // trim 64KB when over limit
	crashCheckSecs = 2
)

type devServerEntry struct {
	cmd      *exec.Cmd
	command  string
	port     int
	started  time.Time
	env      []string
	logs     *ringBuffer
	exited   bool
	exitCode int
	exitedAt time.Time
	crashed  bool
	mu       sync.Mutex
}

type ringBuffer struct {
	mu       sync.Mutex
	buf      []byte
	maxBytes int
	lines    int
}

func newRingBuffer(maxBytes int) *ringBuffer {
	return &ringBuffer{maxBytes: maxBytes}
}

func (rb *ringBuffer) Write(p []byte) (int, error) {
	rb.mu.Lock()
	defer rb.mu.Unlock()
	rb.buf = append(rb.buf, p...)
	rb.lines += bytes.Count(p, []byte("\n"))
	// Trim from front if over limit
	for len(rb.buf) > rb.maxBytes && rb.lines > 10 {
		idx := bytes.IndexByte(rb.buf, '\n')
		if idx < 0 {
			break
		}
		rb.buf = rb.buf[idx+1:]
		rb.lines--
	}
	return len(p), nil
}

func (rb *ringBuffer) String() string {
	rb.mu.Lock()
	defer rb.mu.Unlock()
	return string(rb.buf)
}

func (rb *ringBuffer) TruncatedLines() int {
	rb.mu.Lock()
	defer rb.mu.Unlock()
	return rb.lines
}

type DevServer struct {
	workDir   string
	processes map[int]*devServerEntry
	mu        sync.Mutex
	nextKey   int
}

// NewDevServer creates a new DevServer tool instance.
func NewDevServer(workDir string) *DevServer {
	return &DevServer{
		workDir:   workDir,
		processes: make(map[int]*devServerEntry),
	}
}

func (d *DevServer) Name() string               { return "DevServer" }
func (d *DevServer) RiskLevel() types.RiskLevel { return types.RiskMedium }

func (d *DevServer) Description() string {
	return `Manage development servers for runtime verification.
Actions:
- "start": Start a dev server with a command (e.g., "npm run dev", "python -m http.server 8000")
- "stop": Stop a running server by its ID
- "status": List all running servers with crash detection
- "check_port": Check if a port is accepting connections
- "logs": Retrieve captured logs for a running or crashed server
- "restart": Restart a server by its ID (stop + start with same command)

Parameters:
- action (required): "start", "stop", "status", "check_port", "logs", or "restart"
- command (for start): Shell command to start the server
- port (for start/check_port): Port the server listens on
- id (for stop/logs/restart): Server ID returned by start
- env (for start/restart): Environment variables as KEY=VALUE array
- tail (for logs): Number of log lines to return from end (default 100)`
}

func (d *DevServer) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"action": {
				"type": "string",
				"enum": ["start", "stop", "status", "check_port", "logs", "restart"],
				"description": "Action to perform"
			},
			"command": {
				"type": "string",
				"description": "Shell command to start the server (for action=start or restart)"
			},
			"port": {
				"type": "integer",
				"description": "Port number the server listens on (for action=start or check_port)"
			},
			"id": {
				"type": "integer",
				"description": "Server ID (for action=stop, logs, or restart)"
			},
			"env": {
				"type": "array",
				"items": {"type": "string"},
				"description": "Environment variables as KEY=VALUE strings (for action=start or restart)"
			},
			"tail": {
				"type": "integer",
				"description": "Number of log lines to return from end (for action=logs, default 100)"
			}
		},
		"required": ["action"]
	}`
}

func (d *DevServer) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	action, _ := input.Params["action"].(string)
	switch action {
	case "start":
		return d.startServer(ctx, input, start)
	case "stop":
		return d.stopServer(input, start)
	case "status":
		return d.listServers(start)
	case "check_port":
		return d.checkPort(input, start)
	case "logs":
		return d.getLogs(input, start)
	case "restart":
		return d.restartServer(ctx, input, start)
	default:
		return types.ToolResult{
			Error:      fmt.Sprintf("unknown action %q; use start, stop, status, check_port, logs, or restart", action),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}
}

func (d *DevServer) startServer(ctx context.Context, input types.ToolInput, start time.Time) (types.ToolResult, error) {
	command, _ := input.Params["command"].(string)
	if command == "" {
		return types.ToolResult{
			Error:      "command is required for action=start",
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	port := 0
	if p, ok := input.Params["port"].(float64); ok {
		port = int(p)
	}

	// Parse environment variables
	env := d.parseEnv(input)

	cmd := shell.Command(command)
	cmd.Dir = d.workDir

	// Set environment variables
	cmd.Env = mergeEnv(os.Environ(), env)

	// Capture logs via ring buffer
	logs := newRingBuffer(maxLogBytes)
	cmd.Stdout = logs
	cmd.Stderr = logs

	if err := cmd.Start(); err != nil {
		return types.ToolResult{
			Error:      fmt.Sprintf("failed to start server: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	d.mu.Lock()
	d.nextKey++
	id := d.nextKey
	entry := &devServerEntry{
		cmd:     cmd,
		command: command,
		port:    port,
		started: time.Now(),
		env:     env,
		logs:    logs,
	}
	d.processes[id] = entry
	d.mu.Unlock()

	// Monitor for crashes in background
	go d.monitorCrash(id, entry)

	if port > 0 {
		ready := waitForPort(port, 30*time.Second)
		if !ready {
			_ = d.stopByID(id)
			return types.ToolResult{
				Output:     fmt.Sprintf("Server started (id=%d) but port %d not ready after 30s\nLogs:\n%s", id, port, d.getTruncatedLogs(entry, 50)),
				Error:      fmt.Sprintf("port %d not accepting connections after 30s timeout", port),
				DurationMs: time.Since(start).Milliseconds(),
			}, nil
		}
	}

	return types.ToolResult{
		Output:     fmt.Sprintf("Server started: id=%d, command=%q, port=%d, pid=%d", id, command, port, cmd.Process.Pid),
		DurationMs: time.Since(start).Milliseconds(),
	}, nil
}

func (d *DevServer) restartServer(ctx context.Context, input types.ToolInput, start time.Time) (types.ToolResult, error) {
	id := 0
	if v, ok := input.Params["id"].(float64); ok {
		id = int(v)
	}
	if id == 0 {
		return types.ToolResult{
			Error:      "id is required for action=restart",
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	d.mu.Lock()
	entry, ok := d.processes[id]
	if !ok {
		d.mu.Unlock()
		return types.ToolResult{
			Error:      fmt.Sprintf("server %d not found", id),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}
	command := entry.command
	port := entry.port
	d.mu.Unlock()

	// Stop the existing server
	if err := d.stopByID(id); err != nil {
		return types.ToolResult{
			Error:      fmt.Sprintf("failed to stop server %d: %v", id, err),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	// Parse new env vars if provided, else reuse old ones
	env := d.parseEnv(input)
	if len(env) == 0 {
		d.mu.Lock()
		if e, ok := d.processes[id]; !ok {
			// entry was deleted, use stored env
			_ = e
		}
		// Use the env from the original entry
		for _, e := range d.processes {
			if e.command == command {
				env = e.env
				break
			}
		}
		d.mu.Unlock()
	}

	// Build new start params
	newInput := types.ToolInput{
		Params: map[string]any{
			"command": command,
			"port":    float64(port),
		},
	}
	if len(env) > 0 {
		envAny := make([]any, len(env))
		for i, e := range env {
			envAny[i] = e
		}
		newInput.Params["env"] = envAny
	}

	return d.startServer(ctx, newInput, start)
}

func (d *DevServer) stopServer(input types.ToolInput, start time.Time) (types.ToolResult, error) {
	id := 0
	if v, ok := input.Params["id"].(float64); ok {
		id = int(v)
	}
	if id == 0 {
		return types.ToolResult{
			Error:      "id is required for action=stop",
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	if err := d.stopByID(id); err != nil {
		return types.ToolResult{
			Error:      err.Error(),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	return types.ToolResult{
		Output:     fmt.Sprintf("Server %d stopped", id),
		DurationMs: time.Since(start).Milliseconds(),
	}, nil
}

func (d *DevServer) stopByID(id int) error {
	d.mu.Lock()
	entry, ok := d.processes[id]
	if !ok {
		d.mu.Unlock()
		return fmt.Errorf("server %d not found", id)
	}
	delete(d.processes, id)
	d.mu.Unlock()

	if entry.cmd.Process != nil {
		pgid, err := getProcessGroup(entry.cmd.Process.Pid)
		if err == nil && pgid > 0 {
			_ = killProcessGroup(pgid)
		} else {
			_ = entry.cmd.Process.Kill()
		}
		_ = entry.cmd.Wait()
	}
	return nil
}

func (d *DevServer) listServers(start time.Time) (types.ToolResult, error) {
	d.mu.Lock()
	defer d.mu.Unlock()

	type serverInfo struct {
		ID       int    `json:"id"`
		Command  string `json:"command"`
		Port     int    `json:"port"`
		PID      int    `json:"pid"`
		Uptime   string `json:"uptime"`
		Running  bool   `json:"running"`
		Crashed  bool   `json:"crashed,omitempty"`
		ExitCode int    `json:"exit_code,omitempty"`
		LogLines int    `json:"log_lines,omitempty"`
	}

	var servers []serverInfo
	for id, entry := range d.processes {
		entry.mu.Lock()
		pid := 0
		if entry.cmd.Process != nil {
			pid = entry.cmd.Process.Pid
		}
		running := !entry.exited
		crashed := entry.crashed
		exitCode := entry.exitCode
		entry.mu.Unlock()

		servers = append(servers, serverInfo{
			ID:       id,
			Command:  entry.command,
			Port:     entry.port,
			PID:      pid,
			Uptime:   time.Since(entry.started).Round(time.Second).String(),
			Running:  running,
			Crashed:  crashed,
			ExitCode: exitCode,
			LogLines: entry.logs.TruncatedLines(),
		})
	}

	data, _ := json.MarshalIndent(servers, "", "  ")
	return types.ToolResult{
		Output:     string(data),
		DurationMs: time.Since(start).Milliseconds(),
	}, nil
}

func (d *DevServer) getLogs(input types.ToolInput, start time.Time) (types.ToolResult, error) {
	id := 0
	if v, ok := input.Params["id"].(float64); ok {
		id = int(v)
	}
	if id == 0 {
		return types.ToolResult{
			Error:      "id is required for action=logs",
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	d.mu.Lock()
	entry, ok := d.processes[id]
	d.mu.Unlock()

	if !ok {
		return types.ToolResult{
			Error:      fmt.Sprintf("server %d not found", id),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	tail := 100
	if t, ok := input.Params["tail"].(float64); ok && t > 0 {
		tail = int(t)
	}

	logs := d.getTruncatedLogs(entry, tail)

	status := "running"
	entry.mu.Lock()
	if entry.exited {
		if entry.crashed {
			status = fmt.Sprintf("crashed (exit code %d)", entry.exitCode)
		} else {
			status = fmt.Sprintf("stopped (exit code %d)", entry.exitCode)
		}
	}
	entry.mu.Unlock()

	return types.ToolResult{
		Output:     fmt.Sprintf("Server %d [%s] — last %d log lines:\n\n%s", id, status, tail, logs),
		DurationMs: time.Since(start).Milliseconds(),
	}, nil
}

func (d *DevServer) checkPort(input types.ToolInput, start time.Time) (types.ToolResult, error) {
	port := 0
	if p, ok := input.Params["port"].(float64); ok {
		port = int(p)
	}
	if port == 0 {
		return types.ToolResult{
			Error:      "port is required for action=check_port",
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	addr := fmt.Sprintf("localhost:%d", port)
	conn, err := net.DialTimeout("tcp", addr, 5*time.Second)
	if err != nil {
		return types.ToolResult{
			Output:     fmt.Sprintf("Port %d is NOT accepting connections: %v", port, err),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}
	_ = conn.Close()

	return types.ToolResult{
		Output:     fmt.Sprintf("Port %d is accepting connections", port),
		DurationMs: time.Since(start).Milliseconds(),
	}, nil
}

func (d *DevServer) parseEnv(input types.ToolInput) []string {
	var env []string
	if envRaw, ok := input.Params["env"].([]any); ok {
		for _, v := range envRaw {
			if s, ok := v.(string); ok && s != "" {
				env = append(env, s)
			}
		}
	}
	return env
}

func (d *DevServer) monitorCrash(id int, entry *devServerEntry) {
	err := entry.cmd.Wait()
	entry.mu.Lock()
	entry.exited = true
	entry.exitedAt = time.Now()
	if err != nil {
		entry.crashed = true
		if exitErr, ok := err.(*exec.ExitError); ok {
			entry.exitCode = exitErr.ExitCode()
		} else {
			entry.exitCode = -1
		}
	} else {
		entry.exitCode = 0
	}
	entry.mu.Unlock()
}

func (d *DevServer) getTruncatedLogs(entry *devServerEntry, tailLines int) string {
	full := entry.logs.String()
	lines := strings.Split(full, "\n")
	if len(lines) <= tailLines {
		return full
	}
	return strings.Join(lines[len(lines)-tailLines:], "\n")
}

// StopAll terminates all managed dev servers. Called during cleanup.
func (d *DevServer) StopAll() {
	d.mu.Lock()
	ids := make([]int, 0, len(d.processes))
	for id := range d.processes {
		ids = append(ids, id)
	}
	d.mu.Unlock()

	for _, id := range ids {
		_ = d.stopByID(id)
	}
}

// mergeEnv merges custom environment variables with the base environment.
// Custom vars override base vars with the same key.
func mergeEnv(base, custom []string) []string {
	if len(custom) == 0 {
		return base
	}
	// Build lookup of custom env
	customMap := make(map[string]string, len(custom))
	for _, e := range custom {
		if idx := strings.IndexByte(e, '='); idx >= 0 {
			customMap[e[:idx]] = e[idx+1:]
		}
	}
	// Start with base, skip keys that are overridden
	var result []string
	for _, e := range base {
		if idx := strings.IndexByte(e, '='); idx >= 0 {
			if _, overridden := customMap[e[:idx]]; overridden {
				result = append(result, e[:idx]+"="+customMap[e[:idx]])
				delete(customMap, e[:idx])
				continue
			}
		}
		result = append(result, e)
	}
	// Add remaining custom vars
	for k, v := range customMap {
		result = append(result, k+"="+v)
	}
	return result
}

// waitForPort polls a TCP port until it accepts connections or timeout expires.
func waitForPort(port int, timeout time.Duration) bool {
	deadline := time.Now().Add(timeout)
	addr := fmt.Sprintf("localhost:%d", port)
	for time.Now().Before(deadline) {
		conn, err := net.DialTimeout("tcp", addr, 2*time.Second)
		if err == nil {
			_ = conn.Close()
			return true
		}
		time.Sleep(500 * time.Millisecond)
	}
	return false
}