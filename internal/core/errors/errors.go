package errors

import (
	"errors"
	"fmt"
	"regexp"
	"strings"
)

var (
	reHTTP401 = regexp.MustCompile(`\b401\b`)
	reHTTP429 = regexp.MustCompile(`\b429\b`)
	reHTTP503 = regexp.MustCompile(`\b503\b`)

	// Provider errors.
	ErrProviderUnreachable = errors.New("provider unreachable")
	ErrProviderNotFound    = errors.New("provider not found")
	ErrInvalidProvider     = errors.New("invalid provider name")
	ErrRateLimited         = errors.New("rate limited")
	ErrInvalidKey          = errors.New("invalid API key")
	ErrNoCredits           = errors.New("no credits available")
	ErrContextExceeded     = errors.New("context window exceeded")
	ErrModelNotFound       = errors.New("model not found")
	// SSE stream truncated before [DONE] sentinel.
	ErrStreamTruncated = errors.New("stream truncated before completion")

	// Session and workflow errors.
	ErrSessionCorrupted   = errors.New("session data corrupted")
	ErrSessionNotFound    = errors.New("session not found")
	ErrSessionPermission  = errors.New("session access denied")
	ErrCheckpointNotFound = errors.New("checkpoint not found")
	ErrPhaseTransition    = errors.New("invalid phase transition")
	ErrTaskFailed         = errors.New("task failed")

	// Tool and input errors.
	ErrToolExecution      = errors.New("tool execution failed")
	ErrToolInputTooLarge  = errors.New("tool input exceeds size limit")
	ErrCircularDependency = errors.New("circular dependency in task graph")

	// File and permission errors.
	ErrPermissionDenied = errors.New("permission denied")
	ErrNoBinaryContent  = errors.New("binary content not displayable")
	ErrFileTooLarge     = errors.New("file exceeds 5MB limit")

	// Validation errors.
	ErrInvalidTimeout = errors.New("invalid timeout: must be > 0 and <= 30m")

	// State errors.
	ErrNotImplemented = errors.New("not implemented")

	// Network and security errors.
	ErrPrivateIPBlocked = errors.New("access to private IP is blocked (SSRF protection)")

	// Git errors.
	ErrBisectResetFailed = errors.New("bisect reset failed")
	ErrBisectFailed      = errors.New("bisect failed")
	ErrGitNotInitialized = errors.New("git not initialized")

	// Intelligence errors (Phase 04).
	ErrSourceUnreachable       = errors.New("intelligence source unreachable")
	ErrCheckpointPending       = errors.New("dependency checkpoint pending approval")
	ErrNotReproducibleInWindow = errors.New("symptom not reproducible in bisect window")

	// Event store errors.
	ErrEventStoreUnavailable = errors.New("event store unavailable")
	ErrEventNotFound         = errors.New("event not found")
	ErrInvalidEventPayload   = errors.New("invalid event payload")
	ErrMigrationFailed       = errors.New("migration failed")
	ErrWriteConflict         = errors.New("write conflict: another process is writing")
	ErrBackupFailed          = errors.New("backup failed")
	ErrProjectionNotFound    = errors.New("projection not found")
	ErrProjectionApplyFailed = errors.New("projection apply failed")
	ErrArtifactParseFailed   = errors.New("artifact parse failed")
	ErrKeyNotFound           = errors.New("key not found")
	ErrKeychainUnavailable   = errors.New("keychain unavailable")
)

// ToolError wraps errors originating from tool execution with tool-specific context.
type ToolError struct {
	// Tool is the name of the tool that failed.
	Tool string
	// Op is the operation attempted (e.g., "execute", "parse input").
	Op string
	// Err is the underlying error.
	Err error
}

func (e *ToolError) Error() string {
	if e.Op != "" {
		return "tool " + e.Tool + ": " + e.Op + ": " + e.Err.Error()
	}
	return "tool " + e.Tool + ": " + e.Err.Error()
}

// Unwrap returns the underlying error.
func (e *ToolError) Unwrap() error { return e.Err }

// ProviderError wraps errors from API providers with HTTP context.
type ProviderError struct {
	// Provider is the provider name (e.g., "openrouter", "zen").
	Provider string
	// Model is the requested model identifier, if applicable.
	Model string
	// StatusCode is the HTTP status code from the provider, 0 if not HTTP.
	StatusCode int
	// Err is the underlying error.
	Err error
}

func (e *ProviderError) Error() string {
	msg := "provider " + e.Provider
	if e.Model != "" {
		msg += " model " + e.Model
	}
	if e.StatusCode > 0 {
		msg += " (HTTP " + itoa(e.StatusCode) + ")"
	}
	return msg + ": " + e.Err.Error()
}

// Unwrap returns the underlying error.
func (e *ProviderError) Unwrap() error { return e.Err }

// ConfigError wraps configuration loading or validation errors.
type ConfigError struct {
	// Key is the configuration key that caused the error, if applicable.
	Key string
	// Err is the underlying error.
	Err error
}

func (e *ConfigError) Error() string {
	if e.Key != "" {
		return "config " + e.Key + ": " + e.Err.Error()
	}
	return "config: " + e.Err.Error()
}

// Unwrap returns the underlying error.
func (e *ConfigError) Unwrap() error { return e.Err }

// itoa converts an int to its decimal string representation without importing strconv.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[i:])
}

// Wrap wraps an error with a descriptive message, preserving the error chain.
// Returns nil if err is nil. Use this instead of fmt.Errorf("...: %w", err)
// for consistent error wrapping across the codebase.
func Wrap(err error, msg string) error {
	if err == nil {
		return nil
	}
	return fmt.Errorf("%s: %w", msg, err)
}

// Wrapf wraps an error with a formatted message, preserving the error chain.
// Returns nil if err is nil. Use this for error wrapping with dynamic context.
func Wrapf(err error, format string, args ...any) error {
	if err == nil {
		return nil
	}
	return fmt.Errorf("%s: %w", fmt.Sprintf(format, args...), err)
}

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
	case errors.Is(e, ErrNotImplemented):
		return "Not implemented — this feature is not yet available"
	}

	// Handle error types with structured context.
	var toolErr *ToolError
	if errors.As(e, &toolErr) {
		return "Tool " + toolErr.Tool + " failed — check the error details"
	}
	var provErr *ProviderError
	if errors.As(e, &provErr) {
		return "Provider error from " + provErr.Provider + " — try again"
	}
	var cfgErr *ConfigError
	if errors.As(e, &cfgErr) {
		return "Configuration error — check your settings"
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
	case strings.Contains(errStr, "closed network connection") || strings.Contains(errStr, "broken pipe") || strings.Contains(errStr, "connection reset"):
		return "Connection lost — try again"
	case strings.Contains(errStr, "i/o timeout") || strings.Contains(errStr, "dial tcp"):
		return "Connection timed out — check your internet connection"
	case strings.Contains(errStr, "tls:") || strings.Contains(errStr, "certificate") || strings.Contains(errStr, "tls handshake"):
		return "TLS error — check your network proxy or certificates"
	case strings.Contains(errStr, "bad request") || strings.Contains(errStr, "status 400"):
		return "Bad request — check your input parameters"
	case strings.Contains(errStr, "forbidden") || strings.Contains(errStr, "status 403"):
		return "Access forbidden — check API key scope or permissions"
	case strings.Contains(errStr, "unavailable or deprecated") ||
		(strings.Contains(errStr, "not found") && strings.Contains(errStr, "model")):
		return "Model not found — run /models to see available models"
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
