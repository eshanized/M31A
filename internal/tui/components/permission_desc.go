package components

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/eshanized/M31A/internal/tools"
)

// PermissionDescription holds the plain English description and consequence
// text for a permission request. Generated from tool name + command.
type PermissionDescription struct {
	// Action is the human-readable action, e.g. "delete a file"
	Action string
	// Consequence explains what will happen if allowed
	Consequence string
	// Target is the specific target (file, URL, command, etc.)
	Target string
	// RiskSummary is a one-line risk explanation (empty for safe operations)
	RiskSummary string
}

// GenerateDescription produces a plain English description for a permission
// request. Falls back to a generic description for unknown tools.
func GenerateDescription(req tools.PermissionRequest, phase, goal string) PermissionDescription {
	desc := PermissionDescription{}

	switch req.ToolName {
	case "Bash":
		desc = describeBash(req.Command)
	case "FileWrite":
		desc = describeFileWrite(req.Command)
	case "FileRead":
		desc = describeFileRead(req.Command)
	case "Edit":
		desc = describeEdit(req.Command)
	case "FileDelete":
		desc = describeFileDelete(req.Command)
	case "FileMove":
		desc = describeFileMove(req.Command)
	case "Glob":
		desc = describeGlob(req.Command)
	case "Grep":
		desc = describeGrep(req.Command)
	case "WebFetch":
		desc = describeWebFetch(req.Command)
	case "WebSearch":
		desc = describeWebSearch(req.Command)
	case "TodoWrite", "TodoRead":
		desc = describeTodo(req.ToolName, req.Command)
	case "DevServer":
		desc = describeDevServer(req.Command)
	case "HTTPCheck":
		desc = describeHTTPCheck(req.Command)
	case "CodeMap":
		desc = describeCodeMap(req.Command)
	case "Agent":
		desc = describeAgent(req.Command)
	default:
		desc = describeGeneric(req.ToolName, req.Command)
	}

	// Prepend phase context if available
	if phase != "" && phase != "idle" {
		phaseLabel := capitalizeFirst(phase)
		desc.Action = fmt.Sprintf("[%s] %s", phaseLabel, desc.Action)
	}

	// Append goal snippet if available
	if goal != "" && len(desc.Consequence) > 0 {
		goalSnippet := goal
		if len(goalSnippet) > 60 {
			goalSnippet = goalSnippet[:57] + "..."
		}
		desc.Consequence = fmt.Sprintf("%s (Goal: %s)", desc.Consequence, goalSnippet)
	}

	return desc
}

// describeBash parses a bash command into a plain English description.
func describeBash(cmd string) PermissionDescription {
	cmd = strings.TrimSpace(cmd)
	words := strings.Fields(cmd)
	if len(words) == 0 {
		return PermissionDescription{
			Action:      "run a shell command",
			Consequence: "Shell commands can modify files, install packages, or affect system state.",
		}
	}

	// Extract the base command name
	bin := filepath.Base(words[0])
	args := words[1:]

	switch bin {
	case "rm":
		return PermissionDescription{
			Action:      "delete files",
			Target:      describeRmTarget(args),
			Consequence: "Deleted files are removed from disk. Recovery may not be possible.",
			RiskSummary: "Permanent file deletion",
		}
	case "rmdir":
		return PermissionDescription{
			Action:      "delete a directory",
			Target:      strings.Join(args, " "),
			Consequence: "The directory will be removed from disk.",
			RiskSummary: "Directory removal",
		}
	case "mv":
		return PermissionDescription{
			Action:      "move or rename files",
			Target:      describeMvTarget(args),
			Consequence: "Files will be moved to a new location. Original path will no longer exist.",
		}
	case "cp":
		return PermissionDescription{
			Action:      "copy files",
			Target:      describeCpTarget(args),
			Consequence: "A duplicate of the file(s) will be created at the destination.",
		}
	case "mkdir":
		return PermissionDescription{
			Action:      "create a directory",
			Target:      strings.Join(args, " "),
			Consequence: "A new directory will be created on disk.",
		}
	case "touch":
		return PermissionDescription{
			Action:      "create or modify a file",
			Target:      strings.Join(args, " "),
			Consequence: "A new empty file will be created, or an existing file's timestamp updated.",
		}
	case "cat", "less", "more", "head", "tail":
		return PermissionDescription{
			Action:      "read file contents",
			Target:      strings.Join(args, " "),
			Consequence: "This is a read-only operation. No changes will be made.",
		}
	case "ls", "find", "tree", "du", "df", "wc":
		return PermissionDescription{
			Action:      "list or inspect files",
			Target:      strings.Join(args, " "),
			Consequence: "This is a read-only operation. No changes will be made.",
		}
	case "git":
		return describeGitCommand(args)
	case "npm", "npx", "yarn", "pnpm", "bun":
		return PermissionDescription{
			Action:      "run a package manager command",
			Target:      strings.Join(args, " "),
			Consequence: "Package managers can install, update, or remove dependencies and scripts.",
		}
	case "go":
		return PermissionDescription{
			Action:      "run a Go command",
			Target:      strings.Join(args, " "),
			Consequence: "Go commands can compile, test, or modify Go source files and modules.",
		}
	case "cargo":
		return PermissionDescription{
			Action:      "run a Cargo command",
			Target:      strings.Join(args, " "),
			Consequence: "Cargo can build, test, or modify Rust source files and dependencies.",
		}
	case "python", "python3", "pip", "pip3":
		return PermissionDescription{
			Action:      "run a Python command",
			Target:      strings.Join(args, " "),
			Consequence: "Python commands can execute scripts, install packages, or modify files.",
		}
	case "docker", "docker-compose":
		return PermissionDescription{
			Action:      "run a Docker command",
			Target:      strings.Join(args, " "),
			Consequence: "Docker commands can create, modify, or remove containers and images.",
			RiskSummary: "Container operations",
		}
	case "curl", "wget":
		return PermissionDescription{
			Action:      "make an HTTP request",
			Target:      describeUrlTarget(args),
			Consequence: "Data will be fetched from an external URL. No local files are modified.",
		}
	case "ssh", "scp", "rsync":
		return PermissionDescription{
			Action:      "connect to a remote system",
			Target:      describeRemoteTarget(args),
			Consequence: "This will establish a connection to a remote server.",
			RiskSummary: "Remote access",
		}
	case "sudo":
		return PermissionDescription{
			Action:      "run a command with elevated privileges",
			Target:      strings.Join(args, " "),
			Consequence: "This command runs as root with full system access.",
			RiskSummary: "Elevated privileges",
		}
	case "chmod":
		return PermissionDescription{
			Action:      "change file permissions",
			Target:      strings.Join(args, " "),
			Consequence: "File access permissions will be modified.",
		}
	case "chown":
		return PermissionDescription{
			Action:      "change file ownership",
			Target:      strings.Join(args, " "),
			Consequence: "File ownership will be changed. May require elevated privileges.",
		}
	case "sed", "awk":
		return PermissionDescription{
			Action:      "process text with " + bin,
			Target:      describeSedTarget(args),
			Consequence: "Text processing commands can transform file contents.",
		}
	case "echo":
		return PermissionDescription{
			Action:      "output text",
			Target:      truncateText(strings.Join(args, " "), 60),
			Consequence: "This is typically safe but check for file redirection (>, >>).",
		}
	case "cd":
		return PermissionDescription{
			Action:      "change directory",
			Target:      strings.Join(args, " "),
			Consequence: "The current working directory will be changed.",
		}
	case "export", "set", "unset":
		return PermissionDescription{
			Action:      "set environment variables",
			Target:      strings.Join(args, " "),
			Consequence: "Environment variables will be modified for the current session.",
		}
	default:
		return PermissionDescription{
			Action:      fmt.Sprintf("run the %s command", bin),
			Target:      truncateText(strings.Join(args, " "), 60),
			Consequence: "Shell commands can modify files, install packages, or affect system state.",
		}
	}
}

// describeFileWrite parses a file write command into a description.
func describeFileWrite(cmd string) PermissionDescription {
	cmd = strings.TrimSpace(cmd)
	words := strings.Fields(cmd)
	if len(words) < 2 {
		return PermissionDescription{
			Action:      "write to a file",
			Consequence: "File contents will be replaced or appended.",
		}
	}

	path := words[1]
	return PermissionDescription{
		Action:      fmt.Sprintf("write to %s", filepath.Base(path)),
		Target:      path,
		Consequence: "The file will be created or overwritten with new contents.",
	}
}

// describeFileRead parses a file read command into a description.
func describeFileRead(cmd string) PermissionDescription {
	cmd = strings.TrimSpace(cmd)
	words := strings.Fields(cmd)
	if len(words) < 2 {
		return PermissionDescription{
			Action:      "read a file",
			Consequence: "This is a read-only operation. No changes will be made.",
		}
	}

	path := words[1]
	return PermissionDescription{
		Action:      fmt.Sprintf("read %s", filepath.Base(path)),
		Target:      path,
		Consequence: "This is a read-only operation. No changes will be made.",
	}
}

// describeEdit parses an edit command into a description.
func describeEdit(cmd string) PermissionDescription {
	cmd = strings.TrimSpace(cmd)
	words := strings.Fields(cmd)
	if len(words) < 2 {
		return PermissionDescription{
			Action:      "edit a file",
			Consequence: "The file contents will be modified.",
		}
	}

	path := words[1]
	return PermissionDescription{
		Action:      fmt.Sprintf("modify %s", filepath.Base(path)),
		Target:      path,
		Consequence: "The file contents will be modified in place.",
	}
}

// describeFileDelete parses a file delete command into a description.
func describeFileDelete(cmd string) PermissionDescription {
	cmd = strings.TrimSpace(cmd)
	words := strings.Fields(cmd)
	if len(words) < 2 {
		return PermissionDescription{
			Action:      "delete a file",
			Consequence: "The file will be permanently removed from disk.",
			RiskSummary: "Permanent file deletion",
		}
	}

	path := words[1]
	return PermissionDescription{
		Action:      fmt.Sprintf("delete %s", filepath.Base(path)),
		Target:      path,
		Consequence: "The file will be permanently removed from disk.",
		RiskSummary: "Permanent file deletion",
	}
}

// describeFileMove parses a file move command into a description.
func describeFileMove(cmd string) PermissionDescription {
	cmd = strings.TrimSpace(cmd)
	words := strings.Fields(cmd)
	if len(words) < 3 {
		return PermissionDescription{
			Action:      "move a file",
			Consequence: "The file will be moved to a new location.",
		}
	}

	src := words[1]
	dst := words[2]
	return PermissionDescription{
		Action:      fmt.Sprintf("move %s to %s", filepath.Base(src), filepath.Base(dst)),
		Target:      fmt.Sprintf("%s -> %s", src, dst),
		Consequence: "The file will be moved to a new location. Original path will no longer exist.",
	}
}

// describeGlob parses a glob command into a description.
func describeGlob(cmd string) PermissionDescription {
	cmd = strings.TrimSpace(cmd)
	words := strings.Fields(cmd)
	pattern := "*"
	if len(words) > 1 {
		pattern = words[1]
	}

	return PermissionDescription{
		Action:      fmt.Sprintf("find files matching %s", pattern),
		Consequence: "This is a read-only operation. No changes will be made.",
	}
}

// describeGrep parses a grep command into a description.
func describeGrep(cmd string) PermissionDescription {
	cmd = strings.TrimSpace(cmd)
	return PermissionDescription{
		Action:      "search file contents",
		Target:      truncateText(cmd, 60),
		Consequence: "This is a read-only operation. No changes will be made.",
	}
}

// describeWebFetch parses a web fetch command into a description.
func describeWebFetch(cmd string) PermissionDescription {
	return PermissionDescription{
		Action:      "fetch content from a URL",
		Target:      truncateText(cmd, 60),
		Consequence: "Content will be downloaded from the specified URL. No local files are modified.",
	}
}

// describeWebSearch parses a web search command into a description.
func describeWebSearch(cmd string) PermissionDescription {
	return PermissionDescription{
		Action:      "search the web",
		Target:      truncateText(cmd, 60),
		Consequence: "Search results will be fetched from the web. No local files are modified.",
	}
}

// describeTodo parses a todo command into a description.
func describeTodo(tool, cmd string) PermissionDescription {
	if tool == "TodoWrite" {
		return PermissionDescription{
			Action:      "update TODO list",
			Consequence: "The TODO.md file will be modified.",
		}
	}
	return PermissionDescription{
		Action:      "read TODO list",
		Consequence: "This is a read-only operation. No changes will be made.",
	}
}

// describeDevServer parses a dev server command into a description.
func describeDevServer(cmd string) PermissionDescription {
	cmd = strings.TrimSpace(cmd)
	words := strings.Fields(cmd)
	action := "manage"
	if len(words) > 0 {
		action = words[0]
	}

	switch action {
	case "start":
		return PermissionDescription{
			Action:      "start a development server",
			Consequence: "A background process will be started on a local port.",
		}
	case "stop":
		return PermissionDescription{
			Action:      "stop the development server",
			Consequence: "The background server process will be terminated.",
		}
	case "restart":
		return PermissionDescription{
			Action:      "restart the development server",
			Consequence: "The server will be stopped and started again.",
		}
	case "logs":
		return PermissionDescription{
			Action:      "view server logs",
			Consequence: "This is a read-only operation. No changes will be made.",
		}
	default:
		return PermissionDescription{
			Action:      "manage the development server",
			Consequence: "Server state may be modified.",
		}
	}
}

// describeHTTPCheck parses an HTTP check command into a description.
func describeHTTPCheck(cmd string) PermissionDescription {
	return PermissionDescription{
		Action:      "make an HTTP request",
		Target:      truncateText(cmd, 60),
		Consequence: "An HTTP request will be sent. No local files are modified.",
	}
}

// describeCodeMap parses a code map command into a description.
func describeCodeMap(cmd string) PermissionDescription {
	return PermissionDescription{
		Action:      "analyze code structure",
		Consequence: "This is a read-only analysis operation. No changes will be made.",
	}
}

// describeAgent parses an agent command into a description.
func describeAgent(cmd string) PermissionDescription {
	return PermissionDescription{
		Action:      "spawn a sub-agent",
		Consequence: "A child agent will be created with its own context and tools.",
		RiskSummary: "Sub-agent execution",
	}
}

// describeGeneric produces a fallback description for unknown tools.
func describeGeneric(tool, cmd string) PermissionDescription {
	return PermissionDescription{
		Action:      fmt.Sprintf("use the %s tool", tool),
		Target:      truncateText(cmd, 60),
		Consequence: fmt.Sprintf("The %s tool will be executed with the provided arguments.", tool),
	}
}

// describeGitCommand parses git subcommands into descriptions.
func describeGitCommand(args []string) PermissionDescription {
	if len(args) == 0 {
		return PermissionDescription{
			Action:      "run a git command",
			Consequence: "Git commands can modify repository state.",
		}
	}

	subcmd := args[0]
	switch subcmd {
	case "commit":
		return PermissionDescription{
			Action:      "create a git commit",
			Consequence: "A new commit will be created with staged changes.",
		}
	case "push":
		return PermissionDescription{
			Action:      "push to a remote repository",
			Consequence: "Commits will be sent to the remote repository.",
			RiskSummary: "Remote changes",
		}
	case "pull", "fetch":
		return PermissionDescription{
			Action:      "fetch from a remote repository",
			Consequence: "Remote changes will be downloaded. Local files may be modified.",
		}
	case "checkout", "switch":
		return PermissionDescription{
			Action:      "switch branches",
			Consequence: "Working directory files will be changed to match the target branch.",
		}
	case "merge":
		return PermissionDescription{
			Action:      "merge a branch",
			Consequence: "Branch history will be combined. May modify files.",
		}
	case "rebase":
		return PermissionDescription{
			Action:      "rebase a branch",
			Consequence: "Commit history will be rewritten. May modify files.",
			RiskSummary: "History rewrite",
		}
	case "reset":
		return PermissionDescription{
			Action:      "reset to a commit",
			Consequence: "Working directory or staging area will be modified.",
			RiskSummary: "May discard changes",
		}
	case "stash":
		return PermissionDescription{
			Action:      "stash changes",
			Consequence: "Working directory changes will be saved and reverted.",
		}
	case "add":
		return PermissionDescription{
			Action:      "stage files for commit",
			Consequence: "Files will be added to the staging area.",
		}
	case "rm":
		return PermissionDescription{
			Action:      "remove files from git",
			Consequence: "Files will be removed from both disk and git tracking.",
			RiskSummary: "File deletion",
		}
	case "clean":
		return PermissionDescription{
			Action:      "clean untracked files",
			Consequence: "Untracked files will be removed from disk.",
			RiskSummary: "Permanent file deletion",
		}
	default:
		return PermissionDescription{
			Action:      fmt.Sprintf("run git %s", subcmd),
			Target:      truncateText(strings.Join(args[1:], " "), 60),
			Consequence: fmt.Sprintf("The git %s command may modify repository state.", subcmd),
		}
	}
}

// Helper functions for parsing command arguments.

func describeRmTarget(args []string) string {
	if len(args) == 0 {
		return ""
	}
	targets := []string{}
	for _, a := range args {
		if !strings.HasPrefix(a, "-") {
			targets = append(targets, a)
		}
	}
	if len(targets) == 0 {
		return ""
	}
	result := strings.Join(targets, " ")
	if len(result) > 60 {
		result = result[:57] + "..."
	}
	return result
}

func describeMvTarget(args []string) string {
	nonFlag := []string{}
	for _, a := range args {
		if !strings.HasPrefix(a, "-") {
			nonFlag = append(nonFlag, a)
		}
	}
	if len(nonFlag) < 2 {
		return strings.Join(nonFlag, " ")
	}
	return fmt.Sprintf("%s -> %s", nonFlag[0], nonFlag[1])
}

func describeCpTarget(args []string) string {
	nonFlag := []string{}
	for _, a := range args {
		if !strings.HasPrefix(a, "-") {
			nonFlag = append(nonFlag, a)
		}
	}
	if len(nonFlag) < 2 {
		return strings.Join(nonFlag, " ")
	}
	return fmt.Sprintf("%s -> %s", nonFlag[0], nonFlag[1])
}

func describeUrlTarget(args []string) string {
	for _, a := range args {
		if strings.HasPrefix(a, "http://") || strings.HasPrefix(a, "https://") {
			if len(a) > 60 {
				return a[:57] + "..."
			}
			return a
		}
	}
	if len(args) > 0 {
		return args[len(args)-1]
	}
	return ""
}

func describeRemoteTarget(args []string) string {
	for _, a := range args {
		if strings.Contains(a, "@") || strings.Contains(a, ":") {
			return a
		}
	}
	if len(args) > 0 {
		return args[len(args)-1]
	}
	return ""
}

func describeSedTarget(args []string) string {
	// Skip flags, return remaining args
	nonFlag := []string{}
	for _, a := range args {
		if !strings.HasPrefix(a, "-") {
			nonFlag = append(nonFlag, a)
		}
	}
	result := strings.Join(nonFlag, " ")
	if len(result) > 60 {
		result = result[:57] + "..."
	}
	return result
}

func truncateText(s string, max int) string {
	if len(s) <= max {
		return s
	}
	return s[:max-3] + "..."
}

func capitalizeFirst(s string) string {
	if s == "" {
		return s
	}
	return strings.ToUpper(s[:1]) + s[1:]
}
