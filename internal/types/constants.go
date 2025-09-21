package types

import "time"

const (
	ModelCacheTTL           = 5 * time.Minute
	HealthCheckInterval     = 60 * time.Second
	MaxFileSize             = 5 * 1024 * 1024
	MaxToolOutputChars      = 10_000
	MaxHealAttempts         = 2
	MaxPlanRetries          = 3
	SessionIDLength         = 8
	AutoDreamThreshold      = 0.60
	ContextWarningThreshold = 0.80
	HTTPDialTimeout         = 30 * time.Second
	BashTimeout             = 30 * time.Minute
	BashOutputLimit         = 50_000
	DefaultContextLength    = 128_000
	// Fix C-4: Maximum allowed LLM response size (1 MB) to prevent OOM
	// in parseToolCalls.
	MaxLLMResponseBytes = 1 << 20
	// DefaultPermissionTimeout is the default permission modal timeout in seconds.
	// Used by both the TUI permission modal and the tool dispatcher.
	DefaultPermissionTimeout = 300
	// StaleCacheTTL is the fallback TTL for stale model cache entries.
	StaleCacheTTL = 24 * time.Hour
	// DefaultHealthLiveMs is the default health check latency threshold for "live" status.
	DefaultHealthLiveMs = 500
	// DefaultHealthSlowMs is the default health check latency threshold for "slow" status.
	DefaultHealthSlowMs = 2000

	// Health status string constants (S-12)
	HealthStatusLive     = "live"
	HealthStatusSlow     = "slow"
	HealthStatusOffline  = "offline"
	HealthStatusDegraded = "degraded"

	// MaxProviderErrorChars is the max chars for sanitized provider errors (C-27)
	MaxProviderErrorChars = 200

	// DefaultOpenRouterBaseURL is the default OpenRouter API base URL (C-1)
	DefaultOpenRouterBaseURL = "https://openrouter.ai/api/v1"

	// DefaultZenBaseURL is the default Zen API base URL (C-1)
	DefaultZenBaseURL = "https://opencode.ai/zen/v1"

	// DefaultReferer is the default HTTP-Referer header for OpenRouter (C-2)
	DefaultReferer = "https://github.com/eshanized/M31A"

	// DefaultMaxRecentModels is the default number of recent models to remember
	DefaultMaxRecentModels = 10

	// DirPermission is the default directory permission (0755)
	DirPermission = 0755
	// FilePermission is the default file permission (0644)
	FilePermission = 0644

	// Tool default values (duplicated from tools/constants.go to avoid import cycle)
	DefaultMaxGlobResults       = 1000
	DefaultMaxGrepResults       = 100
	DefaultBashKillGraceSecs    = 5
	DefaultMaxBackupsPerFile    = 10
	DefaultWebfetchMaxRedirects = 5

	// CompressCooldown is the cooldown between /compress commands
	CompressCooldown = 60 * time.Second
	// ChannelSendTimeout is the timeout for sending on tea.Cmd channels
	ChannelSendTimeout = 500 * time.Millisecond
	// ToastDuration is how long toast messages display
	ToastDuration = 10 * time.Second

	// FetchModelsTimeout is the timeout for fetching model catalogs
	FetchModelsTimeout = 15 * time.Second
	// HealthCheckRetryDelay is the delay before retrying a failed health check
	HealthCheckRetryDelay = 5 * time.Second
	// MaxRetryAfterWait is the maximum wait time for retry-after headers
	MaxRetryAfterWait = 120 * time.Second

	// EMACorrectionAlpha is the default EMA correction rate for token estimation calibration.
	EMACorrectionAlpha = 0.3

	// MaxToolsPerCall is the max tool calls allowed per LLM response
	MaxToolsPerCall = 16
	// MaxCwdFileDepth is the max directory depth for cwd file schema
	MaxCwdFileDepth = 3

	// ConfigWatchInterval is the interval for watching config file changes
	ConfigWatchInterval = 5 * time.Second
	// MaxProjectConfigDepth is the max parent directory depth for project config discovery
	MaxProjectConfigDepth = 3
)
