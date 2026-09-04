package cli

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/intelligence/deps"
	"github.com/eshanized/M31A/internal/memory/eventstore"
)

// DepsCLIConfig holds configuration for the deps CLI command.
type DepsCLIConfig struct {
	WorkDir    string
	Config     *config.Config
	Logger     *slog.Logger
	Format     string
	Module     string
	Approve    string
	Stdin      *os.File
	Stdout     *os.File
	Stderr     *os.File
	HTTPClient *http.Client // for test injection
}

// RunDepsCheck executes the deps check command.
func RunDepsCheck(ctx context.Context, cfg DepsCLIConfig) int {
	if cfg.Approve != "" {
		return runDepsApprove(ctx, cfg)
	}

	if cfg.Module == "" {
		fmt.Fprintln(cfg.Stderr, "Error: module argument required")
		printUsage(cfg.Stderr)
		return 1
	}

	if cfg.Format != "table" && cfg.Format != "json" {
		fmt.Fprintf(cfg.Stderr, "Error: unsupported format %q\n", cfg.Format)
		printUsage(cfg.Stderr)
		return 1
	}

	// Compute policy hash from config
	policyHash := computePolicyHash(cfg.Config)

	// Open EventStore if .m31a exists
	var eventStore types.EventStore
	m31aDir := filepath.Join(cfg.WorkDir, ".m31a")
	if _, err := os.Stat(m31aDir); err == nil {
		eventsDB := filepath.Join(m31aDir, "events.db")
		if _, err := os.Stat(eventsDB); err == nil {
			store, err := eventstore.NewEventStore(eventsDB)
			if err != nil {
				cfg.Logger.Warn("failed to open event store", "error", err)
				fmt.Fprintf(cfg.Stderr, "Warning: failed to open event store: %v\n", err)
			} else {
				eventStore = store
				defer store.Close()
			}
		}
	}

	// Build symbol index for transitive impact
	var graph *codeintel.CodeGraph
	indexer := codeintel.NewIndexerWithStore(cfg.WorkDir, eventStore)
	if err := indexer.Build(ctx); err != nil {
		cfg.Logger.Warn("index build failed, proceeding without transitive impact", "error", err)
		fmt.Fprintf(cfg.Stderr, "Warning: failed to build symbol index: %v\n", err)
	} else {
		graph = indexer.Projection().Graph()
	}

	// Create clients with default base URLs
	depsDevClient := deps.NewDepsDevClient("https://api.deps.dev", cfg.HTTPClient)
	osvClient := deps.NewOSVClient("https://api.osv.dev", cfg.HTTPClient)
	githubToken := os.Getenv("GITHUB_TOKEN")
	githubClient := deps.NewGitHubEnricher("https://api.github.com", cfg.HTTPClient, githubToken)

	// Fetch package info to get version
	pkgInfo, err := depsDevClient.GetPackage(ctx, cfg.Module)
	if err != nil {
		cfg.Logger.Error("failed to fetch package info", "error", err)
		fmt.Fprintf(cfg.Stderr, "Error fetching package info: %v\n", err)
		return 1
	}

	if len(pkgInfo.Versions) == 0 {
		fmt.Fprintf(cfg.Stderr, "Error: no versions found for module %s\n", cfg.Module)
		return 1
	}

	targetVersion := pkgInfo.Versions[0].VersionKey.Version

	// Check cache
	cache := deps.NewVerdictCache(eventStore)
	if cached, hit, err := cache.Lookup(ctx, cfg.Module, targetVersion, policyHash); err != nil {
		cfg.Logger.Warn("cache lookup failed", "error", err)
	} else if hit {
		return renderVerdict(cfg, *cached, true)
	}

	// Cache miss - run AssembleVerdict
	verdict, err := deps.AssembleVerdict(ctx, deps.Deps{
		DepsDev: depsDevClient,
		OSV:     osvClient,
		GitHub:  githubClient,
	}, cfg.Module, targetVersion, cfg.Config.Intelligence.DepsRisk, policyHash, graph)
	if err != nil {
		cfg.Logger.Error("verdict assembly failed", "error", err)
		fmt.Fprintf(cfg.Stderr, "Error assembling verdict: %v\n", err)
		return 1
	}

	// Store in cache
	if err := cache.Store(ctx, verdict, policyHash); err != nil {
		cfg.Logger.Warn("cache store failed", "error", err)
	}

	// Check if we should block for high-risk
	isTTY := isTerminal(cfg.Stdin)
	decision := deps.ShouldBlock(verdict, isTTY)

	if decision.Block {
		if decision.Prompt {
			return handleInteractiveApproval(ctx, cfg, verdict, policyHash)
		} else {
			pcs := deps.NewPendingCheckpointStore(eventStore)
			_, err := pcs.WritePending(ctx, verdict, policyHash)
			if err != nil {
				cfg.Logger.Error("failed to write pending checkpoint", "error", err)
				fmt.Fprintf(cfg.Stderr, "Error: failed to record checkpoint: %v\n", err)
				return 1
			}
			fmt.Fprintf(cfg.Stderr, "High-risk dependency finding for %s@%s\n", verdict.Module, verdict.Version)
			fmt.Fprintf(cfg.Stderr, "Risk class: %s\n", verdict.RiskClass)
			fmt.Fprintf(cfg.Stderr, "Run 'm31a deps check --approve %s' to approve\n", verdict.Module)
			return 2
		}
	}

	return renderVerdict(cfg, verdict, false)
}

func runDepsApprove(ctx context.Context, cfg DepsCLIConfig) int {
	m31aDir := filepath.Join(cfg.WorkDir, ".m31a")
	eventsDB := filepath.Join(m31aDir, "events.db")
	if _, err := os.Stat(eventsDB); os.IsNotExist(err) {
		fmt.Fprintf(cfg.Stderr, "Error: no event store found at %s\n", eventsDB)
		return 1
	}

	store, err := eventstore.NewEventStore(eventsDB)
	if err != nil {
		fmt.Fprintf(cfg.Stderr, "Error: failed to open event store: %v\n", err)
		return 1
	}
	defer store.Close()

	pcs := deps.NewPendingCheckpointStore(store)
	pending, err := pcs.IsPending(ctx, cfg.Approve)
	if err != nil {
		fmt.Fprintf(cfg.Stderr, "Error checking pending: %v\n", err)
		return 1
	}
	if !pending {
		fmt.Fprintf(cfg.Stderr, "Error: no pending checkpoint for module %s\n", cfg.Approve)
		return 1
	}

	err = pcs.ApprovePending(ctx, cfg.Approve)
	if err != nil {
		fmt.Fprintf(cfg.Stderr, "Error approving: %v\n", err)
		return 1
	}

	fmt.Fprintf(cfg.Stderr, "Approved pending checkpoint for %s\n", cfg.Approve)
	return 0
}

func printUsage(stderr *os.File) {
	fmt.Fprintln(stderr, "Usage: m31a deps check <module> [--format table|json]")
	fmt.Fprintln(stderr, "       m31a deps check --approve <module>")
	fmt.Fprintln(stderr, "  --format FMT    Output format: table, json (default: table)")
	fmt.Fprintln(stderr, "  --approve MOD   Approve pending high-risk checkpoint for module")
	fmt.Fprintln(stderr, "")
	fmt.Fprintln(stderr, "Exit codes:")
	fmt.Fprintln(stderr, "  0  success")
	fmt.Fprintln(stderr, "  1  usage error or user cancelled")
	fmt.Fprintln(stderr, "  2  high-risk finding blocked (headless) — use --approve to resolve")
}

func renderVerdict(cfg DepsCLIConfig, verdict deps.Verdict, cached bool) int {
	switch cfg.Format {
	case "json":
		enc := json.NewEncoder(cfg.Stdout)
		enc.SetIndent("", "  ")
		if cached {
			type verdictWithCache struct {
				deps.Verdict
				CachedAt string `json:"cached_at,omitempty"`
			}
			vc := verdictWithCache{Verdict: verdict, CachedAt: time.Now().UTC().Format(time.RFC3339)}
			if err := enc.Encode(vc); err != nil {
				fmt.Fprintf(cfg.Stderr, "Error encoding JSON: %v\n", err)
				return 1
			}
		} else {
			if err := enc.Encode(verdict); err != nil {
				fmt.Fprintf(cfg.Stderr, "Error encoding JSON: %v\n", err)
				return 1
			}
		}
	case "table":
		if cached {
			fmt.Fprintf(cfg.Stderr, "(cached at %s)\n", time.Now().UTC().Format(time.RFC3339))
		}
		fmt.Fprintf(cfg.Stdout, "Module:       %s\n", verdict.Module)
		fmt.Fprintf(cfg.Stdout, "Version:      %s\n", verdict.Version)
		fmt.Fprintf(cfg.Stdout, "Exists:       %v\n", verdict.Exists)
		if verdict.Exists {
			fmt.Fprintf(cfg.Stdout, "Age:          %d days\n", verdict.AgeDays)
			fmt.Fprintf(cfg.Stdout, "Latest:       %s\n", verdict.LatestRelease)
			fmt.Fprintf(cfg.Stdout, "Maintenance:  %s\n", verdict.MaintenanceClass)
			fmt.Fprintf(cfg.Stdout, "License:      %s\n", verdict.License)
			fmt.Fprintf(cfg.Stdout, "Risk Class:   %s\n", verdict.RiskClass)
			fmt.Fprintf(cfg.Stdout, "Confidence:   %s\n", verdict.Confidence)
			if len(verdict.Vulnerabilities) > 0 {
				fmt.Fprintf(cfg.Stdout, "Vulnerabilities:\n")
				for _, v := range verdict.Vulnerabilities {
					fmt.Fprintf(cfg.Stdout, "  - %s: %s\n", v.ID, v.Summary)
				}
			}
			if len(verdict.TransitiveImpact) > 0 {
				fmt.Fprintf(cfg.Stdout, "Transitive Impact (%d dependents):\n", len(verdict.TransitiveImpact))
				for _, d := range verdict.TransitiveImpact {
					fmt.Fprintf(cfg.Stdout, "  - %s\n", d)
				}
			}
			fmt.Fprintf(cfg.Stdout, "Sources:\n")
			for _, s := range verdict.Sources {
				fmt.Fprintf(cfg.Stdout, "  - %s: %s (at %s)\n", s.Name, s.Status, s.FetchedAt)
			}
		}
	}
	return 0
}

func handleInteractiveApproval(ctx context.Context, cfg DepsCLIConfig, verdict deps.Verdict, policyHash string) int {
	fmt.Fprintf(cfg.Stderr, "\n⚠ High-risk dependency finding for %s@%s\n", verdict.Module, verdict.Version)
	fmt.Fprintf(cfg.Stderr, "Risk class: %s\n", verdict.RiskClass)
	fmt.Fprintf(cfg.Stderr, "Confidence: %s\n", verdict.Confidence)
	if len(verdict.Vulnerabilities) > 0 {
		fmt.Fprintf(cfg.Stderr, "Vulnerabilities:\n")
		for _, v := range verdict.Vulnerabilities {
			fmt.Fprintf(cfg.Stderr, "  - %s: %s\n", v.ID, v.Summary)
		}
	}
	fmt.Fprintf(cfg.Stderr, "\nApprove this dependency? [y/N] ")

	var response string
	fmt.Fscanln(cfg.Stdin, &response)
	response = strings.ToLower(strings.TrimSpace(response))

	if response != "y" && response != "yes" {
		fmt.Fprintf(cfg.Stderr, "Cancelled\n")
		return 1
	}

	if cfg.WorkDir != "" {
		m31aDir := filepath.Join(cfg.WorkDir, ".m31a")
		eventsDB := filepath.Join(m31aDir, "events.db")
		if _, err := os.Stat(eventsDB); err == nil {
			store, err := eventstore.NewEventStore(eventsDB)
			if err == nil {
				defer store.Close()
				pcs := deps.NewPendingCheckpointStore(store)
				_, _ = pcs.WritePending(ctx, verdict, policyHash)
				_ = pcs.ApprovePending(ctx, verdict.Module)
			}
		}
	}

	return renderVerdict(cfg, verdict, false)
}

func computePolicyHash(cfg *config.Config) string {
	data, err := json.Marshal(struct {
		DepsRisk   config.DepsRiskConfig   `json:"deps_risk"`
		DepsPolicy config.DepsPolicyConfig `json:"deps_policy"`
	}{
		DepsRisk:   cfg.Intelligence.DepsRisk,
		DepsPolicy: cfg.Intelligence.DepsPolicy,
	})
	if err != nil {
		return fmt.Sprintf("%x", sha256.Sum256([]byte("fallback")))[:16]
	}
	return fmt.Sprintf("%x", sha256.Sum256(data))[:16]
}

func isTerminal(f *os.File) bool {
	fi, err := f.Stat()
	if err != nil {
		return false
	}
	return (fi.Mode() & os.ModeCharDevice) != 0
}
