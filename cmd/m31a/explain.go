package main

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/intelligence/explain"
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

// runExplain handles the `m31a explain SYMBOL` command. Evidence is
// collected deterministically BEFORE any provider resolution so unknown
// targets fail fast with a search-scope note even without API keys; the
// single synthesis call happens only for collectable evidence.
func runExplain(args []string, workDir string, cfg *config.Config, logger *slog.Logger, registry provider.RegistryInterface) int {
	usage := func() {
		fmt.Fprintln(os.Stderr, "Usage: m31a explain SYMBOL [--format table|json]")
		fmt.Fprintln(os.Stderr, "  --format FMT    Output format: table, json (default: table)")
	}

	// Flags may precede or follow the symbol (documented usage is
	// "m31a explain SYMBOL [--format table|json]"), while stdlib flag
	// stops at the first positional — split them manually.
	format := "table"
	var positional []string
	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
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
	if len(positional) != 1 {
		fmt.Fprintln(os.Stderr, "Error: exactly one symbol argument required")
		usage()
		return 1
	}
	query := positional[0]

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	graph, index, err := buildExplainIntegrations(workDir)
	if err != nil {
		logger.Error("index build failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error building workspace index: %v\n", err)
		return 1
	}

	deps := explain.CollectorDeps{
		Graph:     graph,
		Index:     index,
		GitClient: git.New(workDir),
		WorkDir:   workDir,
	}

	pack, err := explain.Collect(ctx, deps, query, explain.ModeSymbol, cfg.Model.Default)
	if err != nil {
		if errors.Is(err, explain.ErrTargetNotFound) {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			fmt.Fprintf(os.Stderr, "Searched scopes: indexed workspace symbols (%d symbols); source files under %s\n",
				index.SymbolCount(), workDir)
			return 1
		}
		logger.Error("evidence collection failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error collecting evidence: %v\n", err)
		return 1
	}

	modelID := cfg.Model.Default
	req := provider.ChatRequest{Model: modelID}
	fallbackMode := cfg.Provider.FallbackMode
	if fallbackMode == "" {
		fallbackMode = "manual" // default per D-26
	}
	p, _, err := registry.GetProviderForRequest(req, fallbackMode, cfg.Provider.FallbackPriority, cfg.Provider.HealthCheckTimeoutSecs)
	if err != nil {
		logger.Error("provider selection failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error: failed to select provider: %v\n", err)
		return 1
	}
	if p == nil {
		fmt.Fprintln(os.Stderr, "Error: no active provider configured — set an API key environment variable")
		return 1
	}

	answer, err := explain.Synthesize(ctx, pack, synthesizerAdapter{p: p}, modelID)
	if err != nil {
		logger.Error("synthesis failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error synthesizing explanation: %v\n", err)
		return 1
	}

	switch format {
	case "json":
		return renderExplainJSON(answer)
	case "table":
		fallthrough
	default:
		return renderExplainText(answer, pack)
	}
}

// buildExplainIntegrations assembles the read-only code graph and symbol
// index used for evidence collection. BuildGraph fills the import graph;
// call edges are attached here from parsed call sites, mirroring how the
// event-replay projection assembles them in the indexed runtime.
func buildExplainIntegrations(workDir string) (*codeintel.CodeGraph, *codeintel.SymbolIndex, error) {
	graph, files, err := codeintel.BuildGraph(workDir, codeintel.AllParsers())
	if err != nil {
		return nil, nil, fmt.Errorf("build graph: %w", err)
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
	return graph, codeintel.BuildIndex(files), nil
}

// renderExplainText prints the table-format explanation.
func renderExplainText(answer *explain.ExplainAnswer, pack *types.EvidencePack) int {
	if err := explain.RenderText(os.Stdout, answer, pack); err != nil {
		fmt.Fprintf(os.Stderr, "Error rendering output: %v\n", err)
		return 1
	}
	return 0
}

// renderExplainJSON prints the JSON-format explanation.
func renderExplainJSON(answer *explain.ExplainAnswer) int {
	if err := explain.RenderJSON(os.Stdout, answer); err != nil {
		fmt.Fprintf(os.Stderr, "Error encoding JSON: %v\n", err)
		return 1
	}
	return 0
}
