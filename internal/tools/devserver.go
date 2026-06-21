package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/exec"
	"sync"
	"syscall"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

var _ types.Tool = (*DevServer)(nil)

type devServerEntry struct {
	cmd     *exec.Cmd
	command string
	port    int
	started time.Time
}

type DevServer struct {
	workDir   string
	processes map[int]*devServerEntry
	mu        sync.Mutex
	nextKey   int
}

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
- "status": List all running servers
- "check_port": Check if a port is accepting connections

Parameters:
- action (required): "start", "stop", "status", or "check_port"
- command (for start): Shell command to start the server
- port (for start/check_port): Port the server listens on
- id (for stop): Server ID returned by start`
}

func (d *DevServer) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"action": {
				"type": "string",
				"enum": ["start", "stop", "status", "check_port"],
				"description": "Action to perform"
			},
			"command": {
				"type": "string",
				"description": "Shell command to start the server (for action=start)"
			},
			"port": {
				"type": "integer",
				"description": "Port number the server listens on (for action=start or check_port)"
			},
			"id": {
				"type": "integer",
				"description": "Server ID to stop (for action=stop)"
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
	default:
		return types.ToolResult{
			Error:      fmt.Sprintf("unknown action %q; use start, stop, status, or check_port", action),
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

	cmd := exec.Command("sh", "-c", command)
	cmd.Dir = d.workDir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Start(); err != nil {
		return types.ToolResult{
			Error:      fmt.Sprintf("failed to start server: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	d.mu.Lock()
	d.nextKey++
	id := d.nextKey
	d.processes[id] = &devServerEntry{
		cmd:     cmd,
		command: command,
		port:    port,
		started: time.Now(),
	}
	d.mu.Unlock()

	if port > 0 {
		ready := waitForPort(port, 30*time.Second)
		if !ready {
			_ = d.stopByID(id)
			return types.ToolResult{
				Output:     fmt.Sprintf("Server started (id=%d) but port %d not ready after 30s", id, port),
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
		pgid, err := syscall.Getpgid(entry.cmd.Process.Pid)
		if err == nil {
			_ = syscall.Kill(-pgid, syscall.SIGTERM)
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
		ID      int    `json:"id"`
		Command string `json:"command"`
		Port    int    `json:"port"`
		PID     int    `json:"pid"`
		Uptime  string `json:"uptime"`
	}

	var servers []serverInfo
	for id, entry := range d.processes {
		pid := 0
		if entry.cmd.Process != nil {
			pid = entry.cmd.Process.Pid
		}
		servers = append(servers, serverInfo{
			ID:      id,
			Command: entry.command,
			Port:    entry.port,
			PID:     pid,
			Uptime:  time.Since(entry.started).Round(time.Second).String(),
		})
	}

	data, _ := json.MarshalIndent(servers, "", "  ")
	return types.ToolResult{
		Output:     string(data),
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
