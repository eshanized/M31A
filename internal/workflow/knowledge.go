package workflow

import (
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/codeintel"
)

// Knowledge holds project-specific learnings accumulated across sessions.
// Stored as methods on WorkflowState (not separate package) until data model proven.
type Knowledge struct {
	mu sync.RWMutex

	// Conventions detected from codebase (naming, imports, error handling)
	Conventions []Convention `json:"conventions"`

	// Patterns observed in the codebase (architectural patterns, common shapes)
	Patterns []Pattern `json:"patterns"`

	// Learned facts from previous sessions (what worked, what failed)
	Facts []Fact `json:"facts"`

	// File-specific knowledge (hot files, complex files, test patterns)
	FileKnowledge map[string]FileKnowledge `json:"file_knowledge"`

	// Last analysis timestamp
	LastAnalyzed time.Time `json:"last_analyzed"`
}

// Convention represents a detected coding convention.
type Convention struct {
	Category   string  `json:"category"` // naming, imports, errors, etc.
	Pattern    string  `json:"pattern"`  // e.g., "error handling uses sentinel errors"
	Example    string  `json:"example"`  // e.g., "m31errors.ErrNotFound"
	Confidence float64 `json:"confidence"`
}

// Pattern represents an observed architectural pattern.
type Pattern struct {
	Name        string   `json:"name"`        // e.g., "Opts pattern", "sentinel errors"
	Description string   `json:"description"` // e.g., "Constructor uses Opts struct"
	Files       []string `json:"files"`       // files where pattern is observed
	Confidence  float64  `json:"confidence"`
}

// Fact represents a learned fact from previous sessions.
type Fact struct {
	Category  string    `json:"category"`  // success, failure, preference
	Statement string    `json:"statement"` // e.g., "go test -race catches data races"
	SessionID string    `json:"session_id"`
	Timestamp time.Time `json:"timestamp"`
}

// FileKnowledge holds per-file intelligence.
type FileKnowledge struct {
	LastModified time.Time `json:"last_modified"`
	Complexity   string    `json:"complexity"` // low, medium, high
	IsTest       bool      `json:"is_test"`
	IsHot        bool      `json:"is_hot"` // frequently modified
	Language     string    `json:"language"`
}

// NewKnowledge creates a new Knowledge instance.
func NewKnowledge() *Knowledge {
	return &Knowledge{
		Conventions:   make([]Convention, 0),
		Patterns:      make([]Pattern, 0),
		Facts:         make([]Fact, 0),
		FileKnowledge: make(map[string]FileKnowledge),
	}
}

// AddConvention adds a convention if not already present.
// Caps at 50 conventions to prevent unbounded growth.
func (k *Knowledge) AddConvention(c Convention) {
	k.mu.Lock()
	defer k.mu.Unlock()
	for _, existing := range k.Conventions {
		if existing.Category == c.Category && existing.Pattern == c.Pattern {
			return // already known
		}
	}
	if len(k.Conventions) >= 50 {
		return
	}
	k.Conventions = append(k.Conventions, c)
}

// AddPattern adds a pattern if not already present.
// Caps at 50 patterns to prevent unbounded growth.
func (k *Knowledge) AddPattern(p Pattern) {
	k.mu.Lock()
	defer k.mu.Unlock()
	for _, existing := range k.Patterns {
		if existing.Name == p.Name {
			return // already known
		}
	}
	if len(k.Patterns) >= 50 {
		return
	}
	k.Patterns = append(k.Patterns, p)
}

// AddFact adds a learned fact if not already present (dedup by Statement).
func (k *Knowledge) AddFact(f Fact) {
	k.mu.Lock()
	defer k.mu.Unlock()
	for _, existing := range k.Facts {
		if existing.Statement == f.Statement {
			return
		}
	}
	k.Facts = append(k.Facts, f)
	// Keep only last 100 facts
	if len(k.Facts) > 100 {
		k.Facts = k.Facts[len(k.Facts)-100:]
	}
}

// UpdateFileKnowledge updates knowledge for a specific file.
// Caps at 200 entries to prevent unbounded growth.
func (k *Knowledge) UpdateFileKnowledge(path string, fk FileKnowledge) {
	k.mu.Lock()
	defer k.mu.Unlock()
	if _, exists := k.FileKnowledge[path]; !exists && len(k.FileKnowledge) >= 200 {
		return
	}
	k.FileKnowledge[path] = fk
}

// GetConventions returns a copy of all conventions.
func (k *Knowledge) GetConventions() []Convention {
	k.mu.RLock()
	defer k.mu.RUnlock()
	result := make([]Convention, len(k.Conventions))
	copy(result, k.Conventions)
	return result
}

// GetPatterns returns a copy of all patterns.
func (k *Knowledge) GetPatterns() []Pattern {
	k.mu.RLock()
	defer k.mu.RUnlock()
	result := make([]Pattern, len(k.Patterns))
	copy(result, k.Patterns)
	return result
}

// GetFacts returns a copy of all facts.
func (k *Knowledge) GetFacts() []Fact {
	k.mu.RLock()
	defer k.mu.RUnlock()
	result := make([]Fact, len(k.Facts))
	copy(result, k.Facts)
	return result
}

// FormatContext returns a formatted string of knowledge for injection into LLM context.
func (k *Knowledge) FormatContext(maxLen int) string {
	k.mu.RLock()
	defer k.mu.RUnlock()

	if maxLen < 3 {
		maxLen = 3
	}

	var b strings.Builder

	if len(k.Conventions) > 0 {
		b.WriteString("## Project Conventions\n")
		for _, c := range k.Conventions {
			b.WriteString("- " + c.Category + ": " + c.Pattern)
			if c.Example != "" {
				b.WriteString(" (e.g., " + c.Example + ")")
			}
			b.WriteString("\n")
		}
		b.WriteString("\n")
	}

	if len(k.Patterns) > 0 {
		b.WriteString("## Architectural Patterns\n")
		for _, p := range k.Patterns {
			b.WriteString("- " + p.Name + ": " + p.Description + "\n")
		}
		b.WriteString("\n")
	}

	if len(k.Facts) > 0 {
		// Show only last 10 facts
		start := 0
		if len(k.Facts) > 10 {
			start = len(k.Facts) - 10
		}
		b.WriteString("## Learned Facts\n")
		for _, f := range k.Facts[start:] {
			b.WriteString("- [" + f.Category + "] " + f.Statement + "\n")
		}
		b.WriteString("\n")
	}

	result := b.String()
	if len(result) > maxLen {
		result = result[:maxLen-3] + "..."
	}
	return result
}

// DetectConventions analyzes codebase files to detect conventions.
func (k *Knowledge) DetectConventions(workDir string, files []*codeintel.FileInfo) {
	namingConventions := detectNamingConventions(files)
	for _, c := range namingConventions {
		k.AddConvention(c)
	}

	k.mu.Lock()
	defer k.mu.Unlock()
	k.LastAnalyzed = time.Now()
}

// detectNamingConventions analyzes file names to detect naming patterns.
func detectNamingConventions(files []*codeintel.FileInfo) []Convention {
	var conventions []Convention
	snakeCase := 0
	camelCase := 0
	kebabCase := 0

	for _, f := range files {
		name := f.Path
		if strings.Contains(name, "_") {
			snakeCase++
		} else if strings.Contains(name, "-") {
			kebabCase++
		} else if len(name) > 0 && name[0] >= 'A' && name[0] <= 'Z' {
			camelCase++
		}
	}

	total := snakeCase + camelCase + kebabCase
	if total == 0 {
		return conventions
	}

	if snakeCase > total/2 {
		conventions = append(conventions, Convention{
			Category:   "naming",
			Pattern:    "snake_case for file names",
			Confidence: float64(snakeCase) / float64(total),
		})
	}
	if kebabCase > total/2 {
		conventions = append(conventions, Convention{
			Category:   "naming",
			Pattern:    "kebab-case for file names",
			Confidence: float64(kebabCase) / float64(total),
		})
	}
	if camelCase > total/2 {
		conventions = append(conventions, Convention{
			Category:   "naming",
			Pattern:    "PascalCase for file names",
			Confidence: float64(camelCase) / float64(total),
		})
	}

	return conventions
}
