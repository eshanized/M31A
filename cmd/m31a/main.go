package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	_ "net/http/pprof"
	"os"
	"os/signal"
	"path/filepath"
	"runtime"
	"strings"
	"sync/atomic"
	"syscall"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/rollback"
	"github.com/eshanized/M31A/internal/engine/session"
	"github.com/eshanized/M31A/internal/engine/tokens"
	"github.com/eshanized/M31A/internal/engine/workflow"
	"github.com/eshanized/M31A/internal/integrations/autodream"
	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/integrations/keychain"
	"github.com/eshanized/M31A/internal/integrations/ledger"
	"github.com/eshanized/M31A/internal/integrations/log"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/observability"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/ui/tui"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

var (
	Version   = "dev"
	Commit    = "unknown"
	Date      = "unknown"
	GoVersion = "unknown"
)

// restoreTerminal writes ANSI escape sequences to undo alt-screen mode,
// mouse capture, and hidden cursor. Called before os.Exit in the hard
// fallback path so the user's terminal is not left in a broken state.
func restoreTerminal() {
	fmt.Fprint(os.Stderr,
		"\033[?1049l", // exit alt-screen
		"\033[?1003l", // disable mouse tracking
		"\033[?25h",   // show cursor
		"\033[0m",     // reset attributes
	)
}

// runHeadlessWorkflow executes a full workflow in headless mode (no TUI).
// Runs the given goal through the workflow engine and returns exit code.
func runHeadlessWorkflow(goal string, cmdRegistry *tui.CommandRegistry, cfg *config.Config, model string, logger *slog.Logger, registry provider.RegistryInterface) int {
	// Working directory
	workDir, err := os.Getwd()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: cannot determine working directory: %v\n", err)
		return 1
	}

	// Session manager
	globalConfigDir, err := os.UserHomeDir()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: cannot determine home directory: %v\n", err)
		return 1
	}
	globalConfigDir = filepath.Join(globalConfigDir, ".m31a")
	sessionMgr := session.NewManager(globalConfigDir, workDir, session.ManagerOpts{
		CoordinatorTimeoutSecs: cfg.Features.CoordinatorTimeoutSecs,
	})

	// Tools dispatcher (headless mode uses deny policy for safety)
	backupDir := filepath.Join(workDir, ".m31a", "backups")
	dispatcher, err := tools.DefaultDispatcher(workDir, backupDir, backupDir, &cfg.Permissions, &cfg.Tools, tools.NewHeadlessDenyDecider())
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: failed to create tools dispatcher: %v\n", err)
		return 1
	}

	// Git client
	gitClient := git.New(workDir)

	// Ledger
	ledgerPath := filepath.Join(globalConfigDir, "LEDGER.md")
	ledgerClient := ledger.New(ledgerPath)

	// Token estimator
	tokenEst := tokens.NewEstimator(model)

	// Create a ChatRequest to get provider selection with fallback
	req := provider.ChatRequest{
		Model: model,
	}

	// Get provider with selection logic and fallback
	fallbackMode := cfg.Provider.FallbackMode
	if fallbackMode == "" {
		fallbackMode = "manual" // default per D-26
	}
	p, providerName, err := registry.GetProviderForRequest(req, fallbackMode, cfg.Provider.FallbackPriority, cfg.Provider.HealthCheckTimeoutSecs)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: failed to select provider: %v\n", err)
		return 1
	}
	if p == nil {
		fmt.Fprintln(os.Stderr, "error: no active provider")
		return 1
	}

	// Create session
	sess, err := sessionMgr.NewSession(model, providerName)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: failed to create session: %v\n", err)
		return 1
	}

	// Create workflow engine
	engine, err := workflow.NewEngine(
		sess.ID,
		workDir,
		backupDir,
		filepath.Join(workDir, ".m31a", "planning"),
		p,
		model,
		dispatcher,
		tokenEst,
		sessionMgr,
		cfg,
	)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: failed to create workflow engine: %v\n", err)
		return 1
	}
	defer engine.Close()

	engine.SetGit(gitClient)
	engine.SetLedger(ledgerClient)

	// Run all phases sequentially
	phases := []types.WorkflowPhase{
		types.PhaseInitialize,
		types.PhaseDiscuss,
		types.PhasePlan,
		types.PhaseExecute,
		types.PhaseVerify,
		types.PhaseRuntime,
		types.PhaseShip,
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Minute)
	defer cancel()

	for _, phase := range phases {
		logger.Info("running phase", "phase", phase, "goal", goal)
		fmt.Fprintf(os.Stderr, "Phase: %s\n", phase)

		result, err := engine.RunPhase(ctx, phase, goal)
		if err != nil {
			logger.Error("phase failed", "phase", phase, "error", err)
			fmt.Fprintf(os.Stderr, "Error in phase %s: %v\n", phase, err)
			return 1
		}
		if !result.Success {
			logger.Error("phase did not succeed", "phase", phase, "error", result.Error)
			fmt.Fprintf(os.Stderr, "Phase %s failed: %s\n", phase, result.Error)
			return 1
		}

		// Transition to next phase (except after last phase)
		if phase != types.PhaseShip {
			if err := engine.Transition(ctx, phase, phases[indexOf(phase, phases)+1]); err != nil {
				logger.Warn("transition failed", "from", phase, "error", err)
			}
		}
	}

	logger.Info("headless workflow completed", "goal", goal)
	fmt.Fprintln(os.Stderr, "Workflow completed successfully.")
	return 0
}

// indexOf returns the index of a phase in the slice.
func indexOf(phase types.WorkflowPhase, phases []types.WorkflowPhase) int {
	for i, p := range phases {
		if p == phase {
			return i
		}
	}
	return -1
}

func runHeadless(prompt string, registry provider.RegistryInterface, defaultModel string, logger *slog.Logger) int {
	// We need config for fallback settings; load minimal config
	configPath := os.Getenv("M31A_CONFIG")
	if configPath == "" {
		home, homeErr := os.UserHomeDir()
		if homeErr != nil {
			fmt.Fprintf(os.Stderr, "error: cannot determine home directory: %v\n", homeErr)
			return 1
		}
		configPath = filepath.Join(home, ".m31a", "config.toml")
	}

	cfg, err := config.Load(configPath)
	if err != nil {
		// Config not found or invalid; use defaults
		cfg = config.DefaultConfig()
	}

	modelID := defaultModel
	if modelID == "" {
		modelID = cfg.Model.Default
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	req := provider.ChatRequest{
		Model:    modelID,
		Messages: []types.Message{{Role: "user", Content: prompt}},
	}

	// Get provider with selection logic and fallback
	fallbackMode := cfg.Provider.FallbackMode
	if fallbackMode == "" {
		fallbackMode = "manual" // default per D-26
	}
	p, _, err := registry.GetProviderForRequest(req, fallbackMode, cfg.Provider.FallbackPriority, cfg.Provider.HealthCheckTimeoutSecs)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: failed to select provider: %v\n", err)
		return 1
	}
	if p == nil {
		fmt.Fprintln(os.Stderr, "error: no active provider")
		return 1
	}

	stream, err := p.ChatCompletionStream(ctx, req)
	if err != nil {
		// Handle fallback for sentinel errors in headless mode
		if fallbackMode == "prompt" {
			// In headless mode, treat "prompt" as "manual" per D-26
			fallbackMode = "manual"
		}
		if fallbackMode == "auto" {
			// Try fallback
			fallbackName, event, fallbackErr := provider.FindFallbackProvider(registry, registry.Active(), cfg.Provider.FallbackPriority, cfg.Provider.HealthCheckTimeoutSecs)
			if fallbackErr == nil && event != nil {
				p, _ = registry.Get(fallbackName)
				if p != nil {
					stream, err = p.ChatCompletionStream(ctx, req)
				}
			}
		}
		if err != nil {
			fmt.Fprintf(os.Stderr, "error: chat completion failed: %v\n", err)
			return 1
		}
	}
	defer func() {
		if err := stream.Close(); err != nil {
			slog.Debug("close stream iterator", "error", err, "resource", "headless_chat")
		}
	}()

	var response strings.Builder
	for {
		chunk, chunkErr := stream.Next()
		if chunkErr != nil {
			break
		}
		if chunk == nil {
			continue
		}
		response.WriteString(chunk.Delta)
	}

	fmt.Println(response.String())
	logger.Info("headless mode completed", "model", modelID, "response_length", response.Len())
	return 0
}

// startPprofServer starts a pprof HTTP server on localhost:6060.
// Only call this when debug mode is enabled (--debug or M31A_DEBUG=1).
// Binds to localhost only to prevent external access.
// Returns a cleanup function that shuts down the server.
func startPprofServer() func() {
	// The blank import of net/http/pprof registers handlers on DefaultServeMux.
	listener, err := net.Listen("tcp", "localhost:6060")
	if err != nil {
		slog.Warn("failed to start pprof server", "error", err)
		return nil
	}

	// Register /debug/memstats handler for memory statistics
	http.HandleFunc("/debug/memstats", func(w http.ResponseWriter, r *http.Request) {
		var m runtime.MemStats
		runtime.ReadMemStats(&m)
		_, _ = fmt.Fprintf(w, "Alloc: %d MB\n", m.Alloc/1024/1024)
		_, _ = fmt.Fprintf(w, "TotalAlloc: %d MB\n", m.TotalAlloc/1024/1024)
		_, _ = fmt.Fprintf(w, "Sys: %d MB\n", m.Sys/1024/1024)
		_, _ = fmt.Fprintf(w, "NumGC: %d\n", m.NumGC)
		_, _ = fmt.Fprintf(w, "HeapAlloc: %d MB\n", m.HeapAlloc/1024/1024)
		_, _ = fmt.Fprintf(w, "HeapSys: %d MB\n", m.HeapSys/1024/1024)
		_, _ = fmt.Fprintf(w, "HeapIdle: %d MB\n", m.HeapIdle/1024/1024)
		_, _ = fmt.Fprintf(w, "HeapInuse: %d MB\n", m.HeapInuse/1024/1024)
	})

	server := &http.Server{Handler: http.DefaultServeMux}
	go func() {
		if err := server.Serve(listener); err != nil && err != http.ErrServerClosed {
			slog.Error("pprof server error", "error", err)
		}
	}()

	slog.Info("pprof server started",
		"addr", "http://localhost:6060/debug/pprof/",
		"endpoints", []string{
			"profile (CPU, 30s)",
			"heap",
			"allocs",
			"goroutine",
			"threadcreate",
			"block",
			"mutex",
			"memstats",
		},
	)
	return func() {
		if err := server.Close(); err != nil {
			slog.Debug("pprof server close error", "error", err)
		}
	}
}

// runMigrate handles the migrate command
func runMigrate(args []string, workDir string, logger *slog.Logger) int {
	fs := flag.NewFlagSet("migrate", flag.ExitOnError)
	dryRun := fs.Bool("dry-run", false, "Parse artifacts and show counts without writing")
	replay := fs.Bool("replay", false, "Replay events to rebuild projections from existing .m31a/events.db")
	force := fs.Bool("force", false, "Skip ONE_WAY_DOOR confirmation prompt")
	fs.Usage = func() {
		fmt.Fprintln(os.Stderr, "Usage: m31a migrate [--dry-run] [--replay] [--force]")
		fmt.Fprintln(os.Stderr, "  --dry-run    Parse artifacts and show counts without writing")
		fmt.Fprintln(os.Stderr, "  --replay     Replay events to rebuild projections (no parsing)")
		fmt.Fprintln(os.Stderr, "  --force      Skip ONE_WAY_DOOR confirmation prompt")
	}
	fs.Parse(args)

	planningDir := filepath.Join(workDir, ".planning")
	m31aDir := filepath.Join(workDir, ".m31a")

	if *replay {
		// Replay mode: rebuild projections from existing events
		return runMigrateReplay(m31aDir)
	}

	// Check if .planning/ exists
	if _, err := os.Stat(filepath.Join(workDir, ".planning")); os.IsNotExist(err) {
		fmt.Fprintln(os.Stderr, "error: .planning/ directory not found")
		return 1
	}

	// Check if already migrated
	eventsDB := filepath.Join(workDir, ".m31a", "events.db")
	if _, err := os.Stat(eventsDB); err == nil && !*force {
		fmt.Fprintln(os.Stderr, "error: .m31a/events.db already exists. Use --force to re-migrate.")
		return 1
	}

	// ONE_WAY_DOOR confirmation
	if !*force && !*dryRun {
		fmt.Fprintln(os.Stderr, "")
		fmt.Fprintln(os.Stderr, "⚠️  WARNING: This is a ONE_WAY_DOOR operation (D-09).")
		fmt.Fprintln(os.Stderr, "   It will archive .planning/ and make .m31a/events.db the authoritative source.")
		fmt.Fprintln(os.Stderr, "   There is no automatic rollback — the archive is your backup.")
		fmt.Fprintln(os.Stderr, "")
		fmt.Fprint(os.Stderr, "Continue? [y/N] ")

		var response string
		fmt.Scanln(&response)
		if strings.ToLower(strings.TrimSpace(response)) != "y" {
			fmt.Fprintln(os.Stderr, "Migration cancelled.")
			return 0
		}
	}

	ctx := context.Background()
	planningDir := ".planning"
	m31aDir := ".m31a"

	if *dryRun {
		fmt.Fprintln(os.Stderr, "=== DRY RUN MODE ===")
	}

	counts, err := Migrate(ctx, planningDir, m31aDir)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: migration failed: %v\n", err)
		return 1
	}

	if *dryRun {
		fmt.Fprintln(os.Stderr, "=== DRY RUN COMPLETE ===")
		fmt.Fprintf(os.Stderr, "Would migrate: %d requirements, %d phases, %d decisions, %d research, %d codebase maps\n",
			counts.Requirements, counts.Phases, counts.Decisions, counts.Research, counts.CodebaseMaps)
		fmt.Fprintln(os.Stderr, "No files were written.")
		return 0
	}

	fmt.Fprintf(os.Stderr, "Migrated: %d requirements, %d phases, %d decisions, %d research, %d codebase maps\n",
		counts.Requirements, counts.Phases, counts.Decisions, counts.Research, counts.CodebaseMaps)
	fmt.Fprintf(os.Stderr, "Archived .planning/ to .planning.archived.<timestamp>/\n")
	fmt.Fprintln(os.Stderr, "Migration completed successfully.")
	return 0
}

func runMigrateReplay(m31aDir string) int {
	eventsDB := filepath.Join(m31aDir, "events.db")
	if _, err := os.Stat(eventsDB); os.IsNotExist(err) {
		fmt.Fprintln(os.Stderr, "error: events.db not found. Run migration first.")
		return 1
	}

	store, err := NewEventStore(filepath.Join(m31aDir, "events.db"))
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: failed to open event store: %v\n", err)
		return 1
	}
	defer store.Close()

	pm := NewProjectionManager(store)
	pm.Register(NewProjectProjection())
	pm.Register(NewRequirementsProjection())
	pm.Register(NewDecisionsProjection())
	pm.Register(NewResearchProjection())
	pm.Register(NewRunProjection())

	ctx := context.Background()
	for _, name := range []string{"project", "requirements", "decisions", "research", "runs"} {
		fmt.Fprintf(os.Stderr, "Rebuilding projection: %s... ", name)
		if err := pm.Rebuild(ctx, name); err != nil {
			fmt.Fprintf(os.Stderr, "FAILED: %v\n", err)
			return 1
		}
		fmt.Fprintln(os.Stderr, "OK")
	}

	fmt.Fprintln(os.Stderr, "All projections rebuilt successfully.")
	return 0
}

// runModelsList handles the "m31a models list" command
func runModelsList(args []string, cfg *config.Config, logger *slog.Logger, registry provider.RegistryInterface) int {
	fs := flag.NewFlagSet("models list", flag.ExitOnError)
	providerFilter := fs.String("provider", "", "Filter models by provider name")
	jsonOutput := fs.Bool("json", false, "Output as JSON instead of table")
	fs.Usage = func() {
		fmt.Fprintln(os.Stderr, "Usage: m31a models list [--provider <name>] [--json]")
		fmt.Fprintln(os.Stderr, "  --provider <name>  Filter models by provider (openrouter, zen, nvidia)")
		fmt.Fprintln(os.Stderr, "  --json             Output full ModelInfo as JSON")
	}
	fs.Parse(args)

	// Initialize registry (triggers lazy provider registration)
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	var allModels []types.ModelInfo
	providerNames := registry.ListAll()

	if *providerFilter != "" {
		// Validate provider exists
		found := false
		for _, name := range providerNames {
			if name == *providerFilter {
				found = true
				break
			}
		}
		if !found {
			fmt.Fprintf(os.Stderr, "error: provider %q not found\n", *providerFilter)
			return 1
		}
		providerNames = []string{*providerFilter}
	}

	// Fetch models from each provider
	for _, name := range providerNames {
		p, err := registry.Get(name)
		if err != nil {
			logger.Warn("failed to get provider", "provider", name, "error", err)
			continue
		}
		models, err := p.FetchModels(ctx)
		if err != nil {
			logger.Warn("failed to fetch models", "provider", name, "error", err)
			fmt.Fprintf(os.Stderr, "warning: failed to fetch models from %s: %v\n", name, err)
			continue
		}
		allModels = append(allModels, models...)
	}

	if len(allModels) == 0 {
		fmt.Fprintln(os.Stderr, "No models found")
		return 0
	}

	if *jsonOutput {
		// JSON output with full ModelInfo
		encoder := json.NewEncoder(os.Stdout)
		encoder.SetIndent("", "  ")
		if err := encoder.Encode(allModels); err != nil {
			fmt.Fprintf(os.Stderr, "error: failed to encode JSON: %v\n", err)
			return 1
		}
		return 0
	}

	// Table output using simple text table (no lipgloss/bubbles dependency for headless)
	printModelsTable(allModels)
	return 0
}

// printModelsTable prints a simple text table of models
func printModelsTable(models []types.ModelInfo) {
	// Column widths
	idWidth := 50
	providerWidth := 12
	contextWidth := 10
	toolsWidth := 6
	reasoningWidth := 10
	visionWidth := 6

	// Print header
	fmt.Printf("%-*s %-*s %-*s %-*s %-*s %-*s\n",
		idWidth, "ID",
		providerWidth, "Provider",
		contextWidth, "ContextLen",
		toolsWidth, "Tools",
		reasoningWidth, "Reasoning",
		visionWidth, "Vision")
	fmt.Printf("%s %s %s %s %s %s\n",
		strings.Repeat("-", idWidth),
		strings.Repeat("-", providerWidth),
		strings.Repeat("-", contextWidth),
		strings.Repeat("-", toolsWidth),
		strings.Repeat("-", reasoningWidth),
		strings.Repeat("-", visionWidth))

	// Print rows
	for _, m := range models {
		contextStr := fmt.Sprintf("%dk", m.ContextLength/1000)
		toolsStr := boolStr(m.Capabilities.Tools)
		reasoningStr := boolStr(m.Capabilities.Reasoning)
		visionStr := boolStr(m.Capabilities.Vision)

		fmt.Printf("%-*s %-*s %-*s %-*s %-*s %-*s\n",
			idWidth, truncate(m.ID, idWidth),
			providerWidth, m.Provider,
			contextWidth, contextStr,
			toolsWidth, toolsStr,
			reasoningWidth, reasoningStr,
			visionWidth, visionStr)
	}
}

func boolStr(b bool) string {
	if b {
		return "yes"
	}
	return "no"
}

func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}

func main() {
	os.Exit(run())
}

// run contains all application logic so that deferred cleanup functions
// execute before os.Exit. Returns the exit code (0 for success, 1 for error).
func run() int {
	defer observability.RecoverAndCapture(Version, Commit)

	// Build command registry for usage and TUI
	cmdRegistry := tui.DefaultCommands()

	// Parse CLI flags
	versionFlag := flag.Bool("version", false, "Print version and exit")
	helpFlag := flag.Bool("help", false, "Show usage information")
	promptFlag := flag.String("prompt", "", "Run in headless mode: send prompt to LLM and print response")
	goalFlag := flag.String("goal", "", "Run in headless mode: execute full workflow with goal")
	modelFlag := flag.String("model", "", "Model ID for headless mode (default: config model or first available)")
	debugMode := flag.Bool("debug", false, "enable debug mode (pprof on localhost:6060, debug logging)")
	logLevel := flag.String("log-level", "", "log level (debug, info, warn, error); overrides M31A_LOG_LEVEL env")
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
		// Prefer the build-time GoVersion if set; fall back to runtime.
		if GoVersion != "unknown" {
			goVer = GoVersion
		}
		fmt.Printf("m31a %s (%s, %s) %s/%s (Go %s)\n", Version, Commit, Date, runtime.GOOS, runtime.GOARCH, goVer)
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

	// Configure log level based on --debug flag, --log-level flag, or M31A_LOG_LEVEL env.
	// Priority: --log-level flag > --debug flag > M31A_LOG_LEVEL env > default (info)
	if *debugMode || os.Getenv("M31A_DEBUG") == "1" {
		slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelDebug})))
		logger = slog.Default()
	}
	if *logLevel != "" {
		var lvl slog.Level
		switch strings.ToLower(*logLevel) {
		case "debug":
			lvl = slog.LevelDebug
		case "info":
			lvl = slog.LevelInfo
		case "warn":
			lvl = slog.LevelWarn
		case "error":
			lvl = slog.LevelError
		default:
			lvl = slog.LevelInfo
		}
		slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: lvl})))
		logger = slog.Default()
	}

	// Debug mode: pprof available at http://localhost:6060/debug/pprof/
	// Gated behind --debug flag or M31A_DEBUG=1 to prevent production exposure.
	var pprofCleanup func()
	if *debugMode || os.Getenv("M31A_DEBUG") == "1" {
		pprofCleanup = startPprofServer()
	}
	if pprofCleanup != nil {
		defer pprofCleanup()
	}

	logger.Info("M31A starting",
		"version", Version,
		"commit", Commit,
		"date", Date,
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

	// Detect previous unclean shutdown from force-exit sentinel.
	// The signal handler writes this when the TUI doesn't exit within 5 seconds.
	sentinelPath := filepath.Join(filepath.Dir(configPath), ".force-exit")
	if _, serr := os.Stat(sentinelPath); serr == nil {
		logger.Warn("detected previous unclean shutdown (force-exit sentinel present); cleaning up")
		_ = os.Remove(sentinelPath)
	}

	// Provider registry — lazy: provider registration deferred until first LLM call
	tools.SetVersion(Version)

	// Configure model capability detection from config (F-011, F-012)
	provider.SetCapabilityConfig(
		cfg.ModelCapabilities.ExtraReasoningPatterns,
		cfg.ModelCapabilities.ExtraToolCapablePatterns,
		cfg.ModelCapabilities.ExtraCompletionOnlyPatterns,
		cfg.ModelCapabilities.ExtraNonChatPatterns,
	)

	// Keychain — eager init (needed for TUI API key storage, lightweight)
	kc, kcErr := keychain.New()
	if kcErr != nil {
		logger.Warn("keychain initialization failed", "error", kcErr)
	}
	kc = keychain.NewCached(kc)

	registry := provider.NewLazyRegistry(func() *provider.Registry {
		// Resolve API keys via keychain — deferred until first LLM call
		if kc != nil {
			if resolveErr := cfg.ResolveAPIKeys(kc); resolveErr != nil {
				logger.Warn("failed to resolve API keys", "error", resolveErr)
			}
		}

		reg := provider.NewRegistry()

		if cfg.Provider.OpenRouter.APIKey != "" {
			if regErr := tui.RegisterProvider(reg, cfg, "openrouter", cfg.Provider.OpenRouter.APIKey, Version); regErr != nil {
				logger.Warn("failed to register OpenRouter provider", "error", regErr)
			}
		}

		if cfg.Provider.Zen.APIKey != "" {
			if regErr := tui.RegisterProvider(reg, cfg, "zen", cfg.Provider.Zen.APIKey, Version); regErr != nil {
				logger.Warn("failed to register Zen provider", "error", regErr)
			}
		}

		if cfg.Provider.Nvidia.APIKey != "" {
			if regErr := tui.RegisterProvider(reg, cfg, "nvidia", cfg.Provider.Nvidia.APIKey, Version); regErr != nil {
				logger.Warn("failed to register NVIDIA provider", "error", regErr)
			}
		}

		if cfg.Provider.Default != "" {
			if setErr := reg.SetActive(cfg.Provider.Default); setErr != nil {
				logger.Warn("configured default provider not registered", "default", cfg.Provider.Default, "error", setErr)
			}
		}

		return reg
	})

	hasProvider := registry.Active() != ""
	if !hasProvider {
		logger.Warn("no active provider configured — LLM features will be unavailable")
	}

	// Headless mode: --prompt sends a single prompt to the LLM and prints the response
	if *promptFlag != "" {
		if !hasProvider {
			fmt.Fprintln(os.Stderr, "error: no provider configured — set an API key environment variable")
			return 1
		}
		model := *modelFlag
		if model == "" {
			model = cfg.Model.Default
		}
		return runHeadless(*promptFlag, registry, model, logger)
	}

	// Headless mode: --goal runs full workflow without TUI
	if *goalFlag != "" {
		if !hasProvider {
			fmt.Fprintln(os.Stderr, "error: no provider configured — set an API key environment variable")
			return 1
		}
		model := *modelFlag
		if model == "" {
			model = cfg.Model.Default
		}
		// Validate goal flag
		if goalFlag == nil || *goalFlag == "" {
			fmt.Fprintln(os.Stderr, "error: --goal is required in headless mode")
			return 1
		}
		return runHeadlessWorkflow(*goalFlag, cmdRegistry, cfg, model, logger, registry)
	}

	// Migration command: migrate .planning/ to .m31a/
	if flag.Arg(0) == "migrate" {
		return runMigrate(flag.Args()[1:], workDir, logger)
	}

	// Models list command: m31a models list [--provider <name>] [--json]
	if flag.Arg(0) == "models" && flag.Arg(1) == "list" {
		return runModelsList(flag.Args()[2:], cfg, logger, registry)
	}

	// Index command: m31a index [--incremental] [--progress]
	if flag.Arg(0) == "index" {
		return runIndex(flag.Args()[1:], workDir, logger)
	}

	// Impact command: m31a impact <symbol> [--depth N] [--format table|json|graphviz]
	if flag.Arg(0) == "impact" {
		return runImpact(flag.Args()[1:], workDir, logger)
	}

	// Explain command: m31a explain SYMBOL [--format table|json]
	if flag.Arg(0) == "explain" {
		return runExplain(flag.Args()[1:], workDir, cfg, logger, registry)
	}

	// Investigate command: m31a investigate SYMPTOM [--baseline <ref>] [--repro <cmd>] [--format table|json]
	if flag.Arg(0) == "investigate" {
		return runInvestigate(flag.Args()[1:], workDir, cfg, logger, registry)
	}

	// Arch check command: m31a arch check [--config arch.toml]
	if flag.Arg(0) == "arch" && flag.Arg(1) == "check" {
		return runArchCheck(flag.Args()[2:], workDir, logger)
	}

	// Working directory — fail fast if Getwd fails (WP-C03)
	workDir, err := os.Getwd()
	if err != nil {
		logger.Error("cannot determine working directory", "error", err)
		return 1
	}

	// Session manager — project-local sessions in <workDir>/.m31a/
	globalConfigDir := filepath.Dir(configPath)
	sessionMgr := session.NewManager(globalConfigDir, workDir, session.ManagerOpts{
		CoordinatorTimeoutSecs: cfg.Features.CoordinatorTimeoutSecs,
	})

	// Tools dispatcher — use nil policy initially, will set interactive policy after AppState creation
	backupDir := filepath.Join(workDir, ".m31a", "backups")
	dispatcher, err := tools.DefaultDispatcher(workDir, backupDir, backupDir, &cfg.Permissions, &cfg.Tools, nil)
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

	// Theme — M31A ships with a single dark theme.
	// Light/auto themes are not supported; always use dark.
	themeMode := theme.ModeDark

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
	if cfg.Model.Default != "" {
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
		Profiles:    cfg.Agents.Profiles,
		NewDispatcher: tools.NewDispatcherFactory(
			backupDir, backupDir, &cfg.Permissions, &cfg.Tools, nil, cfg.Agents.Profiles,
		),
	})
	// Register the Agent tool on the parent dispatcher (non-child so it can
	// spawn in background). The factory passes nil for the child-side manager
	// reference to avoid a registration cycle; children created by the
	// factory get isChild=true and cannot spawn grandchildren in background.
	if err := dispatcher.Register(tools.NewAgent(subagentMgr, false, 0, cfg.Agents.Profiles)); err != nil {
		logger.Error("failed to register Agent tool on parent dispatcher", "error", err)
		return 1
	}
	app.SetSubagentManager(subagentMgr)

	// Best-effort sweep of stale agent worktrees/branches from prior crashes.
	sweepCtx, sweepCancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer sweepCancel()
	if err := subagent.Sweep(sweepCtx, workDir); err != nil {
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

	// Wire program into REPL model so goroutines can send messages (B23 fix)
	app.SetReplProgram(p)

	// Signal handler sends tea.Quit through the program channel
	// instead of calling app.Shutdown() directly from a goroutine.
	// This ensures all state mutations happen inside Update(), preserving
	// Bubble Tea's single-threaded contract and preventing session corruption.
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
	defer signal.Stop(sigCh)
	sigDone := make(chan struct{})
	var programExited atomic.Bool
	go func() {
		defer func() {
			if r := recover(); r != nil {
				observability.RecoverAndCapture(Version, Commit)
			}
		}()
		for {
			select {
			case <-sigCh:
				if programExited.Load() {
					return
				}
				slog.Info("received shutdown signal, sending quit to TUI...")
				p.Send(tea.QuitMsg{})
				// H-24: Hard fallback — force exit after 5 seconds if TUI doesn't quit.
				// Write a sentinel file so run() can detect an unclean shutdown
				// and clean up stale session state on next launch.
				select {
				case <-sigDone:
					return
				case <-time.After(5 * time.Second):
					if programExited.Load() {
						return
					}
					slog.Warn("TUI did not exit within timeout, forcing exit")
					sentinel := filepath.Join(filepath.Dir(configPath), ".force-exit")
					_ = os.WriteFile(sentinel, []byte("force-exit"), 0o644)
					cleanup()
					restoreTerminal()
					os.Exit(1)
				}
			case <-sigDone:
				return
			}
		}
	}()

	if _, err := p.Run(); err != nil {
		programExited.Store(true)
		logger.Error("TUI exited with error", "error", err)
		close(sigDone)
		return 1
	}

	// Cancel the signal goroutine before Shutdown to prevent the 5-second
	// os.Exit(1) timer from racing with cleanup.
	programExited.Store(true)
	close(sigDone)
	app.Shutdown()
	return 0
}
