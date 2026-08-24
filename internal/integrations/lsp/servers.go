package lsp

import (
	"os/exec"
	"path/filepath"
	"strings"
)

// ServerConfig holds the configuration for a language server.
type ServerConfig struct {
	Language   string
	Command    []string
	Extensions []string
	Binary     string
}

// defaultServers maps language identifiers to their server configurations.
var defaultServers = map[string]ServerConfig{
	"go": {
		Language:   "go",
		Command:    []string{"gopls", "serve"},
		Extensions: []string{".go"},
		Binary:     "gopls",
	},
	"typescript": {
		Language:   "typescript",
		Command:    []string{"typescript-language-server", "--stdio"},
		Extensions: []string{".ts", ".tsx", ".js", ".jsx"},
		Binary:     "typescript-language-server",
	},
	"python": {
		Language:   "python",
		Command:    []string{"pyright-langserver", "--stdio"},
		Extensions: []string{".py"},
		Binary:     "pyright-langserver",
	},
	"rust": {
		Language:   "rust",
		Command:    []string{"rust-analyzer"},
		Extensions: []string{".rs"},
		Binary:     "rust-analyzer",
	},
}

// ServerConfigForLanguage returns the server configuration for the given language.
func ServerConfigForLanguage(lang string) (*ServerConfig, bool) {
	config, ok := defaultServers[lang]
	if !ok {
		return nil, false
	}
	return &config, true
}

// FindLanguageForFile determines the language based on the file extension.
func FindLanguageForFile(path string) (string, bool) {
	ext := strings.ToLower(filepath.Ext(path))
	for lang, config := range defaultServers {
		for _, e := range config.Extensions {
			if ext == e {
				return lang, true
			}
		}
	}
	return "", false
}

// BinaryExists checks if the language server binary is available in PATH.
func BinaryExists(binary string) bool {
	_, err := exec.LookPath(binary)
	return err == nil
}

// SupportedLanguages returns a list of all supported language identifiers.
func SupportedLanguages() []string {
	langs := make([]string, 0, len(defaultServers))
	for lang := range defaultServers {
		langs = append(langs, lang)
	}
	return langs
}

// SupportedExtensions returns a list of all supported file extensions.
func SupportedExtensions() []string {
	var exts []string
	for _, config := range defaultServers {
		exts = append(exts, config.Extensions...)
	}
	return exts
}