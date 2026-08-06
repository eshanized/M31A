package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"os/exec"
	"os/signal"
	"syscall"
)

type Request struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
}

type Response struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Result  any             `json:"result,omitempty"`
	Error   *Error          `json:"error,omitempty"`
}

type Error struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

const (
	MethodHandshake      = "handshake"
	MethodHookPrePhase   = "hook.pre_phase"
	MethodHookPostPhase  = "hook.post_phase"
)

type PhaseHookPayload struct {
	PhaseName       string          `json:"phase_name"`
	WorkflowState   WorkflowStateSnapshot `json:"workflow_state"`
	ExtensionConfig json.RawMessage   `json:"extension_config,omitempty"`
}

type WorkflowStateSnapshot struct {
	CurrentPhase    string          `json:"current_phase"`
	Goal            string          `json:"goal"`
	Tasks           []any           `json:"tasks"`
	Messages        []any           `json:"messages,omitempty"`
	SessionID       string          `json:"session_id"`
	BudgetSpentUSD  float64         `json:"budget_spent_usd"`
}

func main() {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
	go func() { <-sigCh; cancel() }()

	dec := json.NewDecoder(os.Stdin)
	enc := json.NewEncoder(os.Stdout)

	for {
		select {
		case <-ctx.Done():
			return
		default:
			var req Request
			if err := dec.Decode(&req); err != nil {
				log.Printf("decode error: %v", err)
				continue
			}
			enc.Encode(handle(ctx, req))
		}
	}
}

func handle(ctx context.Context, req Request) Response {
	switch req.Method {
	case MethodHandshake:
		return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{
			"protocol_version":  "1.0",
			"supported_methods": []string{MethodHookPrePhase, MethodHookPostPhase},
		}}
	case MethodHookPrePhase:
		var params struct {
			Payload PhaseHookPayload `json:"payload"`
		}
		if err := json.Unmarshal(req.Params, &params); err != nil {
			return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32602, Message: "invalid params"}}
		}

		// Run pre-phase check
		if err := runPrePhaseCheck(ctx, params.Payload); err != nil {
			log.Printf("pre-phase check failed: %v", err)
			return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32603, Message: err.Error()}}
		}

		return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{}}

	case MethodHookPostPhase:
		return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{}}

	default:
		return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32601, Message: "Method not found"}}
	}
}

func runPrePhaseCheck(ctx context.Context, payload PhaseHookPayload) error {
	// Check if we're in a git repo
	if _, err := os.Stat(".git"); os.IsNotExist(err) {
		return nil // Not a git repo, skip
	}

	// Get list of staged files
	cmd := exec.CommandContext(ctx, "git", "diff", "--cached", "--name-only")
	output, err := cmd.Output()
	if err != nil {
		return fmt.Errorf("git diff failed: %w", err)
	}

	files := parseFiles(string(output))
	if len(files) == 0 {
		return nil // No staged files
	}

	// Filter for Go files
	goFiles := filterGoFiles(files)
	if len(goFiles) == 0 {
		return nil // No Go files staged
	}

	log.Printf("Running pre-commit style check on %d Go files", len(goFiles))

	// Run gofmt -l on staged Go files
	args := append([]string{"gofmt", "-l"}, goFiles...)
	cmd = exec.CommandContext(ctx, args[0], args[1:]...)
	output, err = cmd.Output()
	if err != nil {
		// gofmt returns exit code 1 if files need formatting
		if len(output) > 0 {
			return fmt.Errorf("files need formatting:\n%s", string(output))
		}
		return fmt.Errorf("gofmt failed: %w", err)
	}

	if len(output) > 0 {
		return fmt.Errorf("files need formatting:\n%s", string(output))
	}

	log.Println("All Go files are properly formatted")
	return nil
}

func parseFiles(output string) []string {
	var files []string
	for _, line := range splitLines(output) {
		if line != "" {
			files = append(files, line)
		}
	}
	return files
}

func filterGoFiles(files []string) []string {
	var goFiles []string
	for _, f := range files {
		if len(f) > 3 && f[len(f)-3:] == ".go" {
			goFiles = append(goFiles, f)
		}
	}
	return goFiles
}

func splitLines(s string) []string {
	var lines []string
	start := 0
	for i := 0; i < len(s); i++ {
		if s[i] == '\n' {
			lines = append(lines, s[start:i])
			start = i + 1
		}
	}
	if start < len(s) {
		lines = append(lines, s[start:])
	}
	return lines
}