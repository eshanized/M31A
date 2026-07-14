package errors

import (
	pkgErrors "github.com/eshanized/M31A/pkg/errors"
)

// Sentinel errors — re-exported from pkg/errors for backward compatibility.

var (
	ErrProviderUnreachable = pkgErrors.ErrProviderUnreachable
	ErrProviderNotFound    = pkgErrors.ErrProviderNotFound
	ErrInvalidProvider     = pkgErrors.ErrInvalidProvider
	ErrRateLimited         = pkgErrors.ErrRateLimited
	ErrInvalidKey          = pkgErrors.ErrInvalidKey
	ErrNoCredits           = pkgErrors.ErrNoCredits
	ErrContextExceeded     = pkgErrors.ErrContextExceeded
	ErrModelNotFound       = pkgErrors.ErrModelNotFound
	ErrStreamTruncated     = pkgErrors.ErrStreamTruncated

	ErrSessionCorrupted   = pkgErrors.ErrSessionCorrupted
	ErrSessionNotFound    = pkgErrors.ErrSessionNotFound
	ErrSessionPermission  = pkgErrors.ErrSessionPermission
	ErrCheckpointNotFound = pkgErrors.ErrCheckpointNotFound
	ErrPhaseTransition    = pkgErrors.ErrPhaseTransition
	ErrTaskFailed         = pkgErrors.ErrTaskFailed

	ErrToolExecution      = pkgErrors.ErrToolExecution
	ErrToolInputTooLarge  = pkgErrors.ErrToolInputTooLarge
	ErrCircularDependency = pkgErrors.ErrCircularDependency

	ErrPermissionDenied = pkgErrors.ErrPermissionDenied
	ErrNoBinaryContent  = pkgErrors.ErrNoBinaryContent
	ErrFileTooLarge     = pkgErrors.ErrFileTooLarge

	ErrInvalidInput   = pkgErrors.ErrInvalidInput
	ErrInvalidTimeout = pkgErrors.ErrInvalidTimeout

	ErrNotFound = pkgErrors.ErrNotFound

	ErrAlreadyExists  = pkgErrors.ErrAlreadyExists
	ErrCancelled      = pkgErrors.ErrCancelled
	ErrInternal       = pkgErrors.ErrInternal
	ErrNotImplemented = pkgErrors.ErrNotImplemented

	ErrPrivateIPBlocked = pkgErrors.ErrPrivateIPBlocked

	ErrBisectResetFailed = pkgErrors.ErrBisectResetFailed
	ErrBisectFailed      = pkgErrors.ErrBisectFailed
	ErrGitNotInitialized = pkgErrors.ErrGitNotInitialized
)

// Error types — aliases from pkg/errors.

type ToolError = pkgErrors.ToolError

type ProviderError = pkgErrors.ProviderError

type ConfigError = pkgErrors.ConfigError

// UserMessage returns a user-friendly, actionable message for common errors.
func UserMessage(e error) string {
	return pkgErrors.UserMessage(e)
}
