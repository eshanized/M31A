package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"

	"github.com/eshanized/M31A/internal/integrations/archcheck"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
)

// runArchCheck handles the `m31a arch check` command.
func runArchCheck(args []string, workDir string, logger *slog.Logger) int {
	fs := flag.NewFlagSet("arch check", flag.ExitOnError)
	configPath := fs.String("config", "arch.toml", "Architecture rules config file")
	fs.Usage = func() {
		fmt.Fprintln(os.Stderr, "Usage: m31a arch check [--config arch.toml]")
		fmt.Fprintln(os.Stderr, "  --config FILE   Architecture rules config file (default: arch.toml)")
	}
	fs.Parse(args)

	// Load architecture rules
	rules, err := archcheck.LoadArchRules(*configPath)
	if err != nil {
		logger.Error("load arch rules failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error loading arch rules: %v\n", err)
		return 1
	}

	// Create indexer and build
	indexer := codeintel.NewIndexerWithStore(workDir, nil)
	if err := indexer.Build(context.Background()); err != nil {
		logger.Error("index build failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error building index: %v\n", err)
		return 1
	}

	// Get graph and detect violations
	graph := indexer.Projection().Graph()
	violations := archcheck.DetectViolations(graph, rules)

	// Print violations
	hasErrors := false
	for _, v := range violations {
		fmt.Printf("%s: %s: %s -> %s: %s\n",
			v.Severity, v.Type, v.From, v.To, v.Message)
		if v.Severity == archcheck.SeverityError {
			hasErrors = true
		}
	}

	if len(violations) == 0 {
		fmt.Println("No architecture violations found.")
	}

	// Exit code 1 on error-severity violations
	if hasErrors {
		return 1
	}
	return 0
}