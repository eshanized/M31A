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

// runExplain handles the `m31a explain` command. Evidence is
// collected deterministically BEFORE any provider resolution so unknown
// targets fail fast with a search-scope note even without API keys; the
// single synthesis call happens only for collectable evidence.
// Disambiguation order: existing file path → exact symbol → topic search.
func runExplain(args []string, workDir string, cfg *config.Config, logger *slog.Logger, registry provider.RegistryInterface) int {
	usage := func() {
		fmt.Fprintln(os.Stderr, "Usage: m31a explain <symbol|file|topic> [--format table|json]")
		fmt.Fprintln(os.Stderr, "  --format FMT    Output format: table, json (default: table)")
		fmt.Fprintln(os.Stderr, "")
		fmt.Fprintln(os.Stderr, "Disambiguation order:")
		fmt.Fprintln(os.Stderr, "  1. If argument is an existing file path → file mode (archaeology)")
		fmt.Fprintln(os.Stderr, "  2. If argument matches an indexed symbol exactly → symbol mode")
		fmt.Fprintln(os.Stderr, "  3. Otherwise → topic mode (free-text search)")
	}

	// Flags may precede or follow the query (documented usage is
	// "m31a explain QUERY [--format table|json]"), while stdlib flag
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
		fmt.Fprintln(os.Stderr, "Error: exactly one query argument required")
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

	gitClient := git.New(workDir)
	deps := explain.CollectorDeps{
		Graph:     graph,
		Index:     index,
		GitClient: gitClient,
		WorkDir:   workDir,
	}

	// Resolve mode per documented precedence: file → symbol → topic
	mode := explain.ResolveMode(query, workDir, index)

	pack, err := explain.Collect(ctx, deps, query, mode, cfg.Model.Default)
	if err != nil {
		if errors.Is(err, explain.ErrTargetNotFound) {
			var scopes string
			switch mode {
			case explain.ModeSymbol:
				scopes = fmt.Sprintf("indexed workspace symbols (%d symbols)", index.SymbolCount())
			case explain.ModeFile:
				scopes = fmt.Sprintf("file %q under %s", query, workDir)
			case explain.ModeTopic:
				scopes = fmt.Sprintf("indexed workspace symbols (%d symbols); source files under %s", index.SymbolCount(), workDir)
			}
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			fmt.Fprintf(os.Stderr, "Searched scopes: %s\n", scopes)
			return 1
		}
		logger.Error("evidence collection failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error collecting evidence: %v\n", err)
		return 1
	}

	// Compute rationale signals for the verdict section (EXPLAIN-03 / D-08)
	// Determine target path and symbol for signal computation
	targetPath := query
	symbol := query
	if mode == explain.ModeFile {
		// For file mode, find primary symbol in file for signal computation
		symbol = primarySymbolInFile(index, query)
		if symbol == "" {
			symbol = query // fallback
		}
	} else if mode == explain.ModeTopic {
		// For topic mode, we don't have a single symbol; use query as-is
		// Signals will be computed with zero consumers (no specific symbol)
		symbol = ""
	}

	staleThresholdDays := cfg.Intelligence.DepsRisk.StaleMonths * 30
	if staleThresholdDays <= 0 {
		staleThresholdDays = 365 // default 12 months
	}

	signals := explain.ComputeRationaleSignals(
		graph,
		index,
		gitClient,
		targetPath,
		symbol,
		time.Now(),
		staleThresholdDays,
	)

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

	answer, err := explain.Synthesize(ctx, pack, synthesizerAdapter{p: p}, modelID, &signals)
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

// primarySymbolInFile finds the primary exported symbol in a file.
// Mirrors the logic in collector.go for consistency.
func primarySymbolInFile(index *codeintel.SymbolIndex, filePath string) string {
	symbols := index.FileSymbols(filePath)
	for _, s := range symbols {
		if s.Exported && (s.Kind == "func" || s.Kind == "type" || s.Kind == "struct" || s.Kind == "interface") {
			return s.Name
		}
	}
	if len(symbols) > 0 {
		return symbols[0].Name
	}
	return ""
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