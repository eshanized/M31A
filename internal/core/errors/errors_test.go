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
		{"ErrNoBinaryContent", ErrNoBinaryContent, "Binary file cannot be displayed — use a tool to read its contents"},
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

		// Additional uncovered sentinels
		{"ErrProviderNotFound", ErrProviderNotFound, "Provider not found — use /settings to configure providers"},
		{"ErrInvalidProvider", ErrInvalidProvider, "Invalid provider name — cannot be empty"},
		{"ErrNoCredits", ErrNoCredits, "No credits remaining — please top up your account"},
		{"ErrBisectFailed", ErrBisectFailed, "Git bisect failed — check bisect state and retry"},
		{"ErrSessionNotFound", ErrSessionNotFound, "Session not found — check the session ID or start a new session"},
		{"ErrSessionPermission", ErrSessionPermission, "Cannot access session — check file permissions"},

		// New sentinel errors
		{"ErrNotImplemented", ErrNotImplemented, "Not implemented — this feature is not yet available"},

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

		// NVIDIA-specific model-not-found patterns
		{"model unavailable or deprecated", fmt.Errorf("model %q is unavailable or deprecated on NVIDIA NIM", "01-ai/yi-large"), "Model not found — run /models to see available models"},
		{"model not found generic", errors.New("model not found: some-model"), "Model not found — run /models to see available models"},
		{"generic not found", errors.New("resource not found"), "Resource not found — check the model or endpoint"},

		// Unknown error
		{"unknown", errors.New("something completely unexpected"), "An unexpected error occurred — check the logs or try again"},
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

func TestToolError(t *testing.T) {
	inner := errors.New("file not found")
	toolErr := &ToolError{Tool: "bash", Op: "execute", Err: inner}

	// Test Error()
	want := "tool bash: execute: file not found"
	if got := toolErr.Error(); got != want {
		t.Errorf("ToolError.Error() = %q, want %q", got, want)
	}

	// Test Unwrap()
	if !errors.Is(toolErr, inner) {
		t.Error("ToolError should unwrap to inner error")
	}

	// Test errors.As
	var target *ToolError
	if !errors.As(toolErr, &target) {
		t.Error("errors.As should find *ToolError")
	}
	if target.Tool != "bash" {
		t.Errorf("ToolError.Tool = %q, want %q", target.Tool, "bash")
	}
}

func TestToolError_NoOp(t *testing.T) {
	inner := errors.New("something went wrong")
	toolErr := &ToolError{Tool: "grep", Err: inner}

	want := "tool grep: something went wrong"
	if got := toolErr.Error(); got != want {
		t.Errorf("ToolError.Error() without Op = %q, want %q", got, want)
	}
}

func TestProviderError(t *testing.T) {
	inner := errors.New("rate limited")
	provErr := &ProviderError{Provider: "openrouter", Model: "gpt-4", StatusCode: 429, Err: inner}

	// Test Error()
	want := "provider openrouter model gpt-4 (HTTP 429): rate limited"
	if got := provErr.Error(); got != want {
		t.Errorf("ProviderError.Error() = %q, want %q", got, want)
	}

	// Test Unwrap()
	if !errors.Is(provErr, inner) {
		t.Error("ProviderError should unwrap to inner error")
	}

	// Test errors.As
	var target *ProviderError
	if !errors.As(provErr, &target) {
		t.Error("errors.As should find *ProviderError")
	}
	if target.Provider != "openrouter" {
		t.Errorf("ProviderError.Provider = %q, want %q", target.Provider, "openrouter")
	}
}

func TestProviderError_NoModel(t *testing.T) {
	inner := errors.New("timeout")
	provErr := &ProviderError{Provider: "zen", StatusCode: 504, Err: inner}

	want := "provider zen (HTTP 504): timeout"
	if got := provErr.Error(); got != want {
		t.Errorf("ProviderError.Error() without Model = %q, want %q", got, want)
	}
}

func TestProviderError_NoStatus(t *testing.T) {
	inner := errors.New("connection refused")
	provErr := &ProviderError{Provider: "nvidia", Err: inner}

	want := "provider nvidia: connection refused"
	if got := provErr.Error(); got != want {
		t.Errorf("ProviderError.Error() without StatusCode = %q, want %q", got, want)
	}
}

func TestConfigError(t *testing.T) {
	inner := errors.New("invalid value")
	cfgErr := &ConfigError{Key: "model.default", Err: inner}

	// Test Error()
	want := "config model.default: invalid value"
	if got := cfgErr.Error(); got != want {
		t.Errorf("ConfigError.Error() = %q, want %q", got, want)
	}

	// Test Unwrap()
	if !errors.Is(cfgErr, inner) {
		t.Error("ConfigError should unwrap to inner error")
	}

	// Test errors.As
	var target *ConfigError
	if !errors.As(cfgErr, &target) {
		t.Error("errors.As should find *ConfigError")
	}
	if target.Key != "model.default" {
		t.Errorf("ConfigError.Key = %q, want %q", target.Key, "model.default")
	}
}

func TestConfigError_NoKey(t *testing.T) {
	inner := errors.New("file not found")
	cfgErr := &ConfigError{Err: inner}

	want := "config: file not found"
	if got := cfgErr.Error(); got != want {
		t.Errorf("ConfigError.Error() without Key = %q, want %q", got, want)
	}
}

func TestErrorTypesInUserMessage(t *testing.T) {
	// ToolError
	toolErr := &ToolError{Tool: "bash", Op: "execute", Err: errors.New("exit code 1")}
	got := UserMessage(toolErr)
	want := "Tool bash failed — check the error details"
	if got != want {
		t.Errorf("UserMessage(ToolError) = %q, want %q", got, want)
	}

	// ProviderError
	provErr := &ProviderError{Provider: "openrouter", Err: errors.New("rate limited")}
	got = UserMessage(provErr)
	want = "Provider error from openrouter — try again"
	if got != want {
		t.Errorf("UserMessage(ProviderError) = %q, want %q", got, want)
	}

	// ConfigError
	cfgErr := &ConfigError{Key: "api.key", Err: errors.New("missing")}
	got = UserMessage(cfgErr)
	want = "Configuration error — check your settings"
	if got != want {
		t.Errorf("UserMessage(ConfigError) = %q, want %q", got, want)
	}
}

func TestSentinelsAreUnique(t *testing.T) {
	sentinels := []error{
		ErrProviderUnreachable, ErrProviderNotFound, ErrInvalidProvider,
		ErrRateLimited, ErrInvalidKey, ErrNoCredits, ErrContextExceeded,
		ErrModelNotFound, ErrSessionCorrupted, ErrNoBinaryContent,
		ErrFileTooLarge, ErrCircularDependency, ErrPermissionDenied,
		ErrToolExecution, ErrTaskFailed, ErrPhaseTransition,
		ErrCheckpointNotFound, ErrToolInputTooLarge, ErrInvalidTimeout,
		ErrPrivateIPBlocked, ErrStreamTruncated, ErrBisectResetFailed,
		ErrBisectFailed, ErrSessionNotFound, ErrSessionPermission,
		ErrGitNotInitialized, ErrNotImplemented,
	}

	seen := make(map[error]bool)
	for i, s := range sentinels {
		if s == nil {
			t.Errorf("sentinel %d is nil", i)
			continue
		}
		if seen[s] {
			t.Errorf("duplicate sentinel: %v", s)
		}
		seen[s] = true
	}
}

func TestWrapWithPercentW(t *testing.T) {
	// Verify that wrapping with %w preserves errors.Is behavior
	inner := ErrInvalidKey
	wrapped := fmt.Errorf("auth failed: %w", inner)
	if !errors.Is(wrapped, inner) {
		t.Error("fmt.Errorf with %w should preserve errors.Is")
	}

	// Verify error type wrapping
	toolErr := &ToolError{Tool: "bash", Err: inner}
	wrapped2 := fmt.Errorf("exec failed: %w", toolErr)
	var target *ToolError
	if !errors.As(wrapped2, &target) {
		t.Error("errors.As should find ToolError through wrapping")
	}
}
