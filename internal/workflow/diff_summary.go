package workflow

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/eshanized/M31A/internal/git"
	m31types "github.com/eshanized/M31A/pkg/types"
)

// CaptureDiffSummary computes a diff summary between two git commit hashes.
// Uses git diff --numstat for structured additions/deletions data.
func CaptureDiffSummary(g *git.Git, beforeHash, afterHash string) (*m31types.DiffSummary, error) {
	if g == nil {
		return nil, fmt.Errorf("git instance required for diff summary")
	}

	out, err := g.Run("diff", "--numstat", beforeHash+".."+afterHash)
	if err != nil {
		return nil, fmt.Errorf("git diff --numstat: %w", err)
	}

	statusOut, err := g.Run("diff", "--name-status", beforeHash+".."+afterHash)
	if err != nil {
		// Log the error but continue with empty status map.
		// The diff summary will still show additions/deletions from numstat.
		statusOut = ""
	}

	summary := &m31types.DiffSummary{}
	statusMap := parseNameStatus(statusOut)

	for _, line := range strings.Split(strings.TrimSpace(out), "\n") {
		if line == "" {
			continue
		}
		parts := strings.Fields(line)
		if len(parts) < 3 {
			continue
		}

		additions := 0
		deletions := 0
		if parts[0] != "-" {
			additions, _ = strconv.Atoi(parts[0])
		}
		if parts[1] != "-" {
			deletions, _ = strconv.Atoi(parts[1])
		}

		file := parts[2]
		status := "modified"
		if s, ok := statusMap[file]; ok {
			status = s
		}

		summary.Files = append(summary.Files, m31types.FileDiff{
			File:      file,
			Status:    status,
			Additions: additions,
			Deletions: deletions,
		})
		summary.Additions += additions
		summary.Deletions += deletions
	}

	return summary, nil
}

// parseNameStatus parses git diff --name-status output into a file→status map.
func parseNameStatus(out string) map[string]string {
	result := make(map[string]string)
	for _, line := range strings.Split(strings.TrimSpace(out), "\n") {
		if line == "" {
			continue
		}
		parts := strings.Fields(line)
		if len(parts) < 2 {
			continue
		}
		file := parts[1]
		switch parts[0][0] {
		case 'A':
			result[file] = "added"
		case 'D':
			result[file] = "deleted"
		case 'M':
			result[file] = "modified"
		case 'R':
			result[file] = "renamed"
		default:
			result[file] = "modified"
		}
	}
	return result
}

// FormatDiffSummary renders a human-readable diff summary.
func FormatDiffSummary(ds *m31types.DiffSummary) string {
	if ds == nil || len(ds.Files) == 0 {
		return "No file changes."
	}

	var sb strings.Builder
	fmt.Fprintf(&sb, "Files changed: %d (+%d/-%d)\n",
		len(ds.Files), ds.Additions, ds.Deletions)

	for _, f := range ds.Files {
		fmt.Fprintf(&sb, "  %s %s (+%d/-%d)\n",
			statusIcon(f.Status), f.File, f.Additions, f.Deletions)
	}

	return sb.String()
}

func statusIcon(status string) string {
	switch status {
	case "added":
		return "[+]"
	case "deleted":
		return "[-]"
	case "renamed":
		return "[R]"
	default:
		return "[M]"
	}
}
