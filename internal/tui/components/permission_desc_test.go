package components

import (
	"testing"

	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
)

func TestGenerateDescription_Bash(t *testing.T) {
	tests := []struct {
		name           string
		cmd            string
		phase          string
		goal           string
		expectAction   string
		expectConseq   string
		expectRisk     string
	}{
		{
			name:         "rm command",
			cmd:          "rm -rf node_modules",
			expectAction: "delete files",
			expectConseq: "Deleted files are removed from disk. Recovery may not be possible.",
			expectRisk:   "Permanent file deletion",
		},
		{
			name:         "ls command",
			cmd:          "ls -la",
			expectAction: "list or inspect files",
			expectConseq: "This is a read-only operation. No changes will be made.",
		},
		{
			name:         "git commit",
			cmd:          "git commit -m 'fix: bug'",
			expectAction: "create a git commit",
			expectConseq: "A new commit will be created with staged changes.",
		},
		{
			name:         "git push",
			cmd:          "git push origin main",
			expectAction: "push to a remote repository",
			expectConseq: "Commits will be sent to the remote repository.",
			expectRisk:   "Remote changes",
		},
		{
			name:         "npm install",
			cmd:          "npm install express",
			expectAction: "run a package manager command",
			expectConseq: "Package managers can install, update, or remove dependencies and scripts.",
		},
		{
			name:         "empty command",
			cmd:          "",
			expectAction: "run a shell command",
			expectConseq: "Shell commands can modify files, install packages, or affect system state.",
		},
		{
			name:         "with phase",
			cmd:          "ls -la",
			phase:        "execute",
			expectAction: "[Execute] list or inspect files",
		},
		{
			name:         "with goal",
			cmd:          "ls -la",
			goal:         "implement user authentication",
			expectConseq: "This is a read-only operation. No changes will be made. (Goal: implement user authentication)",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := tools.PermissionRequest{
				ToolName:  "Bash",
				Command:   tt.cmd,
				RiskLevel: types.RiskMedium,
			}
			desc := GenerateDescription(req, tt.phase, tt.goal)

			if tt.expectAction != "" && desc.Action != tt.expectAction {
				t.Errorf("Action = %q, want %q", desc.Action, tt.expectAction)
			}
			if tt.expectConseq != "" && desc.Consequence != tt.expectConseq {
				t.Errorf("Consequence = %q, want %q", desc.Consequence, tt.expectConseq)
			}
			if tt.expectRisk != "" && desc.RiskSummary != tt.expectRisk {
				t.Errorf("RiskSummary = %q, want %q", desc.RiskSummary, tt.expectRisk)
			}
		})
	}
}

func TestGenerateDescription_FileTools(t *testing.T) {
	tests := []struct {
		name         string
		tool         string
		cmd          string
		expectAction string
	}{
		{
			name:         "FileWrite",
			tool:         "FileWrite",
			cmd:          "FileWrite src/main.go",
			expectAction: "write to main.go",
		},
		{
			name:         "FileRead",
			tool:         "FileRead",
			cmd:          "FileRead README.md",
			expectAction: "read README.md",
		},
		{
			name:         "Edit",
			tool:         "Edit",
			cmd:          "Edit src/auth.go",
			expectAction: "modify auth.go",
		},
		{
			name:         "FileDelete",
			tool:         "FileDelete",
			cmd:          "FileDelete old_file.txt",
			expectAction: "delete old_file.txt",
		},
		{
			name:         "Glob",
			tool:         "Glob",
			cmd:          "Glob **/*.go",
			expectAction: "find files matching **/*.go",
		},
		{
			name:         "Grep",
			tool:         "Grep",
			cmd:          "Grep TODO",
			expectAction: "search file contents",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := tools.PermissionRequest{
				ToolName:  tt.tool,
				Command:   tt.cmd,
				RiskLevel: types.RiskSafe,
			}
			desc := GenerateDescription(req, "", "")

			if desc.Action != tt.expectAction {
				t.Errorf("Action = %q, want %q", desc.Action, tt.expectAction)
			}
			if desc.Consequence == "" {
				t.Error("Consequence should not be empty")
			}
		})
	}
}

func TestGenerateDescription_UnknownTool(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "UnknownTool",
		Command:   "some command",
		RiskLevel: types.RiskSafe,
	}
	desc := GenerateDescription(req, "", "")

	if desc.Action != "use the UnknownTool tool" {
		t.Errorf("Action = %q, want %q", desc.Action, "use the UnknownTool tool")
	}
	if desc.Consequence == "" {
		t.Error("Consequence should not be empty for unknown tool")
	}
}

func TestGenerateDescription_PhaseContext(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "go test ./...",
		RiskLevel: types.RiskSafe,
	}

	tests := []struct {
		phase        string
		expectPrefix string
	}{
		{"execute", "[Execute]"},
		{"plan", "[Plan]"},
		{"verify", "[Verify]"},
		{"idle", ""},
		{"", ""},
	}

	for _, tt := range tests {
		t.Run(tt.phase, func(t *testing.T) {
			desc := GenerateDescription(req, tt.phase, "")
			if tt.expectPrefix != "" {
				if len(desc.Action) < len(tt.expectPrefix) || desc.Action[:len(tt.expectPrefix)] != tt.expectPrefix {
					t.Errorf("Action = %q, want prefix %q", desc.Action, tt.expectPrefix)
				}
			} else {
				if len(desc.Action) > 0 && desc.Action[0] == '[' {
					t.Errorf("Action should not have phase prefix for phase %q, got %q", tt.phase, desc.Action)
				}
			}
		})
	}
}

func TestGenerateDescription_GoalContext(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "go test ./...",
		RiskLevel: types.RiskSafe,
	}

	goal := "implement user authentication system"
	desc := GenerateDescription(req, "", goal)

	if desc.Consequence == "" {
		t.Error("Consequence should not be empty")
	}
	if len(desc.Consequence) < len(goal) {
		t.Errorf("Consequence should include goal, got %q", desc.Consequence)
	}
}

func TestGenerateDescription_TruncatesLongGoal(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "go test ./...",
		RiskLevel: types.RiskSafe,
	}

	// Create a goal longer than 60 characters
	longGoal := "This is a very long goal that exceeds sixty characters and should be truncated"
	desc := GenerateDescription(req, "", longGoal)

	if desc.Consequence == "" {
		t.Error("Consequence should not be empty")
	}
	// The goal should be truncated
	if len(desc.Consequence) < 60 {
		t.Errorf("Consequence seems too short: %q", desc.Consequence)
	}
}

func TestDescribeBash_SpecificCommands(t *testing.T) {
	tests := []struct {
		cmd          string
		expectAction string
		expectRisk   string
	}{
		{"rm -rf /", "delete files", "Permanent file deletion"},
		{"rmdir empty_dir", "delete a directory", "Directory removal"},
		{"mv old.txt new.txt", "move or rename files", ""},
		{"cp src dst", "copy files", ""},
		{"mkdir new_dir", "create a directory", ""},
		{"touch new_file", "create or modify a file", ""},
		{"cat file.txt", "read file contents", ""},
		{"git commit", "create a git commit", ""},
		{"git push", "push to a remote repository", "Remote changes"},
		{"git pull", "fetch from a remote repository", ""},
		{"git checkout branch", "switch branches", ""},
		{"git merge feature", "merge a branch", ""},
		{"git rebase main", "rebase a branch", "History rewrite"},
		{"git reset --hard", "reset to a commit", "May discard changes"},
		{"git stash", "stash changes", ""},
		{"git add .", "stage files for commit", ""},
		{"git rm file", "remove files from git", "File deletion"},
		{"git clean -fd", "clean untracked files", "Permanent file deletion"},
		{"npm install", "run a package manager command", ""},
		{"go build", "run a Go command", ""},
		{"cargo build", "run a Cargo command", ""},
		{"python script.py", "run a Python command", ""},
		{"docker run", "run a Docker command", "Container operations"},
		{"curl http://example.com", "make an HTTP request", ""},
		{"ssh user@host", "connect to a remote system", "Remote access"},
		{"sudo rm /etc/hosts", "run a command with elevated privileges", "Elevated privileges"},
		{"chmod 755 file", "change file permissions", ""},
		{"echo hello", "output text", ""},
		{"cd /tmp", "change directory", ""},
	}

	for _, tt := range tests {
		t.Run(tt.cmd, func(t *testing.T) {
			desc := describeBash(tt.cmd)
			if desc.Action != tt.expectAction {
				t.Errorf("Action = %q, want %q", desc.Action, tt.expectAction)
			}
			if tt.expectRisk != "" && desc.RiskSummary != tt.expectRisk {
				t.Errorf("RiskSummary = %q, want %q", desc.RiskSummary, tt.expectRisk)
			}
			if desc.Consequence == "" {
				t.Error("Consequence should not be empty")
			}
		})
	}
}

func TestDescribeGitCommand_Subcommands(t *testing.T) {
	tests := []struct {
		subcmd       string
		expectAction string
	}{
		{"commit", "create a git commit"},
		{"push", "push to a remote repository"},
		{"pull", "fetch from a remote repository"},
		{"fetch", "fetch from a remote repository"},
		{"checkout", "switch branches"},
		{"switch", "switch branches"},
		{"merge", "merge a branch"},
		{"rebase", "rebase a branch"},
		{"reset", "reset to a commit"},
		{"stash", "stash changes"},
		{"add", "stage files for commit"},
		{"rm", "remove files from git"},
		{"clean", "clean untracked files"},
		{"status", "run git status"},
		{"log", "run git log"},
		{"diff", "run git diff"},
	}

	for _, tt := range tests {
		t.Run(tt.subcmd, func(t *testing.T) {
			desc := describeGitCommand([]string{tt.subcmd})
			if desc.Action != tt.expectAction {
				t.Errorf("Action = %q, want %q", desc.Action, tt.expectAction)
			}
		})
	}
}

func TestDescribeGitCommand_Empty(t *testing.T) {
	desc := describeGitCommand([]string{})
	if desc.Action != "run a git command" {
		t.Errorf("Action = %q, want %q", desc.Action, "run a git command")
	}
}

func TestDescribeDevServer_Actions(t *testing.T) {
	tests := []struct {
		cmd          string
		expectAction string
	}{
		{"start", "start a development server"},
		{"stop", "stop the development server"},
		{"restart", "restart the development server"},
		{"logs", "view server logs"},
		{"status", "manage the development server"},
		{"", "manage the development server"},
	}

	for _, tt := range tests {
		t.Run(tt.cmd, func(t *testing.T) {
			desc := describeDevServer(tt.cmd)
			if desc.Action != tt.expectAction {
				t.Errorf("Action = %q, want %q", desc.Action, tt.expectAction)
			}
		})
	}
}

func TestTruncateText(t *testing.T) {
	tests := []struct {
		input    string
		max      int
		expected string
	}{
		{"short", 10, "short"},
		{"this is a longer string", 10, "this is..."},
		{"", 10, ""},
		{"exact", 5, "exact"},
		{"toolong", 5, "to..."},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result := truncateText(tt.input, tt.max)
			if result != tt.expected {
				t.Errorf("truncateText(%q, %d) = %q, want %q", tt.input, tt.max, result, tt.expected)
			}
		})
	}
}

func TestCapitalizeFirst(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"hello", "Hello"},
		{"Hello", "Hello"},
		{"", ""},
		{"a", "A"},
		{"EXECUTE", "EXECUTE"},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result := capitalizeFirst(tt.input)
			if result != tt.expected {
				t.Errorf("capitalizeFirst(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}

func TestDescribeRmTarget(t *testing.T) {
	tests := []struct {
		args     []string
		expected string
	}{
		{[]string{"-rf", "node_modules"}, "node_modules"},
		{[]string{"file1.txt", "file2.txt"}, "file1.txt file2.txt"},
		{[]string{"-rf"}, ""},
		{[]string{}, ""},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			result := describeRmTarget(tt.args)
			if result != tt.expected {
				t.Errorf("describeRmTarget(%v) = %q, want %q", tt.args, result, tt.expected)
			}
		})
	}
}

func TestDescribeMvTarget(t *testing.T) {
	tests := []struct {
		args     []string
		expected string
	}{
		{[]string{"old.txt", "new.txt"}, "old.txt -> new.txt"},
		{[]string{"-f", "old.txt", "new.txt"}, "old.txt -> new.txt"},
		{[]string{"single.txt"}, "single.txt"},
		{[]string{}, ""},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			result := describeMvTarget(tt.args)
			if result != tt.expected {
				t.Errorf("describeMvTarget(%v) = %q, want %q", tt.args, result, tt.expected)
			}
		})
	}
}

func TestDescribeUrlTarget(t *testing.T) {
	tests := []struct {
		args     []string
		expected string
	}{
		{[]string{"-s", "https://example.com"}, "https://example.com"},
		{[]string{"http://test.com", "output.html"}, "http://test.com"},
		{[]string{"no-url"}, "no-url"},
		{[]string{}, ""},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			result := describeUrlTarget(tt.args)
			if result != tt.expected {
				t.Errorf("describeUrlTarget(%v) = %q, want %q", tt.args, result, tt.expected)
			}
		})
	}
}

func TestDescribeRemoteTarget(t *testing.T) {
	tests := []struct {
		args     []string
		expected string
	}{
		{[]string{"user@host:/path"}, "user@host:/path"},
		{[]string{"-p", "22", "user@host"}, "user@host"},
		{[]string{"host:path"}, "host:path"},
		{[]string{"no-remote"}, "no-remote"},
		{[]string{}, ""},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			result := describeRemoteTarget(tt.args)
			if result != tt.expected {
				t.Errorf("describeRemoteTarget(%v) = %q, want %q", tt.args, result, tt.expected)
			}
		})
	}
}

func TestDescribeSedTarget(t *testing.T) {
	tests := []struct {
		args     []string
		expected string
	}{
		{[]string{"-i", "'s/foo/bar/'", "file.txt"}, "'s/foo/bar/' file.txt"},
		{[]string{"'s/foo/bar/'", "file.txt"}, "'s/foo/bar/' file.txt"},
		{[]string{}, ""},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			result := describeSedTarget(tt.args)
			if result != tt.expected {
				t.Errorf("describeSedTarget(%v) = %q, want %q", tt.args, result, tt.expected)
			}
		})
	}
}

func TestDescribeGeneric(t *testing.T) {
	desc := describeGeneric("CustomTool", "arg1 arg2")
	if desc.Action != "use the CustomTool tool" {
		t.Errorf("Action = %q, want %q", desc.Action, "use the CustomTool tool")
	}
	if desc.Target != "arg1 arg2" {
		t.Errorf("Target = %q, want %q", desc.Target, "arg1 arg2")
	}
	if desc.Consequence == "" {
		t.Error("Consequence should not be empty")
	}
}

func TestDescribeTodo(t *testing.T) {
	tests := []struct {
		tool         string
		cmd          string
		expectAction string
	}{
		{"TodoWrite", "add task", "update TODO list"},
		{"TodoRead", "read tasks", "read TODO list"},
	}

	for _, tt := range tests {
		t.Run(tt.tool, func(t *testing.T) {
			desc := describeTodo(tt.tool, tt.cmd)
			if desc.Action != tt.expectAction {
				t.Errorf("Action = %q, want %q", desc.Action, tt.expectAction)
			}
		})
	}
}

func TestDescribeWebFetch(t *testing.T) {
	desc := describeWebFetch("https://example.com")
	if desc.Action != "fetch content from a URL" {
		t.Errorf("Action = %q, want %q", desc.Action, "fetch content from a URL")
	}
	if desc.Consequence == "" {
		t.Error("Consequence should not be empty")
	}
}

func TestDescribeWebSearch(t *testing.T) {
	desc := describeWebSearch("golang generics")
	if desc.Action != "search the web" {
		t.Errorf("Action = %q, want %q", desc.Action, "search the web")
	}
}

func TestDescribeHTTPCheck(t *testing.T) {
	desc := describeHTTPCheck("GET https://api.example.com/health")
	if desc.Action != "make an HTTP request" {
		t.Errorf("Action = %q, want %q", desc.Action, "make an HTTP request")
	}
}

func TestDescribeCodeMap(t *testing.T) {
	desc := describeCodeMap("upstream src/auth.go")
	if desc.Action != "analyze code structure" {
		t.Errorf("Action = %q, want %q", desc.Action, "analyze code structure")
	}
}

func TestDescribeAgent(t *testing.T) {
	desc := describeAgent("build")
	if desc.Action != "spawn a sub-agent" {
		t.Errorf("Action = %q, want %q", desc.Action, "spawn a sub-agent")
	}
	if desc.RiskSummary != "Sub-agent execution" {
		t.Errorf("RiskSummary = %q, want %q", desc.RiskSummary, "Sub-agent execution")
	}
}

func TestDescribeFileWrite(t *testing.T) {
	desc := describeFileWrite("FileWrite src/main.go")
	if desc.Action != "write to main.go" {
		t.Errorf("Action = %q, want %q", desc.Action, "write to main.go")
	}
	if desc.Target != "src/main.go" {
		t.Errorf("Target = %q, want %q", desc.Target, "src/main.go")
	}
}

func TestDescribeFileRead(t *testing.T) {
	desc := describeFileRead("FileRead README.md")
	if desc.Action != "read README.md" {
		t.Errorf("Action = %q, want %q", desc.Action, "read README.md")
	}
}

func TestDescribeEdit(t *testing.T) {
	desc := describeEdit("Edit src/auth.go")
	if desc.Action != "modify auth.go" {
		t.Errorf("Action = %q, want %q", desc.Action, "modify auth.go")
	}
}

func TestDescribeFileDelete(t *testing.T) {
	desc := describeFileDelete("FileDelete old_file.txt")
	if desc.Action != "delete old_file.txt" {
		t.Errorf("Action = %q, want %q", desc.Action, "delete old_file.txt")
	}
	if desc.RiskSummary != "Permanent file deletion" {
		t.Errorf("RiskSummary = %q, want %q", desc.RiskSummary, "Permanent file deletion")
	}
}

func TestDescribeFileMove(t *testing.T) {
	desc := describeFileMove("FileMove old.txt new.txt")
	if desc.Action != "move old.txt to new.txt" {
		t.Errorf("Action = %q, want %q", desc.Action, "move old.txt to new.txt")
	}
}

func TestDescribeGlob(t *testing.T) {
	desc := describeGlob("Glob **/*.go")
	if desc.Action != "find files matching **/*.go" {
		t.Errorf("Action = %q, want %q", desc.Action, "find files matching **/*.go")
	}
}

func TestDescribeGrep(t *testing.T) {
	desc := describeGrep("Grep TODO")
	if desc.Action != "search file contents" {
		t.Errorf("Action = %q, want %q", desc.Action, "search file contents")
	}
}

func TestGenerateDescription_EmptyCommand(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "",
		RiskLevel: types.RiskSafe,
	}
	desc := GenerateDescription(req, "", "")

	if desc.Action == "" {
		t.Error("Action should not be empty even for empty command")
	}
	if desc.Consequence == "" {
		t.Error("Consequence should not be empty")
	}
}

func TestGenerateDescription_AllRiskLevels(t *testing.T) {
	// Risk summary comes from the command description, not the risk level parameter.
	// The rm command always has "Permanent file deletion" as risk summary.
	tests := []struct {
		risk       types.RiskLevel
		expectRisk string
	}{
		{types.RiskSafe, "Permanent file deletion"},
		{types.RiskMedium, "Permanent file deletion"},
		{types.RiskDangerous, "Permanent file deletion"},
		{types.RiskDestructive, "Permanent file deletion"},
	}

	for _, tt := range tests {
		t.Run(string(tt.risk), func(t *testing.T) {
			req := tools.PermissionRequest{
				ToolName:  "Bash",
				Command:   "rm -rf /",
				RiskLevel: tt.risk,
			}
			desc := GenerateDescription(req, "", "")
			if desc.RiskSummary != tt.expectRisk {
				t.Errorf("RiskSummary = %q, want %q", desc.RiskSummary, tt.expectRisk)
			}
		})
	}
}
