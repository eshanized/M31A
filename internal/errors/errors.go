package errors

import "errors"

var (
	ErrProviderUnreachable   = errors.New("provider unreachable")
	ErrRateLimited           = errors.New("rate limited")
	ErrInvalidKey            = errors.New("invalid API key")
	ErrContextExceeded       = errors.New("context window exceeded")
	ErrModelNotFound         = errors.New("model not found")
	ErrSessionCorrupted      = errors.New("session data corrupted")
	ErrNoBinaryContent       = errors.New("binary content not displayable")
	ErrFileTooLarge          = errors.New("file exceeds 5MB limit")
	ErrCircularDependency    = errors.New("circular dependency in task graph")
	ErrPermissionDenied      = errors.New("permission denied")
	ErrToolExecution         = errors.New("tool execution failed")
	ErrTaskFailed            = errors.New("task failed")
	ErrPhaseTransition       = errors.New("invalid phase transition")
	ErrCheckpointNotFound    = errors.New("checkpoint not found")
	// Fix C-4: Reject oversized LLM response payloads to prevent OOM.
	ErrToolInputTooLarge = errors.New("tool input exceeds size limit")
	// Fix C-5: Bash timeout must be positive and <= 30 minutes.
	ErrInvalidTimeout = errors.New("invalid timeout: must be > 0 and <= 30m")
	// Fix C-7: WebFetch SSRF — block private/loopback/link-local IPs.
	ErrPrivateIPBlocked = errors.New("access to private IP is blocked (SSRF protection)")
	// Fix M-13: SSE stream truncated before [DONE] sentinel.
	ErrStreamTruncated = errors.New("stream truncated before completion")
)
