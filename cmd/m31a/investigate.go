package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/intelligence/explain"
	"github.com/eshanized/M31A/internal/intelligence/investigate"
)

// synthesizerAdapter adapts a registry-resolved LLMProvider onto the
// narrow explain.Synthesizer seam. Intelligence never talks HTTP to vendors
// directly — it only ever sees this interface (RESEARCH Pitfall 10).
type synthesizerAdapter struct {
	p provider.LLMProvider
}

// ChatCompletion forwards to the underlying provider.
func (a synthesizerAdapter) ChatCompletion(ctx context.Context, req types.ChatRequest) (*types.ChatResponse, error) {
	return a.p.ChatCompletion(ctx, req)
}

// runInvestigate handles the `m31a investigate` command.
func runInvestigate(args []string, workDir string, cfg *config.Config, logger *slog.Logger, registry provider.RegistryInterface) int {
	usage := func() {
		fmt.Fprintln(os.Stderr, "Usage: m31a investigate <symptom> [--baseline <ref>] [--repro <cmd>] [--format table|json]")
		fmt.Fprintln(os.Stderr, "  --baseline REF  Baseline commit (default: HEAD~N, N=bisect_max_commits)")
		fmt.Fprintln(os.Stderr, "  --repro CMD     Repro command (default: config > auto-detect Go/npm)")
		fmt.Fprintln(os.Stderr, "  --format FMT    Output format: table, json (default: table)")
		fmt.Fprintln(os.Stderr, "")
		fmt.Fprintln(os.Stderr, "Exit codes:")
		fmt.Fprintln(os.Stderr, "  0  attributed (culprit found with verified confidence)")
		fmt.Fprintln(os.Stderr, "  1  usage error")
		fmt.Fprintln(os.Stderr, "  3  not-reproducible-in-window")
		fmt.Fprintln(os.Stderr, "  4  unattributable")
	}

	// Flags may precede or follow the symptom (documented usage is
	// "m31a investigate SYMPTOM [--baseline <ref>] [--repro <cmd>] [--format table|json]"),
	// while stdlib flag stops at the first positional — split them manually.
	baseline := ""
	repro := ""
	format := "table"
	var positional []string
	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
		case arg == "--baseline" || arg == "-baseline":
			if i+1 >= len(args) {
				fmt.Fprintf(os.Stderr, "Error: %s requires a value\n", arg)
				usage()
				return 1
			}
			i++
			baseline = args[i]
		case strings.HasPrefix(arg, "--baseline="):
			baseline = strings.TrimPrefix(arg, "--baseline=")
		case strings.HasPrefix(arg, "-baseline="):
			baseline = strings.TrimPrefix(arg, "-baseline=")
		case arg == "--repro" || arg == "-repro":
			if i+1 >= len(args) {
				fmt.Fprintf(os.Stderr, "Error: %s requires a value\n", arg)
				usage()
				return 1
			}
			i++
			repro = args[i]
		case strings.HasPrefix(arg, "--repro="):
			repro = strings.TrimPrefix(arg, "--repro=")
		case strings.HasPrefix(arg, "-repro="):
			repro = strings.TrimPrefix(arg, "-repro=")
		case arg == "--format" || arg == "-format":
			if i+1 >= len(args) {
				fmt.Fprintf(os.Stderr, "Error: %s requires a value\n", arg)
				usage()
				return 1
			}
			i++
			format = args[i]
		case strings.HasPrefix(arg, "--format="):
			format = strings.TrimPrefix(arg, "--format=")
		case strings.HasPrefix(arg, "-format="):
			format = strings.TrimPrefix(arg, "-format=")
		default:
			positional = append(positional, arg)
		}
	}

	if format != "table" && format != "json" {
		fmt.Fprintf(os.Stderr, "Error: unsupported format %q\n", format)
		usage()
		return 1
	}
	if len(positional) == 0 {
		fmt.Fprintln(os.Stderr, "Error: exactly one symptom argument required")
		usage()
		return 1
	}
	symptom := strings.Join(positional, " ")

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()

	// Build code graph and index for impact analysis
	graph, files, err := codeintel.BuildGraph(workDir, codeintel.AllParsers())
	if err != nil {
		logger.Error("index build failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error building workspace index: %v\n", err)
		return 1
	}
	for _, f := range files {
		for _, cs := range f.CallSites {
			graph.AddCallEdge(codeintel.CallEdge{
				CallerFile: f.Path,
				CallerLine: cs.Line,
				CallerName: cs.CallerName,
				CalleeName: cs.CalleeName,
			})
		}
	}
	index := codeintel.BuildIndex(files)

	gitClient := git.New(workDir)
	gitRunner := investigate.NewGitRunner(workDir)
	mgr := investigate.NewWorktreeManager(gitRunner, workDir)

	// Resolve baseline
	var baselineRef string
	if *baseline != "" {
		baselineRef = *baseline
	} else {
		maxCommits := cfg.Intelligence.BisectMaxCommits
		if maxCommits <= 0 {
			maxCommits = 50
		}
		// Get HEAD~N where N = maxCommits
		out, err := gitClient.Run("rev-parse", fmt.Sprintf("HEAD~%d", maxCommits))
		if err != nil {
			// Fallback: use first commit in history
			out, _ = gitClient.Run("rev-list", "--max-parents=0", "HEAD")
		}
		baselineRef = strings.TrimSpace(out)
	}

	// Resolve head (default: HEAD)
	headRef := "HEAD"

	// Resolve repro command following precedence: flag > config > auto-detect
	var reproCmd string
	if strings.TrimSpace(*repro) != "" {
		reproCmd = strings.TrimSpace(*repro)
	} else if strings.TrimSpace(cfg.Intelligence.ReproCommand) != "" {
		reproCmd = strings.TrimSpace(cfg.Intelligence.ReproCommand)
	} else {
		// Auto-detect
		if _, err := os.Stat(filepath.Join(workDir, "go.mod")); err == nil {
			reproCmd = "go test ./..."
		} else if _, err := os.Stat(filepath.Join(workDir, "package.json")); err == nil {
			reproCmd = "npm test --silent"
		}
	}

	// Open EventStore if .m31a exists
	var eventStore types.EventStore
	m31aDir := filepath.Join(workDir, ".m31a")
	eventsDB := filepath.Join(m31aDir, "events.db")
	if _, err := os.Stat(eventsDB); err == nil {
		// Try to open existing event store
		if store, err := openEventStore(eventsDB); err == nil {
			eventStore = store
			defer store.Close()
		} else {
			logger.Warn("failed to open event store", "error", err)
		}
	}

	// Resolve synthesizer (provider + model) like explain CLI
	modelID := cfg.Model.Default
	req := provider.ChatRequest{Model: modelID}
	fallbackMode := cfg.Provider.FallbackMode
	if fallbackMode == "" {
		fallbackMode = "manual"
	}
	p, _, err := registry.GetProviderForRequest(req, fallbackMode, cfg.Provider.FallbackPriority, cfg.Provider.HealthCheckTimeoutSecs)
	var synth investigate.Synthesizer
	if err != nil || p == nil {
		logger.Warn("provider selection failed, continuing without synthesis", "error", err)
	} else {
		synth = synthesizerAdapter{p: p}
	}

	maxCommits := cfg.Intelligence.BisectMaxCommits
	if maxCommits <= 0 {
		maxCommits = 50
	}

	deps := investigate.ReportDeps{
		Manager:    mgr,
		Runner:     gitRunner,
		Graph:      graph,
		Index:      index,
		GitClient:  gitClient,
		Synth:      synth,
		ModelID:    modelID,
		MaxCommits: maxCommits,
	}

	// Emit investigation started event
	_ = investigate.EmitInvestigationStarted(ctx, eventStore, investigate.InvestigationStartedPayload{
		Symptom:    symptom,
		Baseline:   baselineRef,
		Head:       headRef,
		WindowSize: maxCommits,
	})

	report, err := investigate.BuildRootCauseReport(ctx, deps, symptom, baselineRef, headRef, reproCmd)
	if err != nil {
		logger.Error("investigation failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error during investigation: %v\n", err)
		return 1
	}

	// Emit investigation completed event
	_ = investigate.EmitInvestigationCompleted(ctx, eventStore, investigate.InvestigationCompletedPayload{
		Outcome:              report.Outcome,
		CulpritSHA:           report.CulpritSHA,
		CulpritConfidence:    string(report.CulpritConfidence),
		MechanismConfidence:  string(report.MechanismConfidence),
	})

	// Render output
	var exitCode int
	switch report.Outcome {
	case "attributed":
		exitCode = 0
	case "not-reproducible-in-window":
		exitCode = 3
	case "unattributable":
		fallthrough
	default:
		exitCode = 4
	}

	switch *format {
	case "json":
		if err := investigate.RenderInvestigationJSON(os.Stdout, report); err != nil {
			logger.Error("JSON render failed", "error", err)
			fmt.Fprintf(os.Stderr, "Error rendering JSON: %v\n", err)
			return 1
		}
	case "table":
		fallthrough
	default:
		if err := investigate.RenderInvestigationText(os.Stdout, report); err != nil {
			logger.Error("text render failed", "error", err)
			fmt.Fprintf(os.Stderr, "Error rendering text: %v\n", err)
			return 1
		}
	}

	return exitCode
}

// openEventStore opens the SQLite event store.
func openEventStore(path string) (types.EventStore, error) {
	// This is a simplified version - in practice, we'd use the actual event store implementation
	// For now, return nil to use nil-store safe path
	return nil, fmt.Errorf("event store not implemented in CLI yet")
}