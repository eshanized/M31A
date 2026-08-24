package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"time"

	"github.com/eshanized/M31A/internal/integrations/codeintel"
)

// ProgressEvent represents a JSON progress event for the index command.
type ProgressEvent struct {
	Phase      string `json:"phase"`
	FilesDone  int    `json:"files_done"`
	TotalFiles int    `json:"total_files"`
	CurrentFile string `json:"current_file"`
}

// runIndex handles the `m31a index` command.
func runIndex(args []string, workDir string, logger *slog.Logger) int {
	fs := flag.NewFlagSet("index", flag.ExitOnError)
	incremental := fs.Bool("incremental", false, "Only re-index changed files")
	progress := fs.Bool("progress", false, "Emit JSON progress events for scripting")
	fs.Usage = func() {
		fmt.Fprintln(os.Stderr, "Usage: m31a index [--incremental] [--progress]")
		fmt.Fprintln(os.Stderr, "  --incremental  Only re-index changed files")
		fmt.Fprintln(os.Stderr, "  --progress     Emit JSON progress for scripting")
	}
	fs.Parse(args)

	startTime := time.Now()

	// Create indexer
	indexer := codeintel.NewIndexerWithStore(workDir, nil)

	var err error
	if *incremental {
		err = indexer.BuildIncremental(context.Background())
	} else {
		err = indexer.Build(context.Background())
	}

	if err != nil {
		logger.Error("index build failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		return 1
	}

	elapsed := time.Since(startTime)

	// Emit progress events if requested
	if *progress {
		// For simplicity, emit a single progress event with final counts
		// A more sophisticated implementation would emit events during the build
		event := ProgressEvent{
			Phase:      "complete",
			FilesDone:  indexer.FileCount(),
			TotalFiles: indexer.FileCount(),
			CurrentFile: "",
		}
		data, _ := json.Marshal(event)
		fmt.Println(string(data))
	}

	// Print summary
	fmt.Printf("Indexed %d files, %d symbols in %dms\n",
		indexer.FileCount(), indexer.SymbolCount(), elapsed.Milliseconds())

	return 0
}