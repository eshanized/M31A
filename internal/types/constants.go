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
)
