package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/intelligence/deps/cli"
)

// runDepsCheck handles the `m31a deps check` command.
func runDepsCheck(args []string, workDir string, cfg *config.Config, logger *slog.Logger) int {
	fs := flag.NewFlagSet("deps check", flag.ExitOnError)
	format := fs.String("format", "table", "Output format: table, json")
	approve := fs.String("approve", "", "Approve pending checkpoint for module")
	fs.Usage = func() {
		fmt.Fprintln(os.Stderr, "Usage: m31a deps check <module> [--format table|json]")
		fmt.Fprintln(os.Stderr, "       m31a deps check --approve <module>")
		fmt.Fprintln(os.Stderr, "  --format FMT    Output format: table, json (default: table)")
		fmt.Fprintln(os.Stderr, "  --approve MOD   Approve pending high-risk checkpoint for module")
		fmt.Fprintln(os.Stderr, "")
		fmt.Fprintln(os.Stderr, "Exit codes:")
		fmt.Fprintln(os.Stderr, "  0  success")
		fmt.Fprintln(os.Stderr, "  1  usage error or user cancelled")
		fmt.Fprintln(os.Stderr, "  2  high-risk finding blocked (headless) — use --approve to resolve")
	}
	fs.Parse(args)

	if fs.NArg() < 1 && *approve == "" {
		fmt.Fprintln(os.Stderr, "Error: module argument required")
		fs.Usage()
		return 1
	}

	module := ""
	if fs.NArg() > 0 {
		module = fs.Arg(0)
	}

	cliCfg := cli.DepsCLIConfig{
		WorkDir:    workDir,
		Config:     cfg,
		Logger:     logger,
		Format:     *format,
		Module:     module,
		Approve:    *approve,
		Stdin:      os.Stdin,
		Stdout:     os.Stdout,
		Stderr:     os.Stderr,
		HTTPClient: nil,
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	return cli.RunDepsCheck(ctx, cliCfg)
}