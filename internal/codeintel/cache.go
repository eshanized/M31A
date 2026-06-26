package codeintel

import (
	"crypto/sha256"
	"encoding/gob"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// FileCache stores cached parse results for a single file.
type FileCache struct {
	Path    string
	Hash    [32]byte // SHA-256 of file content
	ModTime time.Time
	Info    *FileInfo
}

// IndexCache is the on-disk cache for the codeintel index.
type IndexCache struct {
	Version int
	BuiltAt time.Time
	Files   map[string]*FileCache // relative path → cache entry
}

// cachePath returns the path to the index cache file.
func cachePath(workDir string) string {
	return filepath.Join(workDir, ".m31a", "codeintel.cache")
}

// fileHash computes SHA-256 of file content.
func fileHash(path string) ([32]byte, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return [32]byte{}, err
	}
	return sha256.Sum256(data), nil
}

// LoadCache loads the index cache from disk. Returns nil if no cache exists.
func LoadCache(workDir string) *IndexCache {
	path := cachePath(workDir)
	f, err := os.Open(path)
	if err != nil {
		return nil
	}
	defer func() { _ = f.Close() }()

	var cache IndexCache
	if err := gob.NewDecoder(f).Decode(&cache); err != nil {
		return nil
	}

	if cache.Version != 1 {
		return nil
	}

	return &cache
}

// SaveCache writes the index cache to disk.
func SaveCache(workDir string, cache *IndexCache) error {
	path := cachePath(workDir)
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}

	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer func() { _ = f.Close() }()

	cache.Version = 1
	cache.BuiltAt = time.Now()

	return gob.NewEncoder(f).Encode(cache)
}

// IncrementalBuild checks which files have changed and returns only those
// that need to be reparsed. Unchanged files are loaded from cache.
type IncrementalResult struct {
	Changed   []string             // files that need reparsing
	Unchanged []string             // files that can be loaded from cache
	New       []string             // files not in cache
	Deleted   []string             // files in cache but not on disk
	Cache     map[string]*FileInfo // cached parse results for unchanged files
}

// CheckIncremental compares current files against the cache and returns
// what needs to be reparsed.
func CheckIncremental(workDir string, parsers []Parser, cachedFiles map[string]*FileCache) *IncrementalResult {
	result := &IncrementalResult{
		Cache: make(map[string]*FileInfo),
	}

	skipDirs := map[string]bool{
		"node_modules": true, "vendor": true, ".git": true,
		".next": true, "dist": true, "build": true, "target": true,
		".venv": true, "venv": true, "__pycache__": true,
	}

	// Track which files we've seen
	seen := make(map[string]bool)

	if err := filepath.WalkDir(workDir, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if d.IsDir() {
			if skipDirs[d.Name()] {
				return filepath.SkipDir
			}
			depth := 0
			for i := len(workDir); i < len(path); i++ {
				if path[i] == filepath.Separator {
					depth++
				}
			}
			if depth > 5 {
				return filepath.SkipDir
			}
			return nil
		}

		relPath, relErr := filepath.Rel(workDir, path)
		if relErr != nil {
			return nil
		}

		p := ParserForFile(path, parsers)
		if p == nil {
			return nil
		}

		seen[relPath] = true

		// Check cache
		if cached, ok := cachedFiles[relPath]; ok {
			// Check if file has changed
			info, err := os.Stat(path)
			if err != nil {
				return nil
			}

			// Quick check: modification time
			if !info.ModTime().After(cached.ModTime) {
				// File hasn't changed — use cached result
				result.Unchanged = append(result.Unchanged, relPath)
				if cached.Info != nil {
					result.Cache[relPath] = cached.Info
				}
				return nil
			}

			// ModTime changed — verify with content hash
			hash, err := fileHash(path)
			if err != nil {
				return nil
			}

			if hash == cached.Hash {
				// Content identical — use cached result
				result.Unchanged = append(result.Unchanged, relPath)
				if cached.Info != nil {
					result.Cache[relPath] = cached.Info
				}
				return nil
			}
		}

		// File is new or changed — needs reparsing
		result.New = append(result.New, relPath)
		return nil
	}); err != nil {
		return nil
	}

	// Find deleted files (in cache but not on disk)
	for relPath := range cachedFiles {
		if !seen[relPath] {
			result.Deleted = append(result.Deleted, relPath)
		}
	}

	// Files that need reparsing = new + changed
	result.Changed = append(result.New, result.Deleted...) // deletions also trigger rebuild

	return result
}

// BuildCacheFromFiles builds a cache from parsed files.
func BuildCacheFromFiles(workDir string, files []*FileInfo, cacheMu *sync.Mutex) map[string]*FileCache {
	result := make(map[string]*FileCache, len(files))
	for _, f := range files {
		absPath := filepath.Join(workDir, f.Path)
		hash, err := fileHash(absPath)
		if err != nil {
			continue
		}
		info, err := os.Stat(absPath)
		if err != nil {
			continue
		}
		result[f.Path] = &FileCache{
			Path:    f.Path,
			Hash:    hash,
			ModTime: info.ModTime(),
			Info:    f,
		}
	}
	return result
}
