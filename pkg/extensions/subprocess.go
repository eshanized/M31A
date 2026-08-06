package extensions

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"os/exec"
	"sync"
	"sync/atomic"
	"time"
)

// SubprocessManager manages the lifecycle of an extension subprocess.
// It communicates via JSON-RPC 2.0 over stdin/stdout.
type SubprocessManager struct {
	// Configuration
	cmd     string
	args    []string
	env     map[string]string
	timeout time.Duration
	workDir string

	// Process management
	process *exec.Cmd
	stdin   *os.File
	stdout  *os.File
	stderr  *os.File

	// Communication
	requestID  atomic.Int64
	pendingReq sync.Map // map[int64]chan JSONRPCResponse
	readerWG   sync.WaitGroup
	stopOnce   sync.Once
	stopped    atomic.Bool
	started    atomic.Bool
	mu         sync.Mutex

	// Handshake
	protocolVersion  string
	supportedMethods []string
}

// NewSubprocessManager creates a new SubprocessManager.
func NewSubprocessManager(cmd string, args []string, env map[string]string, timeout time.Duration) *SubprocessManager {
	return &SubprocessManager{
		cmd:     cmd,
		args:    args,
		env:     env,
		timeout: timeout,
		workDir: ".", // default to current directory
	}
}

// SetWorkDir sets the working directory for the subprocess.
func (m *SubprocessManager) SetWorkDir(dir string) {
	m.workDir = dir
}

// Start spawns the subprocess, opens pipes, starts reader goroutines, and performs handshake.
func (m *SubprocessManager) Start(ctx context.Context) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.started.Load() {
		return errors.New("subprocess already started")
	}

	// Create the command
	m.process = exec.CommandContext(ctx, m.cmd, m.args...)
	m.process.Dir = m.workDir

	// Set up environment
	m.process.Env = m.buildEnv()

	// Open pipes
	stdinPipe, err := m.process.StdinPipe()
	if err != nil {
		return fmt.Errorf("create stdin pipe: %w", err)
	}
	stdoutPipe, err := m.process.StdoutPipe()
	if err != nil {
		_ = stdinPipe.Close()
		return fmt.Errorf("create stdout pipe: %w", err)
	}
	stderrPipe, err := m.process.StderrPipe()
	if err != nil {
		_ = stdinPipe.Close()
		_ = stdoutPipe.Close()
		return fmt.Errorf("create stderr pipe: %w", err)
	}

	m.stdin = stdinPipe.(*os.File)
	m.stdout = stdoutPipe.(*os.File)
	m.stderr = stderrPipe.(*os.File)

	// Start the process
	if err := m.process.Start(); err != nil {
		_ = m.cleanupPipes()
		return fmt.Errorf("start process: %w", err)
	}

	m.started.Store(true)

	// Start reader goroutines
	m.readerWG.Add(2)
	go m.readStdout()
	go m.readStderr()

	// Perform handshake
	if err := m.performHandshake(ctx); err != nil {
		_ = m.Stop()
		return fmt.Errorf("handshake failed: %w", err)
	}

	slog.Debug("extension subprocess started", "cmd", m.cmd, "pid", m.process.Process.Pid)
	return nil
}

// buildEnv constructs the environment for the subprocess.
func (m *SubprocessManager) buildEnv() []string {
	// Start with a clean environment
	env := []string{
		"PATH=" + os.Getenv("PATH"),
		"HOME=" + os.Getenv("HOME"),
		"USER=" + os.Getenv("USER"),
		"LANG=C.UTF-8",
		"CI=true",
		"DEBIAN_FRONTEND=noninteractive",
	}

	// Add configured environment variables
	for k, v := range m.env {
		env = append(env, k+"="+v)
	}

	return env
}

// performHandshake sends the handshake request and verifies protocol version.
func (m *SubprocessManager) performHandshake(ctx context.Context) error {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      m.nextRequestID(),
		Method:  MethodHandshake,
	}

	resp, err := m.Call(ctx, req, 10*time.Second)
	if err != nil {
		return fmt.Errorf("handshake request failed: %w", err)
	}

	if resp.Error != nil {
		return fmt.Errorf("handshake error: %s", resp.Error.Message)
	}

	var result HandshakeResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return fmt.Errorf("parse handshake result: %w", err)
	}

	// Verify protocol version compatibility
	if result.ProtocolVersion != "1.0" {
		return fmt.Errorf("unsupported protocol version: %s (expected 1.0)", result.ProtocolVersion)
	}

	m.protocolVersion = result.ProtocolVersion
	m.supportedMethods = result.SupportedMethods

	slog.Debug("extension handshake successful", "protocol_version", m.protocolVersion, "methods", m.supportedMethods)
	return nil
}

// nextRequestID generates a unique request ID.
func (m *SubprocessManager) nextRequestID() json.RawMessage {
	id := m.requestID.Add(1)
	return json.RawMessage(fmt.Sprintf("%d", id))
}

// Call sends a JSON-RPC request and waits for the response.
func (m *SubprocessManager) Call(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error) {
	if !m.started.Load() || m.stopped.Load() {
		return JSONRPCResponse{}, errors.New("subprocess not running")
	}

	// Create response channel
	respCh := make(chan JSONRPCResponse, 1)
	idStr := string(req.ID)
	var id int64
	fmt.Sscanf(idStr, "%d", &id)
	m.pendingReq.Store(id, respCh)

	// Send request
	data, err := json.Marshal(req)
	if err != nil {
		m.pendingReq.Delete(id)
		return JSONRPCResponse{}, fmt.Errorf("marshal request: %w", err)
	}

	data = append(data, '\n')
	if _, err := m.stdin.Write(data); err != nil {
		m.pendingReq.Delete(id)
		return JSONRPCResponse{}, fmt.Errorf("write request: %w", err)
	}

	// Wait for response with timeout
	select {
	case resp := <-respCh:
		m.pendingReq.Delete(id)
		return resp, nil
	case <-ctx.Done():
		m.pendingReq.Delete(id)
		return JSONRPCResponse{}, ctx.Err()
	case <-time.After(timeout):
		m.pendingReq.Delete(id)
		return JSONRPCResponse{}, fmt.Errorf("request timeout after %v", timeout)
	}
}

// readStdout reads JSON-RPC responses from stdout.
func (m *SubprocessManager) readStdout() {
	defer m.readerWG.Done()
	scanner := bufio.NewScanner(m.stdout)

	for scanner.Scan() {
		if m.stopped.Load() {
			break
		}

		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}

		var resp JSONRPCResponse
		if err := json.Unmarshal(line, &resp); err != nil {
			slog.Warn("failed to unmarshal JSON-RPC response", "error", err, "line", string(line))
			continue
		}

		// Route response to waiting caller
		idStr := string(resp.ID)
		var id int64
		fmt.Sscanf(idStr, "%d", &id)

		if ch, ok := m.pendingReq.Load(id); ok {
			if respCh, ok := ch.(chan JSONRPCResponse); ok {
				select {
				case respCh <- resp:
				default:
					slog.Warn("response channel full, dropping response", "id", id)
				}
			}
		} else {
			// Notification (no ID) or late response
			slog.Debug("received notification or unmatched response", "method", resp.Result)
		}
	}

	if err := scanner.Err(); err != nil && !m.stopped.Load() {
		slog.Error("stdout scanner error", "error", err)
	}
}

// readStderr reads and logs stderr from the subprocess.
func (m *SubprocessManager) readStderr() {
	defer m.readerWG.Done()
	scanner := bufio.NewScanner(m.stderr)

	for scanner.Scan() {
		if m.stopped.Load() {
			break
		}
		line := scanner.Text()
		if line != "" {
			slog.Debug("extension stderr", "cmd", m.cmd, "line", line)
		}
	}
}

// Stop gracefully shuts down the subprocess.
func (m *SubprocessManager) Stop() error {
	var stopErr error
	m.stopOnce.Do(func() {
		m.stopped.Store(true)

		// Send shutdown notification if process is still running
		if m.process != nil && m.process.Process != nil {
			shutdownReq := JSONRPCRequest{
				JSONRPC: "2.0",
				ID:      m.nextRequestID(),
				Method:  MethodShutdown,
			}
			data, _ := json.Marshal(shutdownReq)
			data = append(data, '\n')
			_, _ = m.stdin.Write(data) // Best effort
		}

		// Wait for process to exit with timeout
		done := make(chan error, 1)
		go func() {
			done <- m.process.Wait()
		}()

		select {
		case err := <-done:
			if err != nil {
				slog.Debug("extension process exited with error", "cmd", m.cmd, "error", err)
			} else {
				slog.Debug("extension process exited cleanly", "cmd", m.cmd)
			}
			stopErr = err
		case <-time.After(5 * time.Second):
			// Force kill process group
			if m.process != nil && m.process.Process != nil {
				slog.Warn("extension process did not exit gracefully, killing", "cmd", m.cmd)
				_ = m.process.Process.Kill()
				// Wait for process to be reaped
				_ = m.process.Wait()
			}
			stopErr = errors.New("process kill timeout")
		}

		// Close pipes
		_ = m.cleanupPipes()

		// Wait for reader goroutines
		m.readerWG.Wait()

		// Drain pending requests
		m.pendingReq.Range(func(key, value interface{}) bool {
			if ch, ok := value.(chan JSONRPCResponse); ok {
				close(ch)
			}
			m.pendingReq.Delete(key)
			return true
		})
	})

	return stopErr
}

// cleanupPipes closes all pipes.
func (m *SubprocessManager) cleanupPipes() error {
	var errs []error
	if m.stdin != nil {
		if err := m.stdin.Close(); err != nil {
			errs = append(errs, err)
		}
	}
	if m.stdout != nil {
		if err := m.stdout.Close(); err != nil {
			errs = append(errs, err)
		}
	}
	if m.stderr != nil {
		if err := m.stderr.Close(); err != nil {
			errs = append(errs, err)
		}
	}
	if len(errs) > 0 {
		return fmt.Errorf("cleanup pipes: %v", errs)
	}
	return nil
}

// IsRunning returns true if the subprocess is running.
func (m *SubprocessManager) IsRunning() bool {
	return m.started.Load() && !m.stopped.Load()
}

// ProtocolVersion returns the negotiated protocol version.
func (m *SubprocessManager) ProtocolVersion() string {
	return m.protocolVersion
}

// SupportedMethods returns the methods supported by the extension.
func (m *SubprocessManager) SupportedMethods() []string {
	return m.supportedMethods
}
