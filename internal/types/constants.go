package types

import (
	pkgTypes "github.com/eshanized/M31A/pkg/types"
)

// Constants — re-exported from pkg/types for backward compatibility.

const (
	ModelCacheTTL            = pkgTypes.ModelCacheTTL
	HealthCheckInterval      = pkgTypes.HealthCheckInterval
	MaxFileSize              = pkgTypes.MaxFileSize
	MaxToolOutputChars       = pkgTypes.MaxToolOutputChars
	MaxHealAttempts          = pkgTypes.MaxHealAttempts
	MaxPlanRetries           = pkgTypes.MaxPlanRetries
	MaxPlanRefinements       = pkgTypes.MaxPlanRefinements
	SessionIDLength          = pkgTypes.SessionIDLength
	AutoDreamThreshold       = pkgTypes.AutoDreamThreshold
	ContextWarningThreshold  = pkgTypes.ContextWarningThreshold
	HTTPDialTimeout          = pkgTypes.HTTPDialTimeout
	BashTimeout              = pkgTypes.BashTimeout
	BashOutputLimit          = pkgTypes.BashOutputLimit
	DefaultContextLength     = pkgTypes.DefaultContextLength
	MaxLLMResponseBytes      = pkgTypes.MaxLLMResponseBytes
	MaxSessionFileSize       = pkgTypes.MaxSessionFileSize
	DefaultPermissionTimeout = pkgTypes.DefaultPermissionTimeout
	StaleCacheTTL            = pkgTypes.StaleCacheTTL
	DefaultHealthLiveMs      = pkgTypes.DefaultHealthLiveMs
	DefaultHealthSlowMs      = pkgTypes.DefaultHealthSlowMs

	HealthStatusLive     = pkgTypes.HealthStatusLive
	HealthStatusSlow     = pkgTypes.HealthStatusSlow
	HealthStatusOffline  = pkgTypes.HealthStatusOffline
	HealthStatusDegraded = pkgTypes.HealthStatusDegraded

	MaxProviderErrorChars = pkgTypes.MaxProviderErrorChars

	DefaultOpenRouterBaseURL = pkgTypes.DefaultOpenRouterBaseURL
	DefaultZenBaseURL        = pkgTypes.DefaultZenBaseURL
	DefaultNvidiaBaseURL     = pkgTypes.DefaultNvidiaBaseURL
	DefaultReferer           = pkgTypes.DefaultReferer
	DefaultMaxRecentModels   = pkgTypes.DefaultMaxRecentModels

	DirPermission  = pkgTypes.DirPermission
	FilePermission = pkgTypes.FilePermission

	DefaultMaxGlobResults       = pkgTypes.DefaultMaxGlobResults
	DefaultMaxGrepResults       = pkgTypes.DefaultMaxGrepResults
	DefaultBashKillGraceSecs    = pkgTypes.DefaultBashKillGraceSecs
	DefaultMaxBackupsPerFile    = pkgTypes.DefaultMaxBackupsPerFile
	DefaultWebfetchMaxRedirects = pkgTypes.DefaultWebfetchMaxRedirects

	DefaultOutputMaxLines = pkgTypes.DefaultOutputMaxLines
	DefaultOutputMaxBytes = pkgTypes.DefaultOutputMaxBytes

	CompressCooldown   = pkgTypes.CompressCooldown
	ChannelSendTimeout = pkgTypes.ChannelSendTimeout
	ToastDuration      = pkgTypes.ToastDuration

	FetchModelsTimeout    = pkgTypes.FetchModelsTimeout
	HealthCheckRetryDelay = pkgTypes.HealthCheckRetryDelay
	MaxRetryAfterWait     = pkgTypes.MaxRetryAfterWait

	EMACorrectionAlpha = pkgTypes.EMACorrectionAlpha

	MaxToolsPerCall = pkgTypes.MaxToolsPerCall
	MaxCwdFileDepth = pkgTypes.MaxCwdFileDepth

	ConfigWatchInterval   = pkgTypes.ConfigWatchInterval
	MaxProjectConfigDepth = pkgTypes.MaxProjectConfigDepth

	DefaultSessionCacheTTL = pkgTypes.DefaultSessionCacheTTL

	DefaultVerifyTimeout = pkgTypes.DefaultVerifyTimeout

	DefaultFetchModelsTimeout = pkgTypes.DefaultFetchModelsTimeout

	DefaultUserAgent = pkgTypes.DefaultUserAgent
	DefaultXTitle    = pkgTypes.DefaultXTitle
	DateFormat       = pkgTypes.DateFormat
	DateTimeFormat   = pkgTypes.DateTimeFormat

	DefaultMaxParallelTasks = pkgTypes.DefaultMaxParallelTasks
)

// SkipDirs — re-exported from pkg/types for backward compatibility.
var SkipDirs = pkgTypes.SkipDirs

// SkipDirsMap returns a cached map for O(1) lookup of skip directories.
func SkipDirsMap() map[string]bool {
	return pkgTypes.SkipDirsMap()
}
