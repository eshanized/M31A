package context

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/integrations/git"
)

// DateTimeSource provides the current date and time.
type DateTimeSource struct{}

func (DateTimeSource) Key() string { return "core/datetime" }
func (DateTimeSource) Load(_ context.Context) (string, error) {
	return time.Now().Format("2006-01-02 15:04:05 MST"), nil
}
func (DateTimeSource) Render(value string) string {
	return fmt.Sprintf("Current date and time: %s", value)
}
func (DateTimeSource) RenderUpdate(_, newVal string) string {
	return fmt.Sprintf("Updated time: %s", newVal)
}
func (DateTimeSource) RenderRemoval(_ string) string { return "" }

// EnvironmentSource provides working directory, platform, and shell info.
type EnvironmentSource struct {
	WorkDir string
}

func (EnvironmentSource) Key() string { return "core/environment" }
func (e EnvironmentSource) Load(_ context.Context) (string, error) {
	shell := os.Getenv("SHELL")
	if shell == "" {
		if runtime.GOOS == "windows" {
			shell = os.Getenv("COMSPEC")
			if shell == "" {
				shell = "cmd"
			}
		} else {
			shell = "/bin/bash"
		}
	}
	platform := fmt.Sprintf("%s/%s", runtime.GOOS, runtime.GOARCH)
	return fmt.Sprintf("cwd: %s\nshell: %s\nplatform: %s", e.WorkDir, shell, platform), nil
}
func (EnvironmentSource) Render(value string) string {
	return fmt.Sprintf("Environment:\n%s", value)
}
func (EnvironmentSource) RenderUpdate(_, newVal string) string {
	return fmt.Sprintf("Environment updated:\n%s", newVal)
}
func (EnvironmentSource) RenderRemoval(_ string) string { return "" }

// GitSource provides the current branch and last commit.
type GitSource struct {
	WorkDir string
}

func (GitSource) Key() string { return "core/git" }
func (g GitSource) Load(_ context.Context) (string, error) {
	gitClient := git.New(g.WorkDir)
	branch, err := gitClient.Run("branch", "--show-current")
	if err != nil {
		return "", err
	}
	lastCommit, _ := gitClient.Run("log", "-1", "--format=%h %s")
	return fmt.Sprintf("branch: %s\nlast_commit: %s", strings.TrimSpace(branch), strings.TrimSpace(lastCommit)), nil
}
func (GitSource) Render(value string) string {
	return fmt.Sprintf("Git:\n%s", value)
}
func (GitSource) RenderUpdate(_, newVal string) string {
	return fmt.Sprintf("Git updated:\n%s", newVal)
}
func (GitSource) RenderRemoval(_ string) string { return "" }

// InstructionsSource discovers and renders AGENTS.md files.
type InstructionsSource struct {
	ProjectRoot string
	WorkDir     string
}

func (InstructionsSource) Key() string { return "core/instructions" }
func (i InstructionsSource) Load(_ context.Context) (string, error) {
	root := i.ProjectRoot
	if root == "" {
		root = i.WorkDir
	}

	var parts []string

	// Global AGENTS.md
	if homeDir, err := os.UserHomeDir(); err == nil {
		p := filepath.Join(homeDir, ".m31a", "AGENTS.md")
		if data, err := os.ReadFile(p); err == nil {
			parts = append(parts, strings.TrimSpace(string(data)))
		}
	}

	// Project AGENTS.md (walk from root to workDir)
	dir := i.WorkDir
	for {
		p := filepath.Join(dir, "AGENTS.md")
		if data, err := os.ReadFile(p); err == nil {
			parts = append([]string{strings.TrimSpace(string(data))}, parts...)
		}
		if dir == root || dir == filepath.Dir(dir) {
			break
		}
		dir = filepath.Dir(dir)
	}

	if len(parts) == 0 {
		return "", fmt.Errorf("no AGENTS.md files found")
	}
	return strings.Join(parts, "\n\n"), nil
}
func (InstructionsSource) Render(value string) string {
	return fmt.Sprintf("# Project Instructions\n\n%s", value)
}
func (InstructionsSource) RenderUpdate(_, newVal string) string {
	return fmt.Sprintf("Project instructions updated:\n\n%s", newVal)
}
func (InstructionsSource) RenderRemoval(_ string) string {
	return "Project instructions are no longer available."
}
