package workflow

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/git"
)

// ShipPreflightResult holds the outcome of the pre-ship checklist.
type ShipPreflightResult struct {
	Passed bool
	Issues []string
}

// runShipPreflight checks for common issues before the final commit.
func (e *Engine) runShipPreflight(tasks []m31types.Task) ShipPreflightResult {
	result := ShipPreflightResult{Passed: true}

	// Collect all files touched by tasks
	var allFiles []string
	seen := make(map[string]bool)
	for _, task := range tasks {
		for _, f := range task.Files {
			if !seen[f] {
				allFiles = append(allFiles, f)
				seen[f] = true
			}
		}
	}

	for _, f := range allFiles {
		fullPath := filepath.Join(e.workDir, f)
		data, err := os.ReadFile(fullPath)
		if err != nil {
			continue
		}
		content := string(data)

		// Check for TODO/FIXME/HACK markers in changed files
		markers := []string{"TODO", "FIXME", "HACK", "XXX", "BROKEN"}
		for _, marker := range markers {
			if strings.Contains(content, marker) {
				result.Issues = append(result.Issues,
					fmt.Sprintf("%s contains %s marker", f, marker))
			}
		}

		// Check for debug statements
		debugPatterns := []string{
			"console.log(", "console.debug(", "console.warn(",
			"print(", "puts ", "fmt.Println(", "fmt.Printf(",
			"debugger;", "binding.pry", "import pdb",
			"log.Println(", "log.Printf(",
		}
		for _, pattern := range debugPatterns {
			if strings.Contains(content, pattern) {
				result.Issues = append(result.Issues,
					fmt.Sprintf("%s contains debug statement: %s", f, pattern))
			}
		}

		// Check for hardcoded secrets (simple heuristic)
		secretPatterns := []struct {
			pattern string
			desc    string
		}{
			{"sk-", "OpenAI/Stripe API key prefix"},
			{"ghp_", "GitHub personal access token"},
			{"glpat-", "GitLab personal access token"},
			{"xoxb-", "Slack bot token"},
			{"AKIA", "AWS access key prefix"},
		}
		for _, sp := range secretPatterns {
			if strings.Contains(content, sp.pattern) {
				result.Issues = append(result.Issues,
					fmt.Sprintf("%s may contain hardcoded secret (%s)", f, sp.desc))
				result.Passed = false
			}
		}
	}

	if len(result.Issues) > 0 && result.Passed {
		// Warnings only (TODO/FIXME/debug) — not blocking
		result.Passed = true
	}

	return result
}

// generateChangelog creates changelog entries from task descriptions and commits.
func (e *Engine) generateChangelog(tasks []m31types.Task, commits []git.CommitInfo) string {
	var sb strings.Builder

	sb.WriteString("## Changelog\n\n")

	// Group tasks by category
	categories := make(map[string][]m31types.Task)
	for _, task := range tasks {
		cat := task.Category
		if cat == "" {
			cat = "General"
		}
		categories[cat] = append(categories[cat], task)
	}

	// Write entries grouped by category
	for cat, catTasks := range categories {
		sb.WriteString(fmt.Sprintf("### %s\n\n", cat))
		for _, task := range catTasks {
			icon := "✨"
			switch {
			case task.Status == m31types.StatusDone:
				icon = "✅"
			case task.Status == m31types.StatusFailed:
				icon = "❌"
			case task.Status == m31types.StatusSkipped:
				icon = "⏭️"
			}

			actionLower := strings.ToLower(task.Action)
			prefix := "Changed"
			if actionLower == "create" || actionLower == "add" {
				prefix = "Added"
			} else if actionLower == "delete" || actionLower == "remove" {
				prefix = "Removed"
			} else if actionLower == "fix" {
				prefix = "Fixed"
			}

			sb.WriteString(fmt.Sprintf("- %s **%s** %s", icon, prefix, task.Description))
			if len(task.Files) > 0 {
				sb.WriteString(fmt.Sprintf(" (`%s`)", strings.Join(task.Files[:min(3, len(task.Files))], "`, `")))
			}
			sb.WriteString("\n")
		}
		sb.WriteString("\n")
	}

	// Commits section
	if len(commits) > 0 {
		sb.WriteString("### Commits\n\n")
		for _, c := range commits {
			hash := c.Hash
			if len(hash) > 7 {
				hash = hash[:7]
			}
			sb.WriteString(fmt.Sprintf("- `%s` %s\n", hash, c.Message))
		}
	}

	return sb.String()
}
