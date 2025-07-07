package errors

import (
	"errors"
	"strings"
)

var (
	ErrProviderUnreachable = errors.New("provider unreachable")
	ErrRateLimited         = errors.New("rate limited")
	ErrInvalidKey          = errors.New("invalid API key")
	ErrContextExceeded     = errors.New("context window exceeded")
	ErrModelNotFound       = errors.New("model not found")
	ErrSessionCorrupted    = errors.New("session data corrupted")
	ErrNoBinaryContent     = errors.New("binary content not displayable")
	ErrFileTooLarge        = errors.New("file exceeds 5MB limit")
	ErrCircularDependency  = errors.New("circular dependency in task graph")
	ErrPermissionDenied    = errors.New("permission denied")
	ErrToolExecution       = errors.New("tool execution failed")
	ErrTaskFailed          = errors.New("task failed")
	ErrPhaseTransition     = errors.New("invalid phase transition")
	ErrCheckpointNotFound  = errors.New("checkpoint not found")
	// Fix C-4: Reject oversized LLM response payloads to prevent OOM.
	ErrToolInputTooLarge = errors.New("tool input exceeds size limit")
	// Fix C-5: Bash timeout must be positive and <= 30 minutes.
	ErrInvalidTimeout = errors.New("invalid timeout: must be > 0 and <= 30m")
	// Fix C-7: WebFetch SSRF — block private/loopback/link-local IPs.
	ErrPrivateIPBlocked = errors.New("access to private IP is blocked (SSRF protection)")
	// Fix M-13: SSE stream truncated before [DONE] sentinel.
	ErrStreamTruncated = errors.New("stream truncated before completion")
	// Fix M-29: git bisect reset failed (e.g. no commits in range).
	ErrBisectResetFailed = errors.New("bisect reset failed")

	// Session-specific errors for distinct failure modes.
	ErrSessionNotFound    = errors.New("session not found")
	ErrSessionPermission  = errors.New("session access denied")
)

// UserMessage returns a user-friendly, actionable message for common errors.
// Falls back to a generic message for unrecognized errors.
func UserMessage(e error) string {
	if e == nil {
		return ""
	}

	// Check sentinel errors first (exact match via errors.Is)
	switch {
	case errors.Is(e, ErrProviderUnreachable):
		return "Provider unreachable — check your internet connection"
	case errors.Is(e, ErrRateLimited):
		return "Rate limited — retry in a moment"
	case errors.Is(e, ErrInvalidKey):
		return "Invalid API key — run /settings to update"
	case errors.Is(e, ErrContextExceeded):
		return "Context window exceeded — conversation too long. Use /compress to reduce context."
	case errors.Is(e, ErrModelNotFound):
		return "Model not found — use /models to see available models"
	case errors.Is(e, ErrSessionCorrupted):
		return "Session data corrupted — try resuming from a different session"
	case errors.Is(e, ErrNoBinaryContent):
		return "Binary file cannot be displayed"
	case errors.Is(e, ErrFileTooLarge):
		return "File exceeds 5MB limit — use a smaller file"
	case errors.Is(e, ErrCircularDependency):
		return "Circular dependency in task graph — check task dependencies"
	case errors.Is(e, ErrPermissionDenied):
		return "Permission denied — check file permissions"
	case errors.Is(e, ErrToolExecution):
		return "Tool execution failed — check the error details"
	case errors.Is(e, ErrTaskFailed):
		return "Task failed — check the task output for details"
	case errors.Is(e, ErrPhaseTransition):
		return "Invalid phase transition — current phase does not allow this action"
	case errors.Is(e, ErrCheckpointNotFound):
		return "Checkpoint not found — no previous state to restore"
	case errors.Is(e, ErrToolInputTooLarge):
		return "Tool input too large — reduce the input size"
	case errors.Is(e, ErrInvalidTimeout):
		return "Invalid timeout — must be between 1 and 30 minutes"
	case errors.Is(e, ErrPrivateIPBlocked):
		return "Access to private IP blocked — SSRF protection active"
	case errors.Is(e, ErrStreamTruncated):
		return "Stream interrupted — try again"
	case errors.Is(e, ErrBisectResetFailed):
		return "Git bisect reset failed — try `git bisect reset` manually"
	case errors.Is(e, ErrSessionNotFound):
		return "Session not found — check the session ID or start a new session"
	case errors.Is(e, ErrSessionPermission):
		return "Cannot access session — check file permissions"
	}

	// Pattern matching for unwrapped errors
	errStr := strings.ToLower(e.Error())
	switch {
	case strings.Contains(errStr, "connection refused"):
		return "Cannot reach provider — check your internet connection"
	case strings.Contains(errStr, "context canceled") || strings.Contains(errStr, "context deadline exceeded"):
		return "Request cancelled"
	case strings.Contains(errStr, "eof") || strings.Contains(errStr, "unexpected end of json"):
		return "Connection lost — try again"
	case strings.Contains(errStr, "401"):
		return "Invalid API key — run /settings to update"
	case strings.Contains(errStr, "429"):
		return "Rate limited — retry in a moment"
	case strings.Contains(errStr, "503"):
		return "Provider temporarily unavailable — try again later"
	}

	return "An unexpected error occurred"
}
