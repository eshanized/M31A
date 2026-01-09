# Changelog

All notable changes to M31A will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed
- **WP-C01**: Signal handler race condition — sends tea.QuitMsg through program channel instead of calling app.Shutdown() from goroutine
- **WP-C02**: Added warning when no provider is active at startup
- **WP-C03**: Fail fast on os.Getwd() error instead of silently discarding
- **WP-C04**: Fail fast on permission config error instead of falling back to nil permissions
- **WP-C05**: Guard autoDreamClient against premature /compress calls
- **WP-H01**: Removed bash blacklist (trivially bypassable substring matching)
- **WP-H02**: Added path traversal validation for Python compile step in verify phase
- **WP-H03**: Replaced time.Sleep with cancellable timer in taskrunner retry loop
- **WP-H04**: Enforced MaxLLMResponseBytes in consumeStream
- **WP-H05**: Added size limits (50MB) to all session file reads via readFileLimited()
- **WP-H07**: Extracted main logic into run() function for proper defer cleanup
- **WP-H08**: All main.go error output now goes through structured logger
- **WP-M02**: Replaced context.Background() with shutdownCtx in TUI commands
- **WP-M03**: Removed duplicated DateTimeFormat constant between types and tools
- **WP-M05**: Removed ResponseHeaderTimeout from streaming HTTP clients (already absent)
- **WP-M09**: Removed init() function in webfetch.go
- **WP-S02**: Improved IPv6 SSRF protection using net.IP.IsPrivate()
- **WP-L01**: Replaced all interface{} with any
- **WP-L06**: Removed 46 stale bug ID references from comments across 18 source files
- **WP-L07**: Removed nolint:gochecknoglobals directive

### Added
- **WP-A03**: GitClient interface in internal/types/ for testability
- **WP-D02**: Package-level doc.go files for all 7 public packages
- **WP-D03**: This CHANGELOG.md
- **WP-M08**: Documented all unsafe.Pointer usages in Windows keychain
- **WP-P02**: Configurable session list cache TTL via ManagerOpts
- **WP-S01**: Comprehensive test suite for API key redaction (14 subtests)
- **WP-S03**: Expanded pass CLI regex to allow digits and hyphens
- **WP-L02**: Centralized timeout constants (DefaultSessionCacheTTL, DefaultVerifyTimeout, DefaultFetchModelsTimeout)
- **WP-L08**: Added documentation to firstrunpreview dev tool

### Changed
- **WP-D01**: Updated AGENTS.md package layout to reflect main.go reality
