package tools

import (
	"context"
	"fmt"
	"os/exec"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
)

// Compile-time interface check
var _ types.Tool = (*Git)(nil)

// Git provides structured git operations as a tool. Rather than exposing raw
// shell commands, it parses output into structured types the LLM can reason
// about: branch lists, commit histories, diff summaries, and status reports.
type Git struct {
	workDir string
}

// NewGit creates a Git tool rooted at the given working directory.
func NewGit(workDir string) *Git {
	return &Git{workDir: workDir}
}

func (g *Git) Name() string               { return "Git" }
func (g *Git) RiskLevel() types.RiskLevel { return types.RiskDangerous }

func (g *Git) Description() string {
	return `Execute git operations with structured output parsing.
Provides safe, structured access to common git commands: add, commit, diff, log, branch, checkout, stash, and status.

Parameters:
- operation (required): One of: add, commit, diff, log, branch, checkout, stash, status
- args (optional): Operation-specific arguments as a string (e.g., "main", "-m 'fix bug'")
- paths (optional): File paths for operations that accept paths (add, commit, diff)`
}

func (g *Git) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"operation": {
				"type": "string",
				"description": "Git operation to perform",
				"enum": ["add", "commit", "diff", "log", "branch", "checkout", "stash", "status"]
			},
			"args": {
				"type": "string",
				"description": "Operation arguments (e.g., '-m \"fix bug\"', 'main', '--stat')"
			},
			"paths": {
				"type": "array",
				"items": {"type": "string"},
				"description": "File paths for operations (add, commit, diff)"
			}
		},
		"required": ["operation"]
	}`
}

// gitResult is the structured output returned by git operations.
type gitResult struct {
	Success   bool   `json:"success"`
	Operation string `json:"operation"`
	Output    string `json:"output,omitempty"`
	Error     string `json:"error,omitempty"`
}

// validateGitArgs checks that all arguments are in the allowlist for the given operation.
func validateGitArgs(operation string, args []string) error {
	allowed := map[string]map[string]bool{
		"add":      {"-f": true, "-p": true, "-v": true, "-n": true, "--dry-run": true, "--verbose": true},
		"commit":   {"-m": true, "-a": true, "--amend": true, "--no-edit": true, "--allow-empty": true, "--allow-empty-message": true},
		"diff":     {"--stat": true, "--name-only": true, "--name-status": true, "--cached": true, "--staged": true},
		"log":      {"--oneline": true, "--graph": true, "--all": true, "--stat": true, "-n": true, "--pretty": true},
		"branch":   {"-d": true, "-D": true, "-m": true, "-M": true, "-r": true, "-a": true, "--list": true},
		"checkout": {"-b": true, "-B": true, "--orphan": true},
		"stash":    {"push": true, "pop": true, "apply": true, "drop": true, "list": true, "show": true},
		"status":   {"--short": true, "--branch": true, "--porcelain": true},
	}

	allowList, ok := allowed[operation]
	if !ok {
		return fmt.Errorf("unknown git operation: %s", operation)
	}

	for i, arg := range args {
		if !allowList[arg] {
			// For commit with -m, treat everything after -m as message text
			if operation == "commit" && arg == "-m" {
				break // remaining args are the commit message
			}
			// For commit, skip tokens that follow -m (they are message words)
			if operation == "commit" && i > 0 {
				prevIsDashM := false
				for j := i - 1; j >= 0; j-- {
					if args[j] == "-m" {
						prevIsDashM = true
						break
					}
				}
				if prevIsDashM {
					continue
				}
			}
			return fmt.Errorf("argument %q is not allowed for git %s operation: %w", arg, operation, m31errors.ErrToolExecution)
		}
	}
	return nil
}

func (g *Git) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	operation, ok := input.Params["operation"].(string)
	if !ok || operation == "" {
		return types.ToolResult{}, fmt.Errorf("missing required parameter: operation: %w", m31errors.ErrToolExecution)
	}

	args, _ := input.Params["args"].(string)
	var paths []string
	if p, ok := input.Params["paths"].([]any); ok {
		for _, v := range p {
			if s, ok := v.(string); ok {
				paths = append(paths, s)
			}
		}
	}

	var result gitResult
	var err error

	// Validate args against per-operation allowlists
	if args != "" {
		parsedArgs := strings.Fields(args)
		if valErr := validateGitArgs(operation, parsedArgs); valErr != nil {
			return types.ToolResult{}, valErr
		}
	}

	switch operation {
	case "add":
		result, err = g.gitAdd(ctx, paths, args)
	case "commit":
		result, err = g.gitCommit(ctx, args, paths)
	case "diff":
		result, err = g.gitDiff(ctx, args, paths)
	case "log":
		result, err = g.gitLog(ctx, args)
	case "branch":
		result, err = g.gitBranch(ctx, args)
	case "checkout":
		result, err = g.gitCheckout(ctx, args)
	case "stash":
		result, err = g.gitStash(ctx, args)
	case "status":
		result, err = g.gitStatus(ctx)
	default:
		return types.ToolResult{}, fmt.Errorf("unknown operation %q: %w", operation, m31errors.ErrToolExecution)
	}

	if err != nil {
		return types.ToolResult{}, err
	}

	output := result.Output
	if result.Error != "" {
		output = result.Error
	}

	return types.ToolResult{
		Output:     output,
		DurationMs: time.Since(start).Milliseconds(),
		Error:      result.Error,
	}, nil
}

// runGit executes a git command in the working directory and returns combined output.
func (g *Git) runGit(ctx context.Context, args ...string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Dir = g.workDir
	out, err := cmd.CombinedOutput()
	return strings.TrimSpace(string(out)), err
}

// gitAdd stages files. If paths is empty and args contains ".", stages all.
func (g *Git) gitAdd(ctx context.Context, paths []string, args string) (gitResult, error) {
	var cmdArgs []string
	cmdArgs = append(cmdArgs, "add")

	if len(paths) > 0 {
		cmdArgs = append(cmdArgs, paths...)
	} else if args != "" {
		cmdArgs = append(cmdArgs, strings.Fields(args)...)
	} else {
		cmdArgs = append(cmdArgs, ".")
	}

	output, err := g.runGit(ctx, cmdArgs...)
	if err != nil {
		return gitResult{
			Success:   false,
			Operation: "add",
			Error:     fmt.Sprintf("git add failed: %v\n%s", err, output),
		}, nil
	}

	// Verify staging
	status, _ := g.runGit(ctx, "status", "--porcelain")
	stagedCount := 0
	for _, line := range strings.Split(status, "\n") {
		if len(line) >= 2 && line[0] != '?' && line[1] != '?' {
			stagedCount++
		}
	}

	return gitResult{
		Success:   true,
		Operation: "add",
		Output:    fmt.Sprintf("Staged %d file(s).\n\n%s", stagedCount, status),
	}, nil
}

// gitCommit creates a commit with the given message.
func (g *Git) gitCommit(ctx context.Context, args string, paths []string) (gitResult, error) {
	// Extract -m message from args
	msg := extractCommitMessage(args)
	if msg == "" {
		return gitResult{
			Success:   false,
			Operation: "commit",
			Error:     "commit requires a message. Use args: '-m \"your message\"'",
		}, nil
	}

	var cmdArgs []string
	cmdArgs = append(cmdArgs, "commit", "-m", msg)

	// If paths are specified, commit only those files
	if len(paths) > 0 {
		cmdArgs = append(cmdArgs, "--")
		cmdArgs = append(cmdArgs, paths...)
	}

	output, err := g.runGit(ctx, cmdArgs...)
	if err != nil {
		return gitResult{
			Success:   false,
			Operation: "commit",
			Error:     fmt.Sprintf("git commit failed: %v\n%s", err, output),
		}, nil
	}

	return gitResult{
		Success:   true,
		Operation: "commit",
		Output:    output,
	}, nil
}

// gitDiff shows changes. Supports --stat, --cached, and file paths.
func (g *Git) gitDiff(ctx context.Context, args string, paths []string) (gitResult, error) {
	var cmdArgs []string
	cmdArgs = append(cmdArgs, "diff")

	if args != "" {
		cmdArgs = append(cmdArgs, strings.Fields(args)...)
	}

	if len(paths) > 0 {
		cmdArgs = append(cmdArgs, "--")
		cmdArgs = append(cmdArgs, paths...)
	}

	output, err := g.runGit(ctx, cmdArgs...)
	if err != nil {
		return gitResult{
			Success:   false,
			Operation: "diff",
			Error:     fmt.Sprintf("git diff failed: %v\n%s", err, output),
		}, nil
	}

	if output == "" {
		output = "No changes."
	}

	return gitResult{
		Success:   true,
		Operation: "diff",
		Output:    output,
	}, nil
}

// gitLog shows commit history. Defaults to last 20 commits.
func (g *Git) gitLog(ctx context.Context, args string) (gitResult, error) {
	var cmdArgs []string
	cmdArgs = append(cmdArgs, "log", "--oneline", "--decorate", "-20")

	if args != "" {
		// Override if user provides their own count
		cmdArgs = append(cmdArgs, strings.Fields(args)...)
	}

	output, err := g.runGit(ctx, cmdArgs...)
	if err != nil {
		return gitResult{
			Success:   false,
			Operation: "log",
			Error:     fmt.Sprintf("git log failed: %v\n%s", err, output),
		}, nil
	}

	if output == "" {
		output = "No commits yet."
	}

	return gitResult{
		Success:   true,
		Operation: "log",
		Output:    output,
	}, nil
}

// gitBranch manages branches. Without args: lists branches. With a name: creates branch.
func (g *Git) gitBranch(ctx context.Context, args string) (gitResult, error) {
	fields := strings.Fields(args)

	if len(fields) == 0 {
		// List branches
		output, err := g.runGit(ctx, "branch", "-a")
		if err != nil {
			return gitResult{
				Success:   false,
				Operation: "branch",
				Error:     fmt.Sprintf("git branch failed: %v\n%s", err, output),
			}, nil
		}
		return gitResult{
			Success:   true,
			Operation: "branch",
			Output:    output,
		}, nil
	}

	// Create or delete branch
	if fields[0] == "-d" || fields[0] == "--delete" {
		if len(fields) < 2 {
			return gitResult{
				Success:   false,
				Operation: "branch",
				Error:     "delete requires a branch name",
			}, nil
		}
		output, err := g.runGit(ctx, "branch", "-d", fields[1])
		if err != nil {
			return gitResult{
				Success:   false,
				Operation: "branch",
				Error:     fmt.Sprintf("git branch delete failed: %v\n%s", err, output),
			}, nil
		}
		return gitResult{
			Success:   true,
			Operation: "branch",
			Output:    output,
		}, nil
	}

	// Create branch
	output, err := g.runGit(ctx, "branch", fields[0])
	if err != nil {
		return gitResult{
			Success:   false,
			Operation: "branch",
			Error:     fmt.Sprintf("git branch create failed: %v\n%s", err, output),
		}, nil
	}
	return gitResult{
		Success:   true,
		Operation: "branch",
		Output:    fmt.Sprintf("Created branch '%s'.", fields[0]),
	}, nil
}

// gitCheckout switches branches or restores files.
func (g *Git) gitCheckout(ctx context.Context, args string) (gitResult, error) {
	fields := strings.Fields(args)
	if len(fields) == 0 {
		return gitResult{
			Success:   false,
			Operation: "checkout",
			Error:     "checkout requires a branch name or path. Use args: 'main' or args: '-- path/to/file'",
		}, nil
	}

	var cmdArgs []string
	cmdArgs = append(cmdArgs, "checkout")
	cmdArgs = append(cmdArgs, fields...)

	output, err := g.runGit(ctx, cmdArgs...)
	if err != nil {
		return gitResult{
			Success:   false,
			Operation: "checkout",
			Error:     fmt.Sprintf("git checkout failed: %v\n%s", err, output),
		}, nil
	}

	return gitResult{
		Success:   true,
		Operation: "checkout",
		Output:    output,
	}, nil
}

// gitStash manages stashes. Without args: lists stashes. With "push": creates stash.
func (g *Git) gitStash(ctx context.Context, args string) (gitResult, error) {
	fields := strings.Fields(args)

	if len(fields) == 0 || fields[0] == "list" {
		output, err := g.runGit(ctx, "stash", "list")
		if err != nil {
			return gitResult{
				Success:   false,
				Operation: "stash",
				Error:     fmt.Sprintf("git stash list failed: %v\n%s", err, output),
			}, nil
		}
		if output == "" {
			output = "No stash entries."
		}
		return gitResult{
			Success:   true,
			Operation: "stash",
			Output:    output,
		}, nil
	}

	if fields[0] == "push" || fields[0] == "save" {
		output, err := g.runGit(ctx, "stash", "push", "-m", "tool-stash")
		if err != nil {
			return gitResult{
				Success:   false,
				Operation: "stash",
				Error:     fmt.Sprintf("git stash push failed: %v\n%s", err, output),
			}, nil
		}
		return gitResult{
			Success:   true,
			Operation: "stash",
			Output:    output,
		}, nil
	}

	if fields[0] == "pop" {
		output, err := g.runGit(ctx, "stash", "pop")
		if err != nil {
			return gitResult{
				Success:   false,
				Operation: "stash",
				Error:     fmt.Sprintf("git stash pop failed: %v\n%s", err, output),
			}, nil
		}
		return gitResult{
			Success:   true,
			Operation: "stash",
			Output:    output,
		}, nil
	}

	if fields[0] == "drop" {
		output, err := g.runGit(ctx, "stash", "drop")
		if err != nil {
			return gitResult{
				Success:   false,
				Operation: "stash",
				Error:     fmt.Sprintf("git stash drop failed: %v\n%s", err, output),
			}, nil
		}
		return gitResult{
			Success:   true,
			Operation: "stash",
			Output:    output,
		}, nil
	}

	return gitResult{
		Success:   false,
		Operation: "stash",
		Error:     fmt.Sprintf("unknown stash subcommand: %s. Use: list, push, pop, or drop", fields[0]),
	}, nil
}

// gitStatus shows working tree status with structured information.
func (g *Git) gitStatus(ctx context.Context) (gitResult, error) {
	// Get porcelain status for machine-readable output
	output, err := g.runGit(ctx, "status", "--porcelain")
	if err != nil {
		return gitResult{
			Success:   false,
			Operation: "status",
			Error:     fmt.Sprintf("git status failed: %v\n%s", err, output),
		}, nil
	}

	// Parse into categories
	var staged, modified, untracked, deleted []string
	for _, line := range strings.Split(output, "\n") {
		if len(line) < 2 {
			continue
		}
		statusCode := line[:2]
		file := strings.TrimSpace(line[3:])

		switch {
		case statusCode[0] == '?' && statusCode[1] == '?':
			untracked = append(untracked, file)
		case statusCode[0] == 'D' || statusCode[1] == 'D':
			deleted = append(deleted, file)
		case statusCode[0] != ' ' && statusCode[0] != '?':
			staged = append(staged, file)
		case statusCode[1] != ' ' && statusCode[1] != '?':
			modified = append(modified, file)
		}
	}

	var sb strings.Builder
	sb.WriteString("Git Status:\n")

	if len(staged) > 0 {
		fmt.Fprintf(&sb, "\nStaged (%d):\n", len(staged))
		for _, f := range staged {
			fmt.Fprintf(&sb, "  + %s\n", f)
		}
	}
	if len(modified) > 0 {
		fmt.Fprintf(&sb, "\nModified (%d):\n", len(modified))
		for _, f := range modified {
			fmt.Fprintf(&sb, "  ~ %s\n", f)
		}
	}
	if len(deleted) > 0 {
		fmt.Fprintf(&sb, "\nDeleted (%d):\n", len(deleted))
		for _, f := range deleted {
			fmt.Fprintf(&sb, "  - %s\n", f)
		}
	}
	if len(untracked) > 0 {
		fmt.Fprintf(&sb, "\nUntracked (%d):\n", len(untracked))
		for _, f := range untracked {
			fmt.Fprintf(&sb, "  ? %s\n", f)
		}
	}

	if len(staged) == 0 && len(modified) == 0 && len(deleted) == 0 && len(untracked) == 0 {
		sb.WriteString("Working tree clean.\n")
	}

	// Add branch info
	branch, err := g.runGit(ctx, "branch", "--show-current")
	if err == nil && branch != "" {
		fmt.Fprintf(&sb, "\nOn branch: %s", branch)
	}

	return gitResult{
		Success:   true,
		Operation: "status",
		Output:    sb.String(),
	}, nil
}

// extractCommitMessage parses a commit message from args like "-m 'fix bug'" or "-m \"fix bug\"".
func extractCommitMessage(args string) string {
	fields := strings.Fields(args)
	for i, f := range fields {
		if f == "-m" && i+1 < len(fields) {
			// Collect all remaining tokens after -m as the full commit message
			msg := strings.Join(fields[i+1:], " ")
			// Strip surrounding quotes
			if len(msg) >= 2 {
				if (msg[0] == '"' && msg[len(msg)-1] == '"') ||
					(msg[0] == '\'' && msg[len(msg)-1] == '\'') {
					msg = msg[1 : len(msg)-1]
				}
			}
			return msg
		}
	}
	// If no -m flag, treat entire args as message
	if args != "" {
		msg := strings.TrimSpace(args)
		if len(msg) >= 2 {
			if (msg[0] == '"' && msg[len(msg)-1] == '"') ||
				(msg[0] == '\'' && msg[len(msg)-1] == '\'') {
				msg = msg[1 : len(msg)-1]
			}
		}
		return msg
	}
	return ""
}
