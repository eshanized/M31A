package main

import (
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"path/filepath"
	"runtime"
	"syscall"

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
	if *helpFlag {
		flag.Usage()
		return 0
	}
	if *versionFlag {
		fmt.Printf("m31a %s %s/%s (Go %s)\n", Version, runtime.GOOS, runtime.GOARCH, runtime.Version())
		return 0
	}

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
		home, err := os.UserHomeDir()
		if err != nil {
			logger.Error("cannot determine home directory", "error", err)
			return 1
		}
		configPath = filepath.Join(home, ".m31a", "config.toml")
	}

	if err := os.MkdirAll(filepath.Dir(configPath), types.DirPermission); err != nil {
		logger.Error("cannot create config directory", "error", err)
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
		if err := cfg.ResolveAPIKeys(kc); err != nil {
			logger.Warn("failed to resolve API keys", "error", err)
		}
	}

	// Provider registry
	openrouter.Version = Version
	zen.Version = Version
	tools.SetVersion(Version)
	registry := provider.NewRegistry()

	if cfg.Provider.OpenRouter.APIKey != "" {
		if err := tui.RegisterProvider(registry, cfg, "openrouter", cfg.Provider.OpenRouter.APIKey); err != nil {
			logger.Warn("failed to register OpenRouter provider", "error", err)
		}
	}

	if cfg.Provider.Zen.APIKey != "" {
		if err := tui.RegisterProvider(registry, cfg, "zen", cfg.Provider.Zen.APIKey); err != nil {
			logger.Warn("failed to register Zen provider", "error", err)
		}
	}

	if cfg.Provider.Default != "" {
		if err := registry.SetActive(cfg.Provider.Default); err != nil {
			logger.Warn("configured default provider not registered", "default", cfg.Provider.Default, "error", err)
		}
	}

	hasProvider := registry.Active() != ""
	if !hasProvider {
		logger.Warn("no active provider configured — LLM features will be unavailable")
	}

	// Session manager
	sessionsDir := filepath.Join(filepath.Dir(configPath), "sessions")
	sessionMgr := session.NewManager(sessionsDir, session.ManagerOpts{})

	// Working directory — fail fast if Getwd fails (WP-C03)
	workDir, err := os.Getwd()
	if err != nil {
		logger.Error("cannot determine working directory", "error", err)
		return 1
	}

	// Tools dispatcher — fail fast on permission config errors (WP-C04)
	backupDir := filepath.Join(filepath.Dir(configPath), "backups")
	dispatcher, err := tools.DefaultDispatcher(workDir, backupDir, sessionsDir, &cfg.Permissions)
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

	if kc != nil {
		app.SetKeychain(kc)
	}

	// Resume on startup (C3): if configured, auto-resume the most recent session
	if cfg.Features.ResumeOnStartup {
		sessions, err := sessionMgr.ListSessions()
		if err == nil && len(sessions) > 0 {
			app.SetResumeSessionID(sessions[0].ID)
		}
	}

	p := tea.NewProgram(app, tea.WithAltScreen())

	// Signal handler sends tea.Quit through the program channel
	// instead of calling app.Shutdown() directly from a goroutine.
	// This ensures all state mutations happen inside Update(), preserving
	// Bubble Tea's single-threaded contract and preventing session corruption.
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
	sigDone := make(chan struct{})
	go func() {
		select {
		case <-sigCh:
			slog.Info("received shutdown signal, sending quit to TUI...")
			p.Send(tea.QuitMsg{})
		case <-sigDone:
		}
	}()

	if _, err := p.Run(); err != nil {
		logger.Error("TUI exited with error", "error", err)
		close(sigDone)
		return 1
	}

	close(sigDone)
	app.Shutdown()
	return 0
}
