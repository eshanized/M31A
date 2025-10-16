package main

import (
	"flag"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"runtime"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/log"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/provider/openrouter"
	"github.com/eshanized/M31A/internal/provider/zen"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/autodream"
	"github.com/eshanized/M31A/pkg/keychain"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/rollback"
	"github.com/eshanized/M31A/pkg/session"
)

var Version = "dev"

func main() {
	// Build command registry for usage and TUI
	cmdRegistry := tui.DefaultCommands()

	// Parse CLI flags
	versionFlag := flag.Bool("version", false, "Print version and exit")
	helpFlag := flag.Bool("help", false, "Show usage information")
	flag.Usage = func() {
		printUsage(cmdRegistry)
	}
	flag.Parse()
	if *helpFlag {
		flag.Usage()
		os.Exit(0)
	}
	if *versionFlag {
		fmt.Printf("m31a %s %s/%s (Go %s)\n", Version, runtime.GOOS, runtime.GOARCH, runtime.Version())
		os.Exit(0)
	}

	logger, cleanup, err := log.NewLogger(Version)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to initialize logger: %v\n", err)
		os.Exit(1)
	}
	defer cleanup()
	slog.SetDefault(logger)

	logger.Info("M31A starting",
		"version", Version,
		"go_version", runtime.Version(),
		"os", runtime.GOOS,
		"arch", runtime.GOARCH,
	)

	// Resolve config path
	configPath := os.Getenv("M31A_CONFIG")
	if configPath == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			logger.Error("cannot determine home directory", "error", err)
			os.Exit(1)
		}
		configPath = filepath.Join(home, ".m31a", "config.toml")
	}

	if err := os.MkdirAll(filepath.Dir(configPath), types.DirPermission); err != nil {
		logger.Error("cannot create config directory", "error", err)
		os.Exit(1)
	}

	cfg, err := config.Load(configPath)
	if err != nil {
		logger.Error("failed to load config", "error", err)
		os.Exit(1)
	}
	if cfg == nil {
		cfg = config.DefaultConfig()
	}

	// Keychain
	kc, kcErr := keychain.New()
	if kcErr != nil {
		logger.Warn("keychain initialization failed", "error", kcErr)
	}
	if kc != nil {
		if err := cfg.ResolveAPIKeys(kc); err != nil {
			logger.Warn("failed to resolve API keys", "error", err)
		}
	}

	// Provider registry
	registry := provider.NewRegistry()

	if cfg.Provider.OpenRouter.APIKey != "" {
		cacheTTL := types.ModelCacheTTL
		if cfg.Features.ModelCacheTTLMinutes > 0 {
			cacheTTL = time.Duration(cfg.Features.ModelCacheTTLMinutes) * time.Minute
		}
		cacheStaleTTL := types.StaleCacheTTL
		if cfg.Features.ModelCacheStaleHours > 0 {
			cacheStaleTTL = time.Duration(cfg.Features.ModelCacheStaleHours) * time.Hour
		}
		orClient, err := openrouter.New(cfg.Provider.OpenRouter.APIKey, openrouter.Options{
			BaseURL:           cfg.Provider.OpenRouterBaseURL,
			CacheTTL:          cacheTTL,
			CacheStaleTTL:     cacheStaleTTL,
			Referer:           cfg.Provider.OpenRouterReferer,
			Title:             cfg.Provider.OpenRouterTitle,
			HealthCheckLiveMs: int64(cfg.Features.HealthCheckLiveMs),
			HealthCheckSlowMs: int64(cfg.Features.HealthCheckSlowMs),
		})
		if err != nil {
			logger.Warn("failed to create OpenRouter client", "error", err)
		} else {
			if err := registry.Register("openrouter", orClient); err != nil {
				logger.Warn("failed to register OpenRouter provider", "error", err)
			} else {
				logger.Info("OpenRouter provider registered")
			}
		}
	}

	if cfg.Provider.Zen.APIKey != "" {
		cacheTTL := types.ModelCacheTTL
		if cfg.Features.ModelCacheTTLMinutes > 0 {
			cacheTTL = time.Duration(cfg.Features.ModelCacheTTLMinutes) * time.Minute
		}
		cacheStaleTTL := types.StaleCacheTTL
		if cfg.Features.ModelCacheStaleHours > 0 {
			cacheStaleTTL = time.Duration(cfg.Features.ModelCacheStaleHours) * time.Hour
		}
		zenClient, err := zen.New(cfg.Provider.Zen.APIKey, zen.Options{
			BaseURL:           cfg.Provider.ZenBaseURL,
			CacheTTL:          cacheTTL,
			CacheStaleTTL:     cacheStaleTTL,
			HealthCheckLiveMs: int64(cfg.Features.HealthCheckLiveMs),
			HealthCheckSlowMs: int64(cfg.Features.HealthCheckSlowMs),
			DefaultContextLen: int64(cfg.Model.DefaultContextLength),
		})
		if err != nil {
			logger.Warn("failed to create Zen client", "error", err)
		} else {
			if err := registry.Register("zen", zenClient); err != nil {
				logger.Warn("failed to register Zen provider", "error", err)
			} else {
				logger.Info("Zen provider registered")
			}
		}
	}

	if cfg.Provider.Default != "" {
		if err := registry.SetActive(cfg.Provider.Default); err != nil {
			logger.Warn("configured default provider not registered", "default", cfg.Provider.Default, "error", err)
		}
	}
	if registry.Active() == "" {
		logger.Warn("no active provider — TUI will start without LLM access")
	}

	// Session manager
	sessionsDir := filepath.Join(filepath.Dir(configPath), "sessions")
	sessionMgr := session.NewManager(sessionsDir, session.ManagerOpts{})

	// Tools dispatcher
	workDir, _ := os.Getwd()
	backupDir := filepath.Join(filepath.Dir(configPath), "backups")
	dispatcher, err := tools.DefaultDispatcher(workDir, backupDir, sessionsDir, &cfg.Permissions)
	if err != nil {
		logger.Warn("failed to create tools dispatcher", "error", err)
		dispatcher, _ = tools.DefaultDispatcher(workDir, backupDir, sessionsDir, nil)
	}

	// Git client
	gitClient := git.New(workDir)

	// Ledger
	ledgerPath := filepath.Join(filepath.Dir(configPath), "LEDGER.md")
	ledgerClient := ledger.New(ledgerPath)

	// Rollback
	rollbackClient := rollback.New(gitClient)

	// AutoDream (starts with empty messages; REPL injects messages later)
	autoDreamClient := autodream.New(nil)

	// Theme
	themeMode := theme.ModeDark
	switch cfg.UI.Theme {
	case "light":
		themeMode = theme.ModeLight
	case "auto":
		themeMode = theme.ModeAuto
	}

	// Build and launch TUI app
	app := tui.NewApp(
		cfg,
		configPath,
		registry,
		sessionMgr,
		dispatcher,
		gitClient,
		ledgerClient,
		rollbackClient,
		autoDreamClient,
		Version,
		themeMode,
	)

	p := tea.NewProgram(app, tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		logger.Error("TUI exited with error", "error", err)
		os.Exit(1)
	}

	app.Shutdown()
}
