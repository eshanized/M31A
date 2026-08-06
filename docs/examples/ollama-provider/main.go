package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"os"
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
	MethodHandshake              = "handshake"
	MethodProviderName           = "provider.name"
	MethodProviderFetchModels    = "provider.fetch_models"
	MethodProviderChatCompletion = "provider.chat_completion_stream"
	MethodProviderEstimateCost   = "provider.estimate_cost"
	MethodProviderHealthCheck    = "provider.health_check"
)

type ModelInfo struct {
	ID            string   `json:"id"`
	Name          string   `json:"name"`
	ContextLength int      `json:"context_length"`
	Capabilities  []string `json:"capabilities,omitempty"`
}

type ChatRequest struct {
	Model    string        `json:"model"`
	Messages []Message     `json:"messages"`
	Stream   bool          `json:"stream"`
	Options  map[string]any `json:"options,omitempty"`
}

type Message struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

type ChatResponse struct {
	Model     string    `json:"model"`
	CreatedAt time.Time `json:"created_at"`
	Message   Message   `json:"message"`
	Done      bool      `json:"done"`
}

var (
	ollamaHost = "http://localhost:11434"
	defaultModel = "llama3"
)

func main() {
	// Allow override via env
	if h := os.Getenv("OLLAMA_HOST"); h != "" {
		ollamaHost = h
	}
	if m := os.Getenv("OLLAMA_MODEL"); m != "" {
		defaultModel = m
	}

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
			"supported_methods": []string{MethodProviderName, MethodProviderFetchModels, MethodProviderChatCompletion, MethodProviderEstimateCost, MethodProviderHealthCheck},
		}}
	case MethodProviderName:
		return Response{JSONRPC: "2.0", ID: req.ID, Result: "ollama"}
	case MethodProviderFetchModels:
		models, err := fetchModels(ctx)
		if err != nil {
			return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32603, Message: err.Error()}}
		}
		data, _ := json.Marshal(map[string]any{"models": models})
		return Response{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(data)}
	case MethodProviderChatCompletion:
		var params struct {
			Request ChatRequest `json:"request"`
		}
		if err := json.Unmarshal(req.Params, &params); err != nil {
			return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32602, Message: "invalid params"}}
		}

		model := params.Request.Model
		if model == "" {
			model = defaultModel
		}

		streamID := fmt.Sprintf("stream-%d", time.Now().UnixNano())

		// Send initial ack with stream_id
		ack := Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Result:  map[string]any{"stream_id": streamID},
		}

		// Start streaming in background
		go func() {
			streamChat(ctx, streamID, model, params.Request.Messages)
		}()

		return ack
	case MethodProviderEstimateCost:
		// Local provider = free
		return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"cost_usd": 0.0}}
	case MethodProviderHealthCheck:
		ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
		defer cancel()
		httpReq, _ := http.NewRequestWithContext(ctx, "GET", ollamaHost+"/api/version", nil)
		resp, err := http.DefaultClient.Do(httpReq)
		if err != nil {
			return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"status": "unhealthy", "error": err.Error()}}
		}
		defer resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"status": "degraded", "latency_ms": 0}}
		}
		return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"status": "healthy", "latency_ms": 10}}
	default:
		return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32601, Message: "Method not found"}}
	}
}

func fetchModels(ctx context.Context) ([]ModelInfo, error) {
	req, _ := http.NewRequestWithContext(ctx, "GET", ollamaHost+"/api/tags", nil)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var result struct {
		Models []struct {
			Name       string `json:"name"`
			Size       int64  `json:"size"`
			Digest     string `json:"digest"`
			ModifiedAt string `json:"modified_at"`
		} `json:"models"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, err
	}

	models := make([]ModelInfo, len(result.Models))
	for i, m := range result.Models {
		models[i] = ModelInfo{
			ID:            m.Name,
			Name:          m.Name,
			ContextLength: 4096,
			Capabilities:  []string{"chat", "completion"},
		}
	}
	return models, nil
}

func streamChat(ctx context.Context, streamID, model string, messages []Message) {
	ollamaReq := map[string]any{
		"model":    model,
		"messages": messages,
		"stream":   true,
	}
	body, _ := json.Marshal(ollamaReq)

	req, _ := http.NewRequestWithContext(ctx, "POST", ollamaHost+"/api/chat", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		sendNotification(streamID, map[string]any{"error": err.Error()})
		sendEndNotification(streamID)
		return
	}
	defer resp.Body.Close()

	dec := json.NewDecoder(resp.Body)
	for dec.More() {
		var chunk ChatResponse
		if err := dec.Decode(&chunk); err != nil {
			break
		}

		notification := map[string]any{
			"jsonrpc": "2.0",
			"method":  "stream.chunk",
			"params": map[string]any{
				"stream_id": streamID,
				"chunk": map[string]any{
					"content": chunk.Message.Content,
					"done":    chunk.Done,
				},
			},
		}
		sendNotification(streamID, notification["params"].(map[string]any))

		if chunk.Done {
			break
		}
	}
	sendEndNotification(streamID)
}

func sendNotification(streamID string, params map[string]any) {
	notification := map[string]any{
		"jsonrpc": "2.0",
		"method":  "stream.chunk",
		"params":  params,
	}
	enc := json.NewEncoder(os.Stdout)
	enc.Encode(notification)
}

func sendEndNotification(streamID string) {
	notification := map[string]any{
		"jsonrpc": "2.0",
		"method":  "stream.end",
		"params":  map[string]any{"stream_id": streamID},
	}
	enc := json.NewEncoder(os.Stdout)
	enc.Encode(notification)
}