package main

import (
	"flag"
	"fmt"
	"os"

	"github.com/eshanized/M31A/internal/ui/tui"
)

// printUsage prints the usage information including dynamically generated
// slash command list from the command registry.
func printUsage(registry *tui.CommandRegistry) {
	fmt.Fprintf(os.Stderr, "M31A — Terminal AI Coding Agent\n\n")
	fmt.Fprintf(os.Stderr, "Usage: m31a [flags]\n\n")
	fmt.Fprintf(os.Stderr, "A terminal-based AI coding agent that guides tasks through a\n")
	fmt.Fprintf(os.Stderr, "seven-phase workflow:\n\n")
	fmt.Fprintf(os.Stderr, "  Initialize — Gather project context and environment info\n")
	fmt.Fprintf(os.Stderr, "  Discuss    — Clarify requirements and scope with the user\n")
	fmt.Fprintf(os.Stderr, "  Plan       — Build a structured task plan with dependencies\n")
	fmt.Fprintf(os.Stderr, "  Execute    — Implement tasks using tools with self-healing\n")
	fmt.Fprintf(os.Stderr, "  Verify     — Run tests and validate correctness\n")
	fmt.Fprintf(os.Stderr, "  Runtime    — Dev server lifecycle and smoke tests\n")
	fmt.Fprintf(os.Stderr, "  Ship       — Commit changes and finalize the session\n\n")
	fmt.Fprintf(os.Stderr, "Flags:\n")
	flag.PrintDefaults()
	fmt.Fprintf(os.Stderr, "\nSlash Commands (available in the TUI):\n")

	cmds := registry.AllCommands()
	// Filter out aliases (plan, execute, verify, ship are aliases of /phase)
	aliases := map[string]bool{"plan": true, "execute": true, "verify": true, "ship": true}
	var filtered []tui.CommandInfo
	for _, cmd := range cmds {
		if !aliases[cmd.Name] {
			filtered = append(filtered, cmd)
		}
	}

	// Print in 4 columns
	cols := 4
	for i := 0; i < len(filtered); i += cols {
		end := i + cols
		if end > len(filtered) {
			end = len(filtered)
		}
		for _, cmd := range filtered[i:end] {
			fmt.Fprintf(os.Stderr, "  %-14s", cmd.Slash)
		}
		fmt.Fprintf(os.Stderr, "\n")
	}

	fmt.Fprintf(os.Stderr, "\nNote: command chaining with ';' is not supported. Use each command separately.\n")

	fmt.Fprintf(os.Stderr, "\nEnvironment Variables:\n")
	fmt.Fprintf(os.Stderr, "  M31A_CONFIG              Config file path (default: ~/.m31a/config.toml)\n")
	fmt.Fprintf(os.Stderr, "  M31A_OPENROUTER_API_KEY  OpenRouter API key\n")
	fmt.Fprintf(os.Stderr, "  M31A_ZEN_API_KEY         Zen API key\n")
	fmt.Fprintf(os.Stderr, "  M31A_NVIDIA_API_KEY      NVIDIA API key\n")
	fmt.Fprintf(os.Stderr, "  M31A_LOG_FORMAT          Log format: json, text (default: json)\n")
	fmt.Fprintf(os.Stderr, "  M31A_LOG_LEVEL           Log level: debug, info, warn, error (default: info)\n")
}
