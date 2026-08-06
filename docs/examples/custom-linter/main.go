package main

import (
	"context"
	"encoding/json"
	"log"
	"os"
	"os/exec"
	"os/signal"
	"syscall"
	"time"
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
	MethodHandshake        = "handshake"
	MethodToolName         = "tool.name"
	MethodToolDescription  = "tool.description"
	MethodToolRiskLevel    = "tool.risk_level"
	MethodToolSchema       = "tool.schema"
	MethodToolExecute      = "tool.execute"
)

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
			"supported_methods": []string{MethodToolName, MethodToolDescription, MethodToolRiskLevel, MethodToolSchema, MethodToolExecute},
		}}
	case MethodToolName:
		return Response{JSONRPC: "2.0", ID: req.ID, Result: "custom-linter"}
	case MethodToolDescription:
		return Response{JSONRPC: "2.0", ID: req.ID, Result: "Runs custom linting rules via golangci-lint"}
	case MethodToolRiskLevel:
		return Response{JSONRPC: "2.0", ID: req.ID, Result: "safe"}
	case MethodToolSchema:
		schema := map[string]any{
			"type": "object",
			"properties": map[string]any{
				"path": map[string]string{"type": "string", "description": "File or directory to lint"},
			},
			"required": []string{"path"},
		}
		data, _ := json.Marshal(schema)
		return Response{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(data)}
	case MethodToolExecute:
		var params struct {
			Input map[string]any `json:"input"`
		}
		if err := json.Unmarshal(req.Params, &params); err != nil {
			return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32602, Message: "invalid params"}}
		}

		path, ok := params.Input["path"].(string)
		if !ok || path == "" {
			return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32602, Message: "path is required"}}
		}

		execCtx, execCancel := context.WithTimeout(ctx, 60*time.Second)
		defer execCancel()

		cmd := exec.CommandContext(execCtx, "golangci-lint", "run", path)
		output, err := cmd.CombinedOutput()

		if err != nil {
			if execCtx.Err() == context.DeadlineExceeded {
				return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32603, Message: "lint timeout"}}
			}
			// golangci-lint returns non-zero on lint issues, that's expected
		}

		return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{
			"output": string(output),
		}}
	default:
		return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32601, Message: "Method not found"}}
	}
}