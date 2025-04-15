package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"runtime"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/log"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/provider/openrouter"
	"github.com/eshanized/M31A/internal/provider/zen"
	"github.com/eshanized/M31A/internal/tui"
	"github.com/eshanized/M31A/pkg/keychain"
)

var Version = "dev"

func main() {
	// Parse CLI flags
	versionFlag := flag.Bool("version", false, "Print version and exit")
	helpFlag := flag.Bool("help", false, "Show usage information")
	flag.Usage = func() {
		fmt.Fprintf(os.Stderr, "M31A — Terminal AI Coding Agent\n\n")
		fmt.Fprintf(os.Stderr, "Usage: m31a [flags]\n\n")
		fmt.Fprintf(os.Stderr, "Flags:\n")
		flag.PrintDefaults()
		fmt.Fprintf(os.Stderr, "\nEnvironment Variables:\n")
		fmt.Fprintf(os.Stderr, "  M31A_CONFIG       Config file path (default: ~/.m31a/config.toml)\n")
		fmt.Fprintf(os.Stderr, "  OPENROUTER_API_KEY  OpenRouter API key\n")
		fmt.Fprintf(os.Stderr, "  ZEN_API_KEY         Zen API key\n")
		fmt.Fprintf(os.Stderr, "  M31A_LOG_FORMAT     Log format: json, text (default: json)\n")
		fmt.Fprintf(os.Stderr, "  M31A_LOG_LEVEL      Log level: debug, info, warn, error (default: info)\n")
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

	logger.Info("M31A starting",
		"version", Version,
		"go_version", runtime.Version(),
		"os", runtime.GOOS,
		"arch", runtime.GOARCH,
	)

	// Resolve config path: respect M31A_CONFIG env var, default to ~/.m31a/config.toml
	configPath := os.Getenv("M31A_CONFIG")
	if configPath == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			logger.Error("cannot determine home directory", "error", err)
			os.Exit(1)
		}
		configPath = filepath.Join(home, ".m31a", "config.toml")
	}

	// Ensure config directory exists
	if err := os.MkdirAll(filepath.Dir(configPath), 0755); err != nil {
		logger.Error("cannot create config directory", "error", err)
		os.Exit(1)
	}

	// Load config
	cfg, err := config.Load(configPath)
	if err != nil {
		logger.Error("failed to load config", "error", err)
		os.Exit(1)
	}
	if cfg == nil {
		cfg = config.DefaultConfig()
	}

	// Initialize keychain (may fail gracefully — keychain is optional)
	kc, _ := keychain.New()
	if kc != nil {
		if err := cfg.ResolveAPIKeys(kc); err != nil {
			logger.Warn("failed to resolve API keys", "error", err)
		}
	}

	// Create provider registry
	registry := provider.NewRegistry()

	// Register OpenRouter if API key available
	if cfg.Provider.OpenRouter.APIKey != "" {
		orClient, err := openrouter.New(cfg.Provider.OpenRouter.APIKey)
		if err != nil {
			logger.Warn("failed to create OpenRouter client", "error", err)
		} else {
			registry.Register("openrouter", orClient)
			logger.Info("OpenRouter provider registered")
		}
	}

	// Register Zen if API key available
	if cfg.Provider.Zen.APIKey != "" {
		zenClient, err := zen.New(cfg.Provider.Zen.APIKey)
		if err != nil {
			logger.Warn("failed to create Zen client", "error", err)
		} else {
			registry.Register("zen", zenClient)
			logger.Info("Zen provider registered")
		}
	}

	// Set active provider based on config default (first registered if default empty)
	if cfg.Provider.Default != "" {
		if err := registry.SetActive(cfg.Provider.Default); err != nil {
			logger.Warn("failed to set active provider, using first registered", "error", err)
		}
	}

	// Resolve API key for TUI (based on active provider)
	resolvedAPIKey := cfg.Provider.OpenRouter.APIKey
	if cfg.Provider.Default == "zen" {
		resolvedAPIKey = cfg.Provider.Zen.APIKey
	}

	// Create and launch TUI app
	app := tui.NewApp(Version, registry, resolvedAPIKey, configPath)
	p := tea.NewProgram(app, tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		logger.Error("TUI exited with error", "error", err)
		os.Exit(1)
	}
}
