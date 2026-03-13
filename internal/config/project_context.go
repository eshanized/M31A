package config

import (
	"os"
	"path/filepath"
)

const maxProjectContextBytes = 8 << 10 // 8 KB

// projectContextFiles lists candidate files in priority order.
var projectContextFiles = []string{
	"AGENTS.md",
	".m31a/agents.md",
	"MEMORY.md",
}

// LoadProjectContext reads the first available project context file from workDir.
// It checks AGENTS.md, .m31a/agents.md, and MEMORY.md in priority order.
// Returns the file content and the path it was loaded from.
// Returns empty string and empty path if no context file is found.
func LoadProjectContext(workDir string) (content string, path string) {
	for _, name := range projectContextFiles {
		full := filepath.Join(workDir, name)
		data, err := os.ReadFile(full)
		if err != nil {
			continue
		}
		if len(data) > maxProjectContextBytes {
			data = data[:maxProjectContextBytes]
		}
		return string(data), full
	}
	return "", ""
}

// ProjectContextMtime returns the modification time of the first available
// project context file. Returns 0 if none found.
func ProjectContextMtime(workDir string) int64 {
	for _, name := range projectContextFiles {
		full := filepath.Join(workDir, name)
		info, err := os.Stat(full)
		if err != nil {
			continue
		}
		return info.ModTime().UnixNano()
	}
	return 0
}
