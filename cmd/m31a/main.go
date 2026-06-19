package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"path/filepath"
	"runtime"
	"strings"
	"syscall"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/log"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tools/subagent"
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
	os.Exit(run())
}

// run contains all application logic so that deferred cleanup functions
// execute before os.Exit. Returns the exit code (0 for success, 1 for error).
func run() int {
	// Build command registry for usage and TUI
	cmdRegistry := tui.DefaultCommands()

	// Parse CLI flags
	versionFlag := flag.Bool("version", false, "Print version and exit")
	helpFlag := flag.Bool("help", false, "Show usage information")
	flag.Usage = func() {
		printUsage(cmdRegistry)
	}
	flag.Parse()
	if *helpFlag || len(flag.Args()) > 0 && flag.Arg(0) == "help" {
		flag.Usage()
		return 0
	}
	if *versionFlag {
		// Normalise runtime.Version() to remove build-tag noise (e.g. "X:nodwarf5")
		goVer := runtime.Version()
		if idx := strings.Index(goVer, ":"); idx != -1 {
			goVer = goVer[:idx]
		}
		fmt.Printf("m31a %s %s/%s (Go %s)\n", Version, runtime.GOOS, runtime.GOARCH, goVer)
		return 0
	}

	// Load .env before logger to avoid goroutine race on os.Setenv (§2.3)
	config.LoadDotEnv()

	logger, cleanup, err := log.NewLogger(Version)
	if err != nil {
		// Logger not yet initialized; use stderr as fallback.
		fmt.Fprintf(os.Stderr, "failed to initialize logger: %v\n", err)
		return 1
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
		home, homeErr := os.UserHomeDir()
		if homeErr != nil {
			logger.Error("cannot determine home directory", "error", homeErr)
			return 1
		}
		configPath = filepath.Join(home, ".m31a", "config.toml")
	}

	if mkdirErr := os.MkdirAll(filepath.Dir(configPath), types.DirPermission); mkdirErr != nil {
		logger.Error("cannot create config directory", "error", mkdirErr)
		return 1
	}

	cfg, err := config.Load(configPath)
	if err != nil {
		logger.Error("failed to load config", "error", err)
		return 1
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
		if resolveErr := cfg.ResolveAPIKeys(kc); resolveErr != nil {
			logger.Warn("failed to resolve API keys", "error", resolveErr)
		}
	}

	// Provider registry
	tools.SetVersion(Version)
	registry := provider.NewRegistry()

	if cfg.Provider.OpenRouter.APIKey != "" {
		if regErr := tui.RegisterProvider(registry, cfg, "openrouter", cfg.Provider.OpenRouter.APIKey, Version); regErr != nil {
			logger.Warn("failed to register OpenRouter provider", "error", regErr)
		}
	}

	if cfg.Provider.Zen.APIKey != "" {
		if regErr := tui.RegisterProvider(registry, cfg, "zen", cfg.Provider.Zen.APIKey, Version); regErr != nil {
			logger.Warn("failed to register Zen provider", "error", regErr)
		}
	}

	if cfg.Provider.Nvidia.APIKey != "" {
		if regErr := tui.RegisterProvider(registry, cfg, "nvidia", cfg.Provider.Nvidia.APIKey, Version); regErr != nil {
			logger.Warn("failed to register NVIDIA provider", "error", regErr)
		}
	}

	if cfg.Provider.Default != "" {
		if setErr := registry.SetActive(cfg.Provider.Default); setErr != nil {
			logger.Warn("configured default provider not registered", "default", cfg.Provider.Default, "error", setErr)
		}
	}

	hasProvider := registry.Active() != ""
	if !hasProvider {
		logger.Warn("no active provider configured — LLM features will be unavailable")
	}

	// Working directory — fail fast if Getwd fails (WP-C03)
	workDir, err := os.Getwd()
	if err != nil {
		logger.Error("cannot determine working directory", "error", err)
		return 1
	}

	// Session manager — project-local sessions in <workDir>/.m31a/
	globalConfigDir := filepath.Dir(configPath)
	sessionMgr := session.NewManager(globalConfigDir, workDir, session.ManagerOpts{})

	// Tools dispatcher — fail fast on permission config errors (WP-C04)
	backupDir := filepath.Join(workDir, ".m31a", "backups")
	dispatcher, err := tools.DefaultDispatcher(workDir, backupDir, backupDir, &cfg.Permissions)
	if err != nil {
		logger.Error("failed to create tools dispatcher — permission configuration is invalid", "error", err)
		return 1
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
	if cfg != nil {
		switch cfg.UI.Theme {
		case "light":
			themeMode = theme.ModeLight
		case "auto":
			themeMode = theme.ModeAuto
		}
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
	app.SetCwd(workDir)

	if kc != nil {
		app.SetKeychain(kc)
	}

	// Parallel subagents: manager + dispatcher integration.
	// Each subagent gets its own dispatcher + worktree; the parent dispatcher
	// exposes the Agent tool so the LLM can spawn children directly.
	var activeModelForSubagents *types.ModelInfo
	if cfg != nil && cfg.Model.Default != "" && registry != nil {
		if p := registry.ActiveProvider(); p != nil {
			if info, err := p.GetModel(cfg.Model.Default); err == nil {
				activeModelForSubagents = info
			}
		}
		if activeModelForSubagents == nil {
			activeModelForSubagents = &types.ModelInfo{ID: cfg.Model.Default}
		}
	}
	subagentMgr := subagent.NewManager(subagent.Dependencies{
		WorkDir:     workDir,
		Registry:    registry,
		ActiveModel: activeModelForSubagents,
		Logger:      logger,
		Worktrees:   &subagent.GitWorktrees{},
		NewDispatcher: tools.NewDispatcherFactory(
			backupDir, backupDir, &cfg.Permissions, nil,
		),
	})
	// Register the Agent tool on the parent dispatcher (non-child so it can
	// spawn in background). The factory passes nil for the child-side manager
	// reference to avoid a registration cycle; children created by the
	// factory get isChild=true and cannot spawn grandchildren in background.
	if err := dispatcher.Register(tools.NewAgent(subagentMgr, false, 0)); err != nil {
		logger.Error("failed to register Agent tool on parent dispatcher", "error", err)
		return 1
	}
	app.SetSubagentManager(subagentMgr)

	// Best-effort sweep of stale agent worktrees/branches from prior crashes.
	if err := subagent.Sweep(context.Background(), workDir); err != nil {
		logger.Warn("subagent worktree sweep failed", "error", err)
	}

	// Resume on startup (C3): if configured, auto-resume the most recent session
	if cfg.Features.ResumeOnStartup {
		sessions, err := sessionMgr.ListSessions()
		if err == nil && len(sessions) > 0 {
			app.SetResumeSessionID(sessions[0].ID)
		}
	}

	p := tea.NewProgram(app,
		tea.WithAltScreen(),
		tea.WithMouseAllMotion(),
	)

	// Signal handler sends tea.Quit through the program channel
	// instead of calling app.Shutdown() directly from a goroutine.
	// This ensures all state mutations happen inside Update(), preserving
	// Bubble Tea's single-threaded contract and preventing session corruption.
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
	sigDone := make(chan struct{})
	go func() {
		defer func() {
			if r := recover(); r != nil {
				slog.Error("signal handler panic", "error", r)
			}
		}()
		for {
			select {
			case <-sigCh:
				slog.Info("received shutdown signal, sending quit to TUI...")
				p.Send(tea.QuitMsg{})
				// H-24: Hard fallback — force exit after 5 seconds if TUI doesn't quit.
				// Write a sentinel file so run() can detect an unclean shutdown
				// and clean up stale session state on next launch.
				select {
				case <-sigDone:
					return
				case <-time.After(5 * time.Second):
					slog.Warn("TUI did not exit within timeout, forcing exit")
					sentinel := filepath.Join(filepath.Dir(configPath), ".force-exit")
					_ = os.WriteFile(sentinel, []byte("force-exit"), 0o644)
					cleanup()
					os.Exit(1)
				}
			case <-sigDone:
				return
			}
		}
	}()

	if _, err := p.Run(); err != nil {
		logger.Error("TUI exited with error", "error", err)
		close(sigDone)
		return 1
	}

	// Cancel the signal goroutine before Shutdown to prevent the 5-second
	// os.Exit(1) timer from racing with cleanup.
	close(sigDone)
	app.Shutdown()
	return 0
}
