package config

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/infrastructure/fileutil"
	"github.com/eshanized/M31A/internal/integrations/keychain"
	"github.com/fsnotify/fsnotify"
)

// ErrValidation is returned when config validation fails.
var ErrValidation = errors.New("config validation")

// DefaultConfig returns a Config with sane defaults. Missing config file
// causes Load to return DefaultConfig without error.
func DefaultConfig() *Config {
	return &Config{
		Provider: ProviderConfig{
			Default:                types.ProviderNvidia,
			FallbackPriority:       []string{types.ProviderNvidia, types.ProviderZen, types.ProviderOpenRouter},
			HealthCheckTimeoutSecs: 10,
			RegistrationOrder:      []string{types.ProviderOpenRouter, types.ProviderZen, types.ProviderNvidia},
			NvidiaBaseURL:          "https://integrate.api.nvidia.com/v1",
			FallbackMode:           "manual", // default per D-26
		},
		UI: UIConfig{
			Theme:                 "dark",
			SidebarWidthThreshold: 120,
			MaxIterations:         100,
			DiscussTimeout:        300,
			LeaderTimeoutMs:       1000,
			ThinkingMaxLines:      20,
			PermissionModalWidth:  60,
			SidebarWidth:          42,
			MaxMessageHistory:     1000,
			FallbackBannerSecs:    15,
			DefaultLogLines:       20,
			SessionListLimit:      20,
			ThinkingOpacity:       0.6,
			FrecentHistorySize:    100,
			// Layout thresholds (F-046)
			WidthUltraCompact: 40,
			WidthCompact:      60,
			WidthFull:         80,
			// Toast (F-047)
			ToastMaxVisible: 3,
			// History (F-048, F-049)
			MaxMessages:  500,
			MaxTodoItems: 50,
			// Sidebar (F-050)
			SidebarRefreshSecs: 5,
			// Welcome screen (F-081, F-082)
			WelcomeTwoColThreshold: 88,
			WelcomeCardMinWidth:    20,
			WelcomeCardMaxWidth:    60,
			// Logo (F-042)
			LogoFile: "",
			LogoText: "",
			// Welcome suggestions (F-043)
			WelcomeSuggestions: []string{},
			// Keyboard hints (F-044)
			KeyboardHints: []string{},
			// Unicode symbols (F-053)
			SymbolOverrides: map[string]string{},
			ASCIIFallback:   false,
			// Theme file (F-052)
			ThemeFile: "",
			// Toast type overrides (F-045)
			ToastTypeOverrides: map[string]ToastTypeConfig{},
		},
		Model: ModelConfig{
			Default:                 "nvidia/nemotron-3-ultra-550b-a55b",
			ContextWarningThreshold: types.ContextWarningThreshold,
			TokenEMAAlpha:           types.EMACorrectionAlpha,
			DefaultContextLength:    types.DefaultContextLength,
		},
		Features: FeaturesConfig{
			MetricsEnabled:            true,
			ModelCacheTTLMinutes:      5,
			ModelCacheStaleHours:      24,
			SessionIDLength:           types.SessionIDLength,
			MaxRecentModels:           types.DefaultMaxRecentModels,
			HealthCheckLiveMs:         types.DefaultHealthLiveMs,
			HealthCheckSlowMs:         types.DefaultHealthSlowMs,
			SessionRetentionDays:      30,
			HealthCheckTimeoutSecs:    10,
			RateLimitBackoffSecs:      120,
			PlanCheckMaxIter:          3,
			PlanChunkThreshold:        10,
			IntentClassification:      true,
			IntentClassifyTimeoutSecs: 25,
			// Enable all quality gates and checks by default
			PlanResearch:        true,
			PlanCheck:           true,
			PlanSecurityGate:    true,
			PlanCoverageGate:    true,
			PlanGapAnalysis:     true,
			PlanChunked:         true,
			DiscussQualityCheck: true,
			DiscussCompleteness: true,
			ExecutePreflight:    true,
			ExecuteQualityGate:  true,
			ExecuteLoopDetect:   true,
			VerifyReport:        true,
			ShipPreflight:       true,
			ShipChangelog:       true,
			InitDeepAnalysis:    true,
			InitPreflight:       true,
			// Workflow thresholds (F-030, F-031)
			MaxHealAttempts: types.MaxHealAttempts,
			MaxPlanRetries:  types.MaxPlanRetries,
			// Context (F-033)
			ContextTruncationThreshold: types.ContextWarningThreshold,
			// Retry policy (F-061)
			RetryMaxAttempts:       3,
			RetryBaseDelayMs:       1000,
			RetryMaxDelayMs:        30000,
			RetryBackoffMultiplier: 2.0,
			// Retry-after (F-062)
			MaxRetryAfterSecs: int(types.MaxRetryAfterWait / time.Second),
			// Task runner (F-076)
			MaxParallelTasks: types.DefaultMaxParallelTasks,
			// Coordinator (F-078)
			CoordinatorTimeoutSecs: 300,
		},
		Tools: ToolsConfig{
			MaxGlobResults:       types.DefaultMaxGlobResults,
			MaxGrepResults:       types.DefaultMaxGrepResults,
			BashKillGraceSecs:    types.DefaultBashKillGraceSecs,
			MaxBackupsPerFile:    types.DefaultMaxBackupsPerFile,
			WebfetchMaxRedirects: types.DefaultWebfetchMaxRedirects,
			WebfetchUserAgent:    "M31A/dev",
			SkipDirs:             types.SkipDirs,
			WebSearchBaseURL:     "https://search.sagibo.net",
			WebSearchEnabled:     true,
			OutputMaxLines:       types.DefaultOutputMaxLines,
			OutputMaxBytes:       types.DefaultOutputMaxBytes,
			// Rate limiting (F-018)
			RateLimitBurst:           20,
			RateLimitPerSec:          10,
			DangerousRateLimitBurst:  5,
			DangerousRateLimitPerSec: 2,
			MaxConcurrent:            8,
			// Output bounds (F-019)
			OutputRetentionDays: 7,
			// DNS (F-023)
			DnsCacheTTLSecs: 300,
			// Edit tool (F-024)
			FuzzyThreshold:   0.7,
			MinLinesForFuzzy: 3,
			// Bash (F-020)
			BashMaxTimeoutSecs: 1800,
			// WebFetch (F-021)
			WebfetchMaxRetries:   3,
			WebfetchRetryDelayMs: 500,
			// Execute phase (F-086, F-087)
			MaxToolConcurrency: 4,
			LoopDetectWindow:   3,
			// Dangerous command extensions (F-017)
			AdditionalBlockedCommands:     []string{},
			AdditionalObfuscationPatterns: []string{},
		},
		Git: GitConfig{
			CommitPrefix: "feat",
			FixPrefix:    "fix",
			ShipPrefix:   "chore",
			UserName:     "M31A",
			UserEmail:    "m31a@local",
		},
		Intelligence: IntelligenceConfig{
			ReproCommand:     "",
			BisectMaxCommits: 50,
			DepsRisk: DepsRiskConfig{
				StaleMonths:      12,
				YoungMonths:      6,
				LicenseAllowlist: DefaultLicenseAllowlist(),
			},
			DepsPolicy: DepsPolicyConfig{
				RequireApprovalHighRisk: true,
			},
		},
		Compaction: CompactionConfig{
			Auto:                true,
			Buffer:              20000,
			KeepTokens:          8000,
			Proactive:           true,
			ToolCallsThreshold:  15,
			PhaseTransitionPct:  60,
			SummaryTemplate:     "",
			SummaryTemplateFile: "",
		},
		Instructions: InstructionsConfig{
			Enabled: true,
		},
		ModelCapabilities: ModelCapabilitiesConfig{
			ExtraReasoningPatterns:      []string{},
			ExtraToolCapablePatterns:    []string{},
			ExtraCompletionOnlyPatterns: []string{},
			ExtraNonChatPatterns:        []string{},
			KnownCapabilities:           map[string]ModelCapabilityOverride{},
		},
		Prompts: PromptConfig{
			SystemPromptFile:       "",
			ProjectPromptDir:       ".m31a/prompts",
			GlobalPromptDir:        "",
			Overrides:              map[string]string{},
			ModelTemplateOverrides: map[string]string{},
		},
		Narrative: NarrativeConfig{
			TemplateOverrides:       map[string]string{},
			ClassificationOverrides: map[string]string{},
		},
		Templates: TemplateConfig{
			ExternalDir:      "",
			WebsiteFramework: "nextjs",
			CustomPalettes:   map[string]map[string]string{},
		},
		EventStore: EventStoreConfig{
			Path:                ".m31a/events.db",
			WALMode:             true,
			BusyTimeoutMs:       5000,
			BackupIntervalHours: 24,
			CheckpointInterval:  100,
			CheckpointRetention: 10,
		},
		Migration: MigrationConfig{
			PlanningDir:       ".planning",
			ArchiveOnComplete: true,
		},
		ModelProfiles: ModelProfileConfig{
			ProviderDefaults: make(map[string]types.ModelProfile),
			ModelOverrides:   make(map[string]types.ModelProfile),
		},
	}
}

// Load reads a TOML config file from the given path, applies multi-layer
// merging (global TOML → workspace TOML → env vars → project JSON), validation, and
// variable substitution, then returns the resulting Config.
//
// Config loading order (later overrides earlier):
//  1. DefaultConfig() — zero-valued defaults
//  2. Global TOML (~/.m31a/config.toml via path arg)
//  3. Workspace TOML (.m31a/workspace.toml walked up from cwd, max 3 levels)
//  4. Environment variable overrides (M31A_*)
//  5. Project-level JSON (m31a.json walked up from cwd, max 3 levels)
//  6. Variable substitution — ${VAR} → env value
//  7. Validation — type/range checks on known fields
//
// Missing global config file is not an error — first-run flow handles creation.
func Load(path string) (*Config, error) {
	// Step 1: M31A_CONFIG env var overrides path argument
	if envPath := os.Getenv("M31A_CONFIG"); envPath != "" {
		path = envPath
	}

	// Step 2: Layer 1 — Defaults (zero-valued Config)
	cfg := DefaultConfig()

	// Step 3: Layer 2 — Global config (~/.m31a/config.toml)
	if path != "" {
		meta, err := toml.DecodeFile(path, cfg)
		if err != nil {
			if !os.IsNotExist(err) {
				return nil, fmt.Errorf("decode global config %s: %w", path, err)
			}
			// Missing global config is not an error
		} else {
			// Warn on unknown TOML keys so typos like [providr] don't silently fail.
			// meta.Keys() returns fully-qualified dotted paths (e.g. "provider.default"),
			// so we check only the top-level section name against the known set.
			knownKeys := knownConfigKeys()
			for _, key := range meta.Keys() {
				k := key.String()
				topLevel := k
				if dotIdx := strings.IndexByte(k, '.'); dotIdx >= 0 {
					topLevel = k[:dotIdx]
				}
				if !knownKeys[topLevel] {
					slog.Warn("unknown config key, check for typos", "key", k, "file", path)
				}
			}
		}
	}

	// Step 3.5: Layer 3 — Workspace config (.m31a/workspace.toml)
	cwd, err := os.Getwd()
	if err != nil {
		slog.Warn("cannot determine working directory, skipping workspace config", "error", err)
	} else {
		if wsPath := findWorkspaceConfig(cwd); wsPath != "" {
			var wsCfg Config
			meta, wsErr := toml.DecodeFile(wsPath, &wsCfg)
			if wsErr != nil {
				slog.Warn("failed to decode workspace config", "path", wsPath, "error", wsErr)
			} else {
				defined := make(map[string]bool)
				for _, key := range meta.Keys() {
					defined[key.String()] = true
				}
				mergeConfig(cfg, &wsCfg, defined)
			}
		}
	}

	// Step 4: Auto-load .env file from cwd (I1)
	LoadDotEnv()

	// Step 5: Layer 5 — Project-level config (m31a.json in cwd)
	if err == nil {
		if projectPath := findProjectConfig(cwd); projectPath != "" {
			var projectCfg Config
			data, err := os.ReadFile(projectPath)
			if err != nil {
				slog.Warn("failed to read project config", "path", projectPath, "error", err)
			} else if err := json.Unmarshal(data, &projectCfg); err != nil {
				slog.Warn("failed to decode project config", "path", projectPath, "error", err)
			} else {
				// Build set of explicitly defined keys to distinguish
				// "not set" from "explicitly set to false" for bool fields.
				defined := make(map[string]bool)
				collectJSONKeys(data, "", defined)
				mergeConfig(cfg, &projectCfg, defined)
			}
		}
	}

	// Step 6: Layer 6 — Environment variable overrides (C2) - highest precedence
	if theme := os.Getenv("M31A_THEME"); theme != "" {
		cfg.UI.Theme = theme
	}
	if model := os.Getenv("M31A_DEFAULT_MODEL"); model != "" {
		cfg.Model.Default = model
	}
	if provider := os.Getenv("M31A_PROVIDER"); provider != "" {
		cfg.Provider.Default = provider
	}
	if mode := os.Getenv("M31A_PERMISSION_MODE"); mode != "" {
		cfg.Permissions.DefaultMode = mode
	}
	if compact := os.Getenv("M31A_COMPACT"); compact == "true" || compact == "1" {
		cfg.UI.CompactMode = true
	}

	// Step 6.5: Zero-value normalization for [intelligence] (Pitfall 10) —
	// TOML absence and explicit zeros both end up safe after the layered merge.
	normalizeIntelligence(&cfg.Intelligence)

	// Step 7: Variable substitution (before validation so ${VAR} in
	// enum fields like theme or permissions.default_mode resolves first)
	unresolvedVars := applyVarSubstitution(cfg)

	// Step 8: Validation
	if err := validateConfig(cfg); err != nil {
		return nil, fmt.Errorf("config validation: %w", err)
	}

	// Check for unresolved ${VAR} patterns — these are almost certainly
	// user mistakes (typo in env var name, forgot to export, etc.).
	// Log warnings but don't block startup; the unresolved patterns are
	// preserved as-is so the user can see them in the running config.
	if len(unresolvedVars) > 0 {
		for _, msg := range unresolvedVars {
			slog.Warn(msg)
		}
	}

	return cfg, nil
}

// DefaultLicenseAllowlist returns the safe default set of acceptable
// dependency licenses (D-16). Licenses outside the list classify as high risk.
func DefaultLicenseAllowlist() []string {
	return []string{"MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "MPL-2.0"}
}

// normalizeIntelligence applies zero-value normalization to the [intelligence]
// section after the layered merge (RESEARCH Pitfall 10): TOML absence and
// explicit zero values are indistinguishable post-decode, so both normalize
// to the safe defaults. Runs before validation so normalized values satisfy
// the >= 1 range checks; negative values also normalize here, keeping load
// safe-by-default while validateConfig still rejects them for callers that
// skip normalization.
func normalizeIntelligence(cfg *IntelligenceConfig) {
	if cfg.BisectMaxCommits <= 0 {
		cfg.BisectMaxCommits = 50
	}
	if cfg.DepsRisk.StaleMonths <= 0 {
		cfg.DepsRisk.StaleMonths = 12
	}
	if cfg.DepsRisk.YoungMonths <= 0 {
		cfg.DepsRisk.YoungMonths = 6
	}
	if cfg.DepsRisk.LicenseAllowlist == nil {
		cfg.DepsRisk.LicenseAllowlist = DefaultLicenseAllowlist()
	}
}

// findProjectConfig walks up from cwd (max 3 parent directories) looking for
// an m31a.json file. Returns the path if found, or "" if none exists.
// Note: this may load config from a parent project directory when running
// in a nested subdirectory of another project.
func findProjectConfig(cwd string) string {
	dir := cwd
	for i := 0; i < types.MaxProjectConfigDepth; i++ {
		candidate := filepath.Join(dir, "m31a.json")
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return ""
}

// findWorkspaceConfig walks up from cwd (max 3 parent directories) looking for
// a .m31a/workspace.toml file. Returns the path if found, or "" if none exists.
func findWorkspaceConfig(cwd string) string {
	dir := cwd
	for i := 0; i < types.MaxProjectConfigDepth; i++ {
		candidate := filepath.Join(dir, WorkspaceConfigPath)
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return ""
}

// LocalConfigPath returns the path where a new project-level m31a.json should
// be created (cwd/m31a.json).
func LocalConfigPath() (string, error) {
	cwd, err := os.Getwd()
	if err != nil {
		return "", err
	}
	return filepath.Join(cwd, "m31a.json"), nil
}

// SaveProject writes the config to a project-level m31a.json in the working
// directory. API keys are always cleared from project config for security —
// they belong in the global config or keychain.
func (c *Config) SaveProject(path string) error {
	cfgCopy := *c
	cfgCopy.Provider.OpenRouter.APIKey = ""
	cfgCopy.Provider.Zen.APIKey = ""
	cfgCopy.Provider.Nvidia.APIKey = ""

	if c.Permissions.Rules != nil {
		rulesCopy := make([]PermissionRule, len(c.Permissions.Rules))
		copy(rulesCopy, c.Permissions.Rules)
		cfgCopy.Permissions.Rules = rulesCopy
	}
	if c.Permissions.Agents != nil {
		agentsCopy := make(map[string]PermissionsAgentConfig, len(c.Permissions.Agents))
		for k, v := range c.Permissions.Agents {
			agentRulesCopy := make([]PermissionRule, len(v.Rules))
			copy(agentRulesCopy, v.Rules)
			v.Rules = agentRulesCopy
			agentsCopy[k] = v
		}
		cfgCopy.Permissions.Agents = agentsCopy
	}
	if c.Tools.SkipDirs != nil {
		skipDirsCopy := make([]string, len(c.Tools.SkipDirs))
		copy(skipDirsCopy, c.Tools.SkipDirs)
		cfgCopy.Tools.SkipDirs = skipDirsCopy
	}

	data, err := json.MarshalIndent(&cfgCopy, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal project config: %w", err)
	}
	if err := fileutil.AtomicWrite(path, data); err != nil {
		return fmt.Errorf("write project config: %w", err)
	}
	return nil
}

// mergeConfig performs a type-safe merge of overlay into base.
// Overlay non-zero values override base values. Zero-valued fields in overlay
// leave base values unchanged. Handles nested structs recursively.
// The defined set tracks which TOML keys were explicitly set, enabling
// bool fields to be overridden with false.
func mergeConfig(base, overlay *Config, defined map[string]bool) {
	MergeConfig(base, overlay, defined)
}

// toTOMLKey converts a Go field name to a TOML key (snake_case).
// Handles acronyms correctly: APIKey → api_key, BaseURL → base_url,
// HTTPSProxy → https_proxy.
// Since Go struct field names are always ASCII, iterates over bytes directly
// instead of converting to []rune.
func toTOMLKey(name string) string {
	var result []byte
	for i := 0; i < len(name); i++ {
		ch := name[i]
		if ch >= 'A' && ch <= 'Z' {
			if i > 0 {
				prev := name[i-1]
				prevIsLower := prev >= 'a' && prev <= 'z'
				prevIsDigit := prev >= '0' && prev <= '9'
				nextIsLower := i+1 < len(name) && name[i+1] >= 'a' && name[i+1] <= 'z'
				if prevIsLower || prevIsDigit || nextIsLower {
					result = append(result, '_')
				}
			}
			result = append(result, ch+32)
		} else {
			result = append(result, ch)
		}
	}
	return string(result)
}

func (c *Config) Save(path string) error {
	return c.SaveWithKeychain(path, nil)
}

// SaveWithKeychain writes the config to a TOML file atomically.
// If a keychain is provided and available, API keys are saved to the keychain
// and cleared from the config file. If the keychain is unavailable or nil,
// API keys are persisted to the config file as a fallback (with a warning).
func (c *Config) SaveWithKeychain(path string, kc keychain.Keychain) error {
	// M31A_CONFIG env var overrides path
	if envPath := os.Getenv("M31A_CONFIG"); envPath != "" {
		path = envPath
	}

	// Copy the config to avoid mutating the original
	cfgCopy := *c

	// H-12: Deep-copy slice and map fields to prevent data races with WatchConfig
	if c.Permissions.Rules != nil {
		rulesCopy := make([]PermissionRule, len(c.Permissions.Rules))
		copy(rulesCopy, c.Permissions.Rules)
		cfgCopy.Permissions.Rules = rulesCopy
	}
	if c.Permissions.Agents != nil {
		agentsCopy := make(map[string]PermissionsAgentConfig, len(c.Permissions.Agents))
		for k, v := range c.Permissions.Agents {
			agentRulesCopy := make([]PermissionRule, len(v.Rules))
			copy(agentRulesCopy, v.Rules)
			v.Rules = agentRulesCopy
			agentsCopy[k] = v
		}
		cfgCopy.Permissions.Agents = agentsCopy
	}
	if c.Tools.SkipDirs != nil {
		skipDirsCopy := make([]string, len(c.Tools.SkipDirs))
		copy(skipDirsCopy, c.Tools.SkipDirs)
		cfgCopy.Tools.SkipDirs = skipDirsCopy
	}

	// Determine if we should persist API keys to the config file.
	// We persist keys if:
	// 1. No keychain is provided, OR
	// 2. Keychain is provided but unavailable (ErrKeychainUnavailable)
	// We clear keys from the config file only if keychain is available and saves succeed.
	persistKeys := true
	if kc != nil {
		// Try to save keys to keychain to test availability
		openRouterKey := c.Provider.OpenRouter.APIKey
		zenKey := c.Provider.Zen.APIKey
		nvidiaKey := c.Provider.Nvidia.APIKey
		openRouterSaved := openRouterKey == ""
		zenSaved := zenKey == ""
		nvidiaSaved := nvidiaKey == ""

		if openRouterKey != "" {
			if err := kc.Set(types.ProviderOpenRouter, openRouterKey); err == nil {
				openRouterSaved = true
			} else if errors.Is(err, keychain.ErrKeychainUnavailable) {
				// Keychain unavailable - will persist to config file
			} else {
				slog.Warn("failed to save OpenRouter key to keychain", "error", err)
			}
		}
		if zenKey != "" {
			if err := kc.Set(types.ProviderZen, zenKey); err == nil {
				zenSaved = true
			} else if errors.Is(err, keychain.ErrKeychainUnavailable) {
				// Keychain unavailable - will persist to config file
			} else {
				slog.Warn("failed to save Zen key to keychain", "error", err)
			}
		}
		if nvidiaKey != "" {
			if err := kc.Set(types.ProviderNvidia, nvidiaKey); err == nil {
				nvidiaSaved = true
			} else if errors.Is(err, keychain.ErrKeychainUnavailable) {
				// Keychain unavailable - will persist to config file
			} else {
				slog.Warn("failed to save NVIDIA key to keychain", "error", err)
			}
		}

		// Clear keys from config file only for providers that were successfully
		// saved to keychain. Providers that failed keychain storage keep their
		// keys in the config file as fallback.
		if openRouterSaved {
			cfgCopy.Provider.OpenRouter.APIKey = ""
		}
		if zenSaved {
			cfgCopy.Provider.Zen.APIKey = ""
		}
		if nvidiaSaved {
			cfgCopy.Provider.Nvidia.APIKey = ""
		}
		if openRouterSaved && zenSaved && nvidiaSaved {
			persistKeys = false
		} else {
			slog.Warn("keychain unavailable or save failed for some providers; persisting API keys to config file as fallback")
		}
	}

	if persistKeys {
		// Keys will be persisted to config file as fallback
		slog.Info("API keys will be stored in config file (keychain unavailable)")
	}

	data, err := toml.Marshal(&cfgCopy)
	if err != nil {
		return fmt.Errorf("marshal config: %w", err)
	}

	// Ensure parent directory exists
	if err := os.MkdirAll(filepath.Dir(path), types.DirPermission); err != nil {
		return fmt.Errorf("create config dir: %w", err)
	}

	// Atomic write: write to temp file in same directory, then rename
	if err := fileutil.AtomicWrite(path, data); err != nil {
		return fmt.Errorf("write config: %w", err)
	}

	return nil
}

// ResolveAPIKeys resolves API keys for OpenRouter and Zen using the priority
// order: environment variable → OS keychain → config file field.
//
// Resolution per provider:
//  1. Check M31A_OPENROUTER_API_KEY / M31A_ZEN_API_KEY env var
//  2. If not set, try keychain.Get("openrouter") / keychain.Get("zen")
//  3. If keychain returns ErrKeyNotFound or ErrKeychainUnavailable,
//     fall back to c.Provider.OpenRouter.APIKey / c.Provider.Zen.APIKey
//
// Never returns an error for missing keys — keys may be left empty for
// first-run or settings screen to handle.
func (c *Config) ResolveAPIKeys(kc keychain.Keychain) error {
	// OpenRouter
	if key := os.Getenv("M31A_OPENROUTER_API_KEY"); key != "" {
		c.Provider.OpenRouter.APIKey = key
	} else if key := os.Getenv("OPENROUTER_API_KEY"); key != "" {
		c.Provider.OpenRouter.APIKey = key
	} else if kc != nil {
		if k, err := kc.Get(types.ProviderOpenRouter); err == nil {
			c.Provider.OpenRouter.APIKey = k
		} else if !errors.Is(err, keychain.ErrKeyNotFound) && !errors.Is(err, keychain.ErrKeychainUnavailable) {
			// Unexpected error — log and continue
			slog.Warn("keychain error", "provider", types.ProviderOpenRouter, "error", err)
		}
		// If keychain returns ErrKeyNotFound or ErrKeychainUnavailable,
		// keep the value from config file (already loaded in c.Provider.OpenRouter.APIKey)
	}

	// Zen
	if key := os.Getenv("M31A_ZEN_API_KEY"); key != "" {
		c.Provider.Zen.APIKey = key
	} else if key := os.Getenv("ZEN_API_KEY"); key != "" {
		c.Provider.Zen.APIKey = key
	} else if kc != nil {
		if k, err := kc.Get(types.ProviderZen); err == nil {
			c.Provider.Zen.APIKey = k
		} else if !errors.Is(err, keychain.ErrKeyNotFound) && !errors.Is(err, keychain.ErrKeychainUnavailable) {
			slog.Warn("keychain error", "provider", types.ProviderZen, "error", err)
		}
	}

	// NVIDIA NIM
	if key := os.Getenv("M31A_NVIDIA_API_KEY"); key != "" {
		c.Provider.Nvidia.APIKey = key
	} else if key := os.Getenv("NVIDIA_API_KEY"); key != "" {
		c.Provider.Nvidia.APIKey = key
	} else if kc != nil {
		if k, err := kc.Get(types.ProviderNvidia); err == nil {
			c.Provider.Nvidia.APIKey = k
		} else if !errors.Is(err, keychain.ErrKeyNotFound) && !errors.Is(err, keychain.ErrKeychainUnavailable) {
			slog.Warn("keychain error", "provider", types.ProviderNvidia, "error", err)
		}
	}

	return nil
}

// ConfigReloadMsg is emitted when the config file changes on disk.
type ConfigReloadMsg struct {
	Config *Config
	Error  error
}

// sendReload loads the config and sends it on ch, retrying briefly if the
// receiver is busy. Blocks until delivered or ctx is cancelled — never drops
// the message silently (BUG-18).
func sendReload(ctx context.Context, ch chan<- ConfigReloadMsg, path string) {
	cfg, err := Load(path)
	msg := ConfigReloadMsg{Config: cfg, Error: err}
	select {
	case ch <- msg:
		return
	case <-ctx.Done():
		return
	case <-time.After(100 * time.Millisecond):
		// Receiver didn't take it within 100ms — block until it does or
		// the watcher is shut down. No silent drop.
		select {
		case ch <- msg:
		case <-ctx.Done():
			slog.Warn("config reload message not delivered: watcher cancelled")
		}
	}
}

// WatchConfig watches the config file for changes using fsnotify and sends
// ConfigReloadMsg to the provided channel when a change is detected.
// Falls back to polling with ConfigWatchInterval if fsnotify is unavailable.
// Runs until ctx is cancelled.
func WatchConfig(ctx context.Context, path string, ch chan<- ConfigReloadMsg) {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		slog.Warn("fsnotify unavailable, falling back to config polling", "error", err)
		watchConfigPolling(ctx, path, ch)
		return
	}
	defer func() {
		if err := watcher.Close(); err != nil {
			slog.Debug("close file watcher", "error", err, "path", path)
		}
	}()

	dir := filepath.Dir(path)
	base := filepath.Base(path)
	if err := watcher.Add(dir); err != nil {
		slog.Warn("fsnotify watch failed, falling back to config polling", "error", err)
		watchConfigPolling(ctx, path, ch)
		return
	}

	var debounce *time.Timer
	for {
		select {
		case <-ctx.Done():
			if debounce != nil {
				debounce.Stop()
			}
			return
		case event, ok := <-watcher.Events:
			if !ok {
				return
			}
			if filepath.Base(event.Name) != base {
				continue
			}
			if event.Op&(fsnotify.Write|fsnotify.Create|fsnotify.Rename) == 0 {
				continue
			}
			if debounce != nil {
				debounce.Stop()
			}
			debounce = time.AfterFunc(50*time.Millisecond, func() {
				sendReload(ctx, ch, path)
			})
		case err, ok := <-watcher.Errors:
			if !ok {
				return
			}
			slog.Warn("config watcher error", "error", err)
		}
	}
}

// watchConfigPolling is the fallback config watcher that polls the file's
// modification time every ConfigWatchInterval.
func watchConfigPolling(ctx context.Context, path string, ch chan<- ConfigReloadMsg) {
	var lastModTime time.Time
	if info, err := os.Stat(path); err == nil {
		lastModTime = info.ModTime()
	}
	ticker := time.NewTicker(types.ConfigWatchInterval)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			info, err := os.Stat(path)
			if err != nil {
				continue
			}
			if info.ModTime().After(lastModTime) {
				lastModTime = info.ModTime()
				sendReload(ctx, ch, path)
			}
		}
	}
}

// DefaultGitConfig returns the default git configuration.
func DefaultGitConfig() GitConfig {
	return GitConfig{
		CommitPrefix: "feat",
		FixPrefix:    "fix",
		ShipPrefix:   "chore",
		UserName:     "M31A",
		UserEmail:    "m31a@local",
	}
}

// LoadDotEnv reads a .env file from the current working directory and sets
// environment variables. Does not override already-set variables.
//
// This must be called before any goroutines that read os.Environ() are started
// (e.g., before logger initialization) because os.Setenv is not goroutine-safe.
// Guarded with sync.Once to prevent duplicate calls from main.go and Load().
var loadDotEnvOnce sync.Once

func LoadDotEnv() {
	loadDotEnvOnce.Do(func() {
		cwd, err := os.Getwd()
		if err != nil {
			return
		}
		envPath := filepath.Join(cwd, ".env")
		info, err := os.Stat(envPath)
		if err != nil {
			return
		}
		if info.Mode().Perm()&0o022 != 0 {
			slog.Warn("skipping group/world-writable .env file", "path", envPath)
			return
		}
		data, err := os.ReadFile(envPath)
		if err != nil {
			return
		}
		for _, line := range strings.Split(string(data), "\n") {
			line = strings.TrimSpace(line)
			if len(line) > 4096 {
				slog.Warn("skipping overly long .env line", "path", envPath)
				continue
			}
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			parts := strings.SplitN(line, "=", 2)
			if len(parts) != 2 {
				continue
			}
			key := strings.TrimSpace(parts[0])
			value := strings.TrimSpace(parts[1])
			// Remove surrounding quotes
			if len(value) >= 2 {
				if (value[0] == '"' && value[len(value)-1] == '"') ||
					(value[0] == '\'' && value[len(value)-1] == '\'') {
					value = value[1 : len(value)-1]
				}
			}
			if _, exists := os.LookupEnv(key); !exists {
				_ = os.Setenv(key, value)
			}
		}
	})
}

// collectJSONKeys recursively traverses a JSON object and populates the
// defined map with fully-qualified dotted paths (e.g., "extensions.tools.mytool.command").
// This enables bool fields in JSON config to be explicitly set to false.
func collectJSONKeys(data []byte, prefix string, defined map[string]bool) {
	var obj map[string]interface{}
	if err := json.Unmarshal(data, &obj); err != nil {
		return
	}
	for k, v := range obj {
		fullKey := k
		if prefix != "" {
			fullKey = prefix + "." + k
		}
		defined[fullKey] = true
		switch val := v.(type) {
		case map[string]interface{}:
			nestedData, _ := json.Marshal(val)
			collectJSONKeys(nestedData, fullKey, defined)
		case []interface{}:
			// For arrays, we don't track individual indices since mergeConfig
			// replaces entire slices. Just mark the array key as defined.
		}
	}
}
