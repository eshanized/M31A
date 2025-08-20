package tools

import "time"

const (
	// Bash tool constants
	BashKillGracePeriod = 5 * time.Second
	BashWaitTimeout     = 30 * time.Second

	// FileWrite constants
	MaxBackupsPerFile = 10
	DirPermission     = 0755
	FilePermission     = 0644

	// Glob constants
	MaxGlobResults = 1000
	DateFormat      = "2006-01-02 15:04"

	// Grep constants
	MaxGrepPatternLength  = 1024
	DefaultMaxGrepResults = 100

	// WebFetch constants
	MaxRedirects       = 5
	DefaultTimeoutSecs = 30
	MaxTimeoutSecs     = 120
	DNSCacheTTL        = 5 * time.Minute

	// Edit constants
	MinLinesForFuzzy     = 3
	LevenshteinThreshold = 0.7

	// Dispatcher constants
	PermissionChannelBuffer = 8
	QuestionChannelBuffer   = 4
	DefaultAgentName        = "default"
)
