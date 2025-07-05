package errors

import (
	"errors"
	"fmt"
	"testing"
)

func TestUserMessage(t *testing.T) {
	tests := []struct {
		name     string
		err      error
		expected string
	}{
		// Sentinel errors
		{"nil", nil, ""},
		{"ErrProviderUnreachable", ErrProviderUnreachable, "Provider unreachable — check your internet connection"},
		{"ErrRateLimited", ErrRateLimited, "Rate limited — retry in a moment"},
		{"ErrInvalidKey", ErrInvalidKey, "Invalid API key — run /settings to update"},
		{"ErrContextExceeded", ErrContextExceeded, "Context window exceeded — conversation too long. Use /compress to reduce context."},
		{"ErrModelNotFound", ErrModelNotFound, "Model not found — use /models to see available models"},
		{"ErrSessionCorrupted", ErrSessionCorrupted, "Session data corrupted — try resuming from a different session"},
		{"ErrNoBinaryContent", ErrNoBinaryContent, "Binary file cannot be displayed"},
		{"ErrFileTooLarge", ErrFileTooLarge, "File exceeds 5MB limit — use a smaller file"},
		{"ErrCircularDependency", ErrCircularDependency, "Circular dependency in task graph — check task dependencies"},
		{"ErrPermissionDenied", ErrPermissionDenied, "Permission denied — check file permissions"},
		{"ErrToolExecution", ErrToolExecution, "Tool execution failed — check the error details"},
		{"ErrTaskFailed", ErrTaskFailed, "Task failed — check the task output for details"},
		{"ErrPhaseTransition", ErrPhaseTransition, "Invalid phase transition — current phase does not allow this action"},
		{"ErrCheckpointNotFound", ErrCheckpointNotFound, "Checkpoint not found — no previous state to restore"},
		{"ErrToolInputTooLarge", ErrToolInputTooLarge, "Tool input too large — reduce the input size"},
		{"ErrInvalidTimeout", ErrInvalidTimeout, "Invalid timeout — must be between 1 and 30 minutes"},
		{"ErrPrivateIPBlocked", ErrPrivateIPBlocked, "Access to private IP blocked — SSRF protection active"},
		{"ErrStreamTruncated", ErrStreamTruncated, "Stream interrupted — try again"},
		{"ErrBisectResetFailed", ErrBisectResetFailed, "Git bisect reset failed — try `git bisect reset` manually"},

		// Wrapped sentinel errors
		{"wrapped ErrInvalidKey", fmt.Errorf("auth failed: %w", ErrInvalidKey), "Invalid API key — run /settings to update"},

		// Pattern-matched errors
		{"connection refused", errors.New("dial tcp: connection refused"), "Cannot reach provider — check your internet connection"},
		{"context canceled", errors.New("context canceled"), "Request cancelled"},
		{"context deadline exceeded", errors.New("context deadline exceeded"), "Request cancelled"},
		{"EOF", errors.New("unexpected EOF"), "Connection lost — try again"},
		{"unexpected end of JSON", errors.New("unexpected end of JSON input"), "Connection lost — try again"},
		{"HTTP 401", errors.New("HTTP 401 Unauthorized"), "Invalid API key — run /settings to update"},
		{"HTTP 429", errors.New("HTTP 429 Too Many Requests"), "Rate limited — retry in a moment"},
		{"HTTP 503", errors.New("HTTP 503 Service Unavailable"), "Provider temporarily unavailable — try again later"},

		// Unknown error
		{"unknown", errors.New("something completely unexpected"), "An unexpected error occurred"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := UserMessage(tt.err)
			if got != tt.expected {
				t.Errorf("UserMessage(%v) = %q, want %q", tt.err, got, tt.expected)
			}
		})
	}
}
