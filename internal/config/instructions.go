package config

import (
	"os"
	"path/filepath"
	"strings"
)

// InstructionFile represents a discovered AGENTS.md file.
type InstructionFile struct {
	Path    string
	Content string
}

// DiscoverInstructions walks from workDir up to projectRoot collecting AGENTS.md
// files. Also checks the global config directory (~/.m31a/AGENTS.md).
// Returns files ordered from outermost (project root) to innermost (workDir).
func DiscoverInstructions(projectRoot, workDir string) []InstructionFile {
	var files []InstructionFile

	homeDir, err := os.UserHomeDir()
	if err == nil {
		globalPath := filepath.Join(homeDir, ".m31a", "AGENTS.md")
		if content, err := os.ReadFile(globalPath); err == nil {
			text := strings.TrimSpace(string(content))
			if text != "" {
				files = append(files, InstructionFile{Path: globalPath, Content: text})
			}
		}
	}

	// Walk from project root downward to workDir
	if projectRoot == "" {
		projectRoot = workDir
	}

	absRoot, err := filepath.Abs(projectRoot)
	if err != nil {
		absRoot = projectRoot
	}
	absWork, err := filepath.Abs(workDir)
	if err != nil {
		absWork = workDir
	}

	var dirs []string
	dir := absWork
	for {
		dirs = append([]string{dir}, dirs...)
		if dir == absRoot || dir == filepath.Dir(dir) {
			break
		}
		dir = filepath.Dir(dir)
	}

	for _, d := range dirs {
		p := filepath.Join(d, "AGENTS.md")
		if content, err := os.ReadFile(p); err == nil {
			text := strings.TrimSpace(string(content))
			if text != "" {
				files = append(files, InstructionFile{Path: p, Content: text})
			}
		}
	}

	return files
}

// RenderInstructions concatenates discovered instruction files into a single
// string suitable for injection into the system prompt.
func RenderInstructions(files []InstructionFile) string {
	if len(files) == 0 {
		return ""
	}
	var sb strings.Builder
	sb.WriteString("# Project Instructions\n\n")
	for _, f := range files {
		sb.WriteString("<!-- Source: ")
		sb.WriteString(f.Path)
		sb.WriteString(" -->\n")
		sb.WriteString(f.Content)
		sb.WriteString("\n\n")
	}
	return strings.TrimSpace(sb.String())
}
