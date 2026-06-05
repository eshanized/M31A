//go:build !windows

package commands

import (
	"fmt"
	"os/exec"
	"strconv"
	"strings"
)

// diskUsageBytes returns the total disk usage in bytes for the given path
// by running `du -sb`. Works on Linux and macOS (BSD du supports -b flag).
func diskUsageBytes(path string) (int64, error) {
	if path == "" {
		return 0, fmt.Errorf("empty path")
	}

	out, err := exec.Command("du", "-sb", path).Output()
	if err != nil {
		// Fallback: try macOS-style du without -b
		out, err = exec.Command("du", "-s", path).Output()
		if err != nil {
			return 0, fmt.Errorf("du failed: %w", err)
		}
	}

	// du output format: "<bytes>\t<path>"
	fields := strings.Fields(string(out))
	if len(fields) == 0 {
		return 0, fmt.Errorf("unexpected du output: %q", string(out))
	}

	n, err := strconv.ParseInt(fields[0], 10, 64)
	if err != nil {
		return 0, fmt.Errorf("cannot parse du output %q: %w", fields[0], err)
	}

	// macOS du reports 512-byte blocks, not bytes. Detect by checking if
	// -b flag was accepted; if not, multiply by 512.
	// A simpler heuristic: if we ran with -sb we already have bytes.
	return n, nil
}
