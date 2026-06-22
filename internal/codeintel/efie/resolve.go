package efie

import (
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

// resolveImport attempts to map an import path to a local file path.
func resolveImport(workDir, fromFile, importPath, language string, fileSet map[string]bool) string {
	switch language {
	case "go":
		return resolveGoImport(workDir, fromFile, importPath, fileSet)
	case "typescript":
		return resolveTSImport(workDir, fromFile, importPath, fileSet)
	case "python":
		return resolvePyImport(workDir, fromFile, importPath, fileSet)
	case "rust":
		return resolveRustImport(workDir, fromFile, importPath, fileSet)
	}
	return ""
}

func resolveGoImport(workDir, fromFile, importPath string, fileSet map[string]bool) string {
	parts := strings.Split(importPath, "/")
	if len(parts) == 0 {
		return ""
	}
	firstElem := parts[0]
	if !strings.Contains(firstElem, ".") {
		return ""
	}

	if strings.HasPrefix(importPath, "./") || strings.HasPrefix(importPath, "../") {
		fromDir := filepath.Dir(fromFile)
		relPath := filepath.Join(fromDir, importPath)
		candidate := filepath.Join(workDir, relPath)
		if info, err := os.Stat(candidate); err == nil && info.IsDir() {
			return filepath.ToSlash(filepath.Join(relPath, "index.go"))
		}
		if _, err := os.Stat(candidate + ".go"); err == nil {
			return filepath.ToSlash(relPath + ".go")
		}
		return ""
	}

	modPath := readGoModModule(workDir)
	if modPath == "" {
		return ""
	}
	if !strings.HasPrefix(importPath, modPath) {
		return ""
	}

	relPath := strings.TrimPrefix(importPath, modPath)
	relPath = strings.TrimPrefix(relPath, "/")
	if relPath == "" {
		return ""
	}

	candidate := filepath.Join(workDir, relPath)
	if info, err := os.Stat(candidate); err == nil && info.IsDir() {
		return filepath.ToSlash(relPath)
	}
	if _, err := os.Stat(candidate + ".go"); err == nil {
		return filepath.ToSlash(relPath + ".go")
	}
	return ""
}

func resolveTSImport(workDir, fromFile, importPath string, fileSet map[string]bool) string {
	if !strings.HasPrefix(importPath, ".") && !strings.HasPrefix(importPath, "/") {
		return ""
	}

	fromDir := filepath.Dir(fromFile)
	candidate := filepath.Join(fromDir, importPath)

	extensions := []string{".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"}
	for _, ext := range extensions {
		relPath := candidate + ext
		if _, err := os.Stat(filepath.Join(workDir, relPath)); err == nil {
			return filepath.ToSlash(relPath)
		}
	}

	if filepath.Ext(candidate) == "" {
		for _, ext := range extensions {
			relPath := filepath.Join(candidate, "index"+ext)
			if _, err := os.Stat(filepath.Join(workDir, relPath)); err == nil {
				return filepath.ToSlash(relPath)
			}
		}
	}

	return ""
}

func resolvePyImport(workDir, fromFile, importPath string, fileSet map[string]bool) string {
	if strings.HasPrefix(importPath, ".") {
		fromDir := filepath.Dir(fromFile)
		depth := strings.Count(importPath, ".") - 1
		base := fromDir
		for i := 0; i < depth; i++ {
			base = filepath.Dir(base)
		}
		module := strings.TrimLeft(importPath, ".")
		if module == "" {
			return ""
		}
		modulePath := strings.ReplaceAll(module, ".", string(filepath.Separator))
		relPath := filepath.Join(base, modulePath) + ".py"
		if _, err := os.Stat(filepath.Join(workDir, relPath)); err == nil {
			return filepath.ToSlash(relPath)
		}
		initPath := filepath.Join(base, modulePath, "__init__.py")
		if _, err := os.Stat(filepath.Join(workDir, initPath)); err == nil {
			return filepath.ToSlash(initPath)
		}
		return ""
	}

	modulePath := strings.ReplaceAll(importPath, ".", string(filepath.Separator))
	relPath := modulePath + ".py"
	if _, err := os.Stat(filepath.Join(workDir, relPath)); err == nil {
		return filepath.ToSlash(relPath)
	}
	initPath := filepath.Join(modulePath, "__init__.py")
	if _, err := os.Stat(filepath.Join(workDir, initPath)); err == nil {
		return filepath.ToSlash(initPath)
	}
	return ""
}

func resolveRustImport(workDir, fromFile, usePath string, fileSet map[string]bool) string {
	if !strings.HasPrefix(usePath, "crate::") {
		return ""
	}

	modulePath := strings.TrimPrefix(usePath, "crate::")
	parts := strings.Split(modulePath, "::")
	candidate := filepath.Join(parts...) + ".rs"
	if _, err := os.Stat(filepath.Join(workDir, "src", candidate)); err == nil {
		return filepath.ToSlash(filepath.Join("src", candidate))
	}
	modCandidate := filepath.Join(parts...)
	modPath := filepath.Join(modCandidate, "mod.rs")
	if _, err := os.Stat(filepath.Join(workDir, "src", modPath)); err == nil {
		return filepath.ToSlash(filepath.Join("src", modPath))
	}
	return ""
}

var goModulePathCache = make(map[string]string)
var goModulePathCacheMu sync.Mutex

func readGoModModule(workDir string) string {
	goModulePathCacheMu.Lock()
	defer goModulePathCacheMu.Unlock()
	if cached, ok := goModulePathCache[workDir]; ok {
		return cached
	}
	content, err := os.ReadFile(filepath.Join(workDir, "go.mod"))
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(content), "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "module ") {
			modPath := strings.TrimSpace(strings.TrimPrefix(line, "module"))
			goModulePathCache[workDir] = modPath
			return modPath
		}
	}
	return ""
}

// AllParsers returns the default set of language parsers.
func AllParsers() []Parser {
	return []Parser{
		&GoParser{},
		&TypeScriptParser{},
		&PythonParser{},
		&RustParser{},
	}
}

// ParserForFile returns the first parser that can handle the given file.
func ParserForFile(path string, parsers []Parser) Parser {
	for _, p := range parsers {
		if p.CanParse(path) {
			return p
		}
	}
	return nil
}

// Parser extracts structured information from a source file.
type Parser interface {
	Language() string
	CanParse(path string) bool
	Parse(path string, content []byte) (*FileInfo, error)
}

// FileInfo is the parsed representation of a single source file.
type FileInfo struct {
	Path     string
	Language string
	Imports  []ImportInfo
	Exports  []SymbolInfo2
	Funcs    []FuncSignature
	Types    []TypeInfo
}

// ImportInfo represents a single import declaration.
type ImportInfo struct {
	Path string
}

// SymbolInfo2 represents a named symbol.
type SymbolInfo2 struct {
	Name     string
	Kind     string
	Exported bool
}

// FuncSignature represents a function signature.
type FuncSignature struct {
	Name     string
	Receiver string
	Params   string
	Returns  string
	Exported bool
}

// TypeInfo represents a type definition.
type TypeInfo struct {
	Name    string
	Kind    string
	Fields  []string
	Methods []string
}

// WalkDir implementation using filepath.WalkDir
func walkDir(root string, skipDirs map[string]bool, maxDepth int, fn func(path string, relPath string, p Parser) error) error {
	return filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if d.IsDir() {
			if skipDirs[d.Name()] {
				return filepath.SkipDir
			}
			depth := strings.Count(path[len(root):], string(filepath.Separator))
			if depth > maxDepth {
				return filepath.SkipDir
			}
			return nil
		}
		relPath, relErr := filepath.Rel(root, path)
		if relErr != nil {
			return nil
		}
		p := ParserForFile(relPath, AllParsers())
		if p == nil {
			return nil
		}
		return fn(path, relPath, p)
	})
}
