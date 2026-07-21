package repl

import (
	"os"
	"path/filepath"
	"strings"
)

// detectProjectLanguage inspects top-level files in cwd and returns the dominant
// programming language, or an empty string if none is detected.
func detectProjectLanguage(cwd string) string {
	if cwd == "" {
		return ""
	}
	entries, err := os.ReadDir(cwd)
	if err != nil {
		return ""
	}
	counts := make(map[string]int)
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		ext := strings.ToLower(filepath.Ext(e.Name()))
		switch ext {
		case ".go":
			counts["Go"]++
		case ".ts", ".tsx":
			counts["TypeScript"]++
		case ".js", ".jsx", ".mjs":
			counts["JavaScript"]++
		case ".py":
			counts["Python"]++
		case ".rs":
			counts["Rust"]++
		case ".java":
			counts["Java"]++
		case ".kt", ".kts":
			counts["Kotlin"]++
		case ".rb":
			counts["Ruby"]++
		case ".c", ".h":
			counts["C"]++
		case ".cpp", ".hpp", ".cc", ".cxx":
			counts["C++"]++
		case ".cs":
			counts["C#"]++
		case ".swift":
			counts["Swift"]++
		case ".zig":
			counts["Zig"]++
		}
	}
	best, bestCount := "", 0
	for lang, count := range counts {
		if count > bestCount {
			best, bestCount = lang, count
		}
	}
	return best
}
