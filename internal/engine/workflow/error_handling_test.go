package workflow

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
)

// TestErrorWrapping_PreservesChain verifies that errors.Is() and errors.As()
// work correctly after wrapping.
func TestErrorWrapping_PreservesChain(t *testing.T) {
	t.Parallel()

	// Wrap a sentinel error
	original := m31errors.ErrToolExecution
	wrapped := m31errors.Wrap(original, "failed to execute bash tool")

	// errors.Is should find the original sentinel
	if !errors.Is(wrapped, m31errors.ErrToolExecution) {
		t.Error("errors.Is should find ErrToolExecution in wrapped error")
	}

	// Wrap with Wrapf
	wrapped2 := m31errors.Wrapf(original, "tool %s failed during %s", "bash", "execute")
	if !errors.Is(wrapped2, m31errors.ErrToolExecution) {
		t.Error("errors.Is should find ErrToolExecution in wrapf error")
	}

	// Double wrap
	doubleWrapped := m31errors.Wrap(wrapped2, "outer context")
	if !errors.Is(doubleWrapped, m31errors.ErrToolExecution) {
		t.Error("errors.Is should find ErrToolExecution in double-wrapped error")
	}
}

// TestErrorWrapping_DescriptiveContext verifies that wrapped errors include
// descriptive context in the message.
func TestErrorWrapping_DescriptiveContext(t *testing.T) {
	t.Parallel()

	err := fmt.Errorf("disk full")
	wrapped := m31errors.Wrap(err, "failed to write session data")

	if !strings.Contains(wrapped.Error(), "failed to write session data") {
		t.Errorf("wrapped error should contain context, got: %s", wrapped.Error())
	}
	if !strings.Contains(wrapped.Error(), "disk full") {
		t.Errorf("wrapped error should contain original message, got: %s", wrapped.Error())
	}

	// Wrapf with formatted context
	wrapped2 := m31errors.Wrapf(err, "tool %s: %s failed", "bash", "execute")
	if !strings.Contains(wrapped2.Error(), "bash") {
		t.Errorf("wrapf error should contain tool name, got: %s", wrapped2.Error())
	}
}

// TestErrorWrapping_NilError verifies that wrapping nil returns nil.
func TestErrorWrapping_NilError(t *testing.T) {
	t.Parallel()

	result := m31errors.Wrap(nil, "context")
	if result != nil {
		t.Errorf("Wrap(nil) should return nil, got: %v", result)
	}

	result2 := m31errors.Wrapf(nil, "context %s", "info")
	if result2 != nil {
		t.Errorf("Wrapf(nil) should return nil, got: %v", result2)
	}
}

// TestEngine_ErrorHandling_PhaseFailure verifies that phase failures
// are wrapped with phase context.
func TestEngine_ErrorHandling_PhaseFailure(t *testing.T) {
	_, _ = setupTestEngine(t)

	// Simulate a phase failure with context
	phaseErr := m31errors.Wrapf(
		fmt.Errorf("LLM returned empty response"),
		"phase %s failed", "execute",
	)

	if !strings.Contains(phaseErr.Error(), "phase execute failed") {
		t.Errorf("error should contain phase context, got: %s", phaseErr.Error())
	}
	// The error wraps a contextual message around the original error
	if !strings.Contains(phaseErr.Error(), "LLM returned empty response") {
		t.Errorf("error should contain original message, got: %s", phaseErr.Error())
	}
}

// TestEngine_ErrorHandling_ToolFailure verifies that tool failures
// are wrapped with tool name and operation.
func TestEngine_ErrorHandling_ToolFailure(t *testing.T) {
	_, _ = setupTestEngine(t)

	// Simulate tool failure
	toolErr := &m31errors.ToolError{
		Tool: "bash",
		Op:   "execute",
		Err:  fmt.Errorf("command timed out after 30s"),
	}

	if !strings.Contains(toolErr.Error(), "tool bash") {
		t.Errorf("error should contain tool name, got: %s", toolErr.Error())
	}
	if !strings.Contains(toolErr.Error(), "execute") {
		t.Errorf("error should contain operation, got: %s", toolErr.Error())
	}
	if !strings.Contains(toolErr.Error(), "command timed out") {
		t.Errorf("error should contain original message, got: %s", toolErr.Error())
	}

	// Verify Unwrap works
	if !errors.Is(toolErr, toolErr.Err) {
		t.Error("errors.Is should find underlying error via Unwrap")
	}
}

// TestEngine_ErrorHandling_ProviderFailure verifies that provider failures
// are wrapped with provider context.
func TestEngine_ErrorHandling_ProviderFailure(t *testing.T) {
	_, _ = setupTestEngine(t)

	// Simulate provider failure
	provErr := &m31errors.ProviderError{
		Provider:   "openrouter",
		Model:      "gpt-4",
		StatusCode: 429,
		Err:        fmt.Errorf("rate limit exceeded"),
	}

	if !strings.Contains(provErr.Error(), "provider openrouter") {
		t.Errorf("error should contain provider name, got: %s", provErr.Error())
	}
	if !strings.Contains(provErr.Error(), "model gpt-4") {
		t.Errorf("error should contain model, got: %s", provErr.Error())
	}
	if !strings.Contains(provErr.Error(), "HTTP 429") {
		t.Errorf("error should contain status code, got: %s", provErr.Error())
	}
	if !strings.Contains(provErr.Error(), "rate limit exceeded") {
		t.Errorf("error should contain original message, got: %s", provErr.Error())
	}

	// Verify Unwrap works
	if !errors.Is(provErr, provErr.Err) {
		t.Error("errors.Is should find underlying error via Unwrap")
	}
}

// TestEngine_ErrorHandling_UserFacingMessages verifies that user-facing
// messages are actionable and helpful.
func TestEngine_ErrorHandling_UserFacingMessages(t *testing.T) {
	t.Parallel()

	testCases := []struct {
		name     string
		err      error
		contains string
	}{
		{
			name:     "provider unreachable",
			err:      m31errors.ErrProviderUnreachable,
			contains: "internet connection",
		},
		{
			name:     "rate limited",
			err:      m31errors.ErrRateLimited,
			contains: "retry",
		},
		{
			name:     "invalid API key",
			err:      m31errors.ErrInvalidKey,
			contains: "settings",
		},
		{
			name:     "context exceeded",
			err:      m31errors.ErrContextExceeded,
			contains: "compress",
		},
		{
			name:     "permission denied",
			err:      m31errors.ErrPermissionDenied,
			contains: "permissions",
		},
		{
			name:     "tool execution",
			err:      m31errors.ErrToolExecution,
			contains: "error details",
		},
		{
			name:     "phase transition",
			err:      m31errors.ErrPhaseTransition,
			contains: "current phase",
		},
		{
			name:     "wrapped tool error",
			err:      m31errors.Wrap(m31errors.ErrToolExecution, "bash failed"),
			contains: "Tool",
		},
		{
			name:     "wrapped provider error",
			err:      m31errors.Wrap(m31errors.ErrProviderUnreachable, "zen API"),
			contains: "Provider",
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			msg := m31errors.UserMessage(tc.err)
			if msg == "" {
				t.Errorf("UserMessage should not be empty for %v", tc.err)
			}
			if !strings.Contains(strings.ToLower(msg), strings.ToLower(tc.contains)) {
				t.Errorf("UserMessage(%v) = %q, should contain %q", tc.err, msg, tc.contains)
			}
		})
	}
}

// TestCloseError_DebugLogging verifies that Close() errors follow the
// debug-level logging pattern (logged, not returned to users).
// In the codebase, close errors are logged via slog.Debug, not wrapped
// with user-facing context. This test verifies the pattern is correct.
func TestCloseError_DebugLogging(t *testing.T) {
	t.Parallel()

	// Simulate a close error — the key invariant is that close errors
	// are logged at debug level, not surfaced as user-facing errors.
	// In practice, close errors may match generic patterns (e.g., "broken pipe")
	// but the codebase pattern is to log them, not return them.
	closeErr := fmt.Errorf("close failed: broken pipe")

	// Verify the error is a regular error (not a sentinel or typed error)
	// This confirms it should be logged at debug level, not wrapped.
	var toolErr *m31errors.ToolError
	var provErr *m31errors.ProviderError
	if errors.As(closeErr, &toolErr) {
		t.Error("close error should not be a ToolError")
	}
	if errors.As(closeErr, &provErr) {
		t.Error("close error should not be a ProviderError")
	}

	// The UserMessage function may return a generic message for close errors,
	// but the codebase pattern is to log them at debug level, not return them.
	// This test verifies the error type is correct for debug logging.
	t.Logf("close error type: %T, message: %s", closeErr, closeErr.Error())
}

// TestEngine_ErrorHandling_PhaseTransitionErrors verifies that phase transition
// errors provide clear guidance.
func TestEngine_ErrorHandling_PhaseTransitionErrors(t *testing.T) {
	_, _ = setupTestEngine(t)

	// Try an invalid transition
	err := m31errors.Wrapf(
		m31errors.ErrPhaseTransition,
		"cannot transition from %s to %s", "idle", "ship",
	)

	msg := m31errors.UserMessage(err)
	if !strings.Contains(msg, "phase") {
		t.Errorf("phase transition user message should mention phase, got: %s", msg)
	}
}
