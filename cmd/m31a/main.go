package main

import (
	"fmt"
	"os"
	"runtime"

	"github.com/eshanized/M31A/internal/log"
)

var Version = "dev"

func main() {
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

	fmt.Printf("M31A %s — AI coding assistant\n", Version)
	fmt.Println("Run 'make build' to build. Implementation coming in Phase 1+.")
}
