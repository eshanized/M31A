package tools

import (
	"time"

	"github.com/eshanized/M31A/internal/types"
)

const (
	BashKillGracePeriod = time.Duration(types.DefaultBashKillGraceSecs) * time.Second
	BashWaitTimeout     = 30 * time.Second

	MaxBackupsPerFile = types.DefaultMaxBackupsPerFile
	DirPermission     = types.DirPermission
	FilePermission     = types.FilePermission

	MaxGlobResults = types.DefaultMaxGlobResults

	MaxGrepPatternLength  = 1024
	DefaultMaxGrepResults = types.DefaultMaxGrepResults

	MaxRedirects       = types.DefaultWebfetchMaxRedirects
	DefaultTimeoutSecs = 30
	MaxTimeoutSecs     = 120
	DNSCacheTTL        = 5 * time.Minute

	MinLinesForFuzzy     = 3
	LevenshteinThreshold = 0.7

	PermissionChannelBuffer = 8
	QuestionChannelBuffer   = 4
	DefaultAgentName        = "default"

	// Rate limiting: token bucket for tool execution.
	// Max 20 tools per second burst, sustained 10 tools/second.
	ToolRateLimitBurst  = 20
	ToolRateLimitPerSec = 10
)
