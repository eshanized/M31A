package errors

import (
	"errors"
	"regexp"
	"strings"
)

var (
	reHTTP401              = regexp.MustCompile(`\b401\b`)
	reHTTP429              = regexp.MustCompile(`\b429\b`)
	reHTTP503              = regexp.MustCompile(`\b503\b`)
	ErrProviderUnreachable = errors.New("provider unreachable")
	ErrProviderNotFound    = errors.New("provider not found")
	ErrInvalidProvider     = errors.New("invalid provider name")
	ErrRateLimited         = errors.New("rate limited")
	ErrInvalidKey          = errors.New("invalid API key")
	ErrNoCredits           = errors.New("no credits available")
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
	// Reject oversized LLM response payloads to prevent OOM.
	ErrToolInputTooLarge = errors.New("tool input exceeds size limit")
	// Bash timeout must be positive and <= 30 minutes.
	ErrInvalidTimeout = errors.New("invalid timeout: must be > 0 and <= 30m")
	// WebFetch SSRF — block private/loopback/link-local IPs.
	ErrPrivateIPBlocked = errors.New("access to private IP is blocked (SSRF protection)")
	// SSE stream truncated before [DONE] sentinel.
	ErrStreamTruncated = errors.New("stream truncated before completion")
	// git bisect reset failed (e.g. no commits in range).
	ErrBisectResetFailed = errors.New("bisect reset failed")
	ErrBisectFailed      = errors.New("bisect failed")

	// Session-specific errors for distinct failure modes.
	ErrSessionNotFound   = errors.New("session not found")
	ErrSessionPermission = errors.New("session access denied")
	ErrGitNotInitialized = errors.New("git not initialized")
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
	case errors.Is(e, ErrProviderNotFound):
		return "Provider not found — use /settings to configure providers"
	case errors.Is(e, ErrInvalidProvider):
		return "Invalid provider name — cannot be empty"
	case errors.Is(e, ErrRateLimited):
		return "Rate limited — retry in a moment"
	case errors.Is(e, ErrInvalidKey):
		return "Invalid API key — run /settings to update"
	case errors.Is(e, ErrNoCredits):
		return "No credits remaining — please top up your account"
	case errors.Is(e, ErrContextExceeded):
		return "Context window exceeded — conversation too long. Use /compress to reduce context."
	case errors.Is(e, ErrModelNotFound):
		return "Model not found — use /models to see available models"
	case errors.Is(e, ErrSessionCorrupted):
		return "Session data corrupted — try resuming from a different session"
	case errors.Is(e, ErrNoBinaryContent):
		return "Binary file cannot be displayed — use a tool to read its contents"
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
	case errors.Is(e, ErrBisectFailed):
		return "Git bisect failed — check bisect state and retry"
	case errors.Is(e, ErrSessionNotFound):
		return "Session not found — check the session ID or start a new session"
	case errors.Is(e, ErrSessionPermission):
		return "Cannot access session — check file permissions"
	case errors.Is(e, ErrGitNotInitialized):
		return "Git not initialized — ensure you're in a git repository"
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
	case strings.Contains(errStr, "i/o timeout") || strings.Contains(errStr, "dial tcp"):
		return "Connection timed out — check your internet connection"
	case strings.Contains(errStr, "tls:") || strings.Contains(errStr, "certificate") || strings.Contains(errStr, "tls handshake"):
		return "TLS error — check your network proxy or certificates"
	case strings.Contains(errStr, "bad request") || strings.Contains(errStr, "status 400"):
		return "Bad request — check your input parameters"
	case strings.Contains(errStr, "forbidden") || strings.Contains(errStr, "status 403"):
		return "Access forbidden — check API key scope or permissions"
	case strings.Contains(errStr, "not found") || strings.Contains(errStr, "status 404"):
		return "Resource not found — check the model or endpoint"
	case strings.Contains(errStr, "provider error"):
		// Extract the provider's actual error message after "provider error: "
		if idx := strings.Index(e.Error(), "provider error: "); idx >= 0 {
			return e.Error()[idx:]
		}
		return "Provider returned an error — try again"
	case strings.Contains(errStr, "internal server error") || strings.Contains(errStr, "status 500"):
		return "Provider internal error — try again later"
	case reHTTP401.MatchString(errStr):
		return "Invalid API key — run /settings to update"
	case reHTTP429.MatchString(errStr):
		return "Rate limited — retry in a moment"
	case reHTTP503.MatchString(errStr):
		return "Provider temporarily unavailable — try again later"
	}

	return "An unexpected error occurred — check the logs or try again"
}
