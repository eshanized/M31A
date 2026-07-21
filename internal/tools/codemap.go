package tools

import (
	"context"
	"fmt"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
)

// Compile-time interface check
var _ types.Tool = (*CodeMap)(nil)

// CodeMap is a tool that queries the codebase intelligence layer for
// import graph, symbol definitions, and relevant file discovery.
type CodeMap struct {
	workDir string

	mu      sync.Mutex
	indexer *codeintel.Indexer
	built   bool
	builtAt time.Time
}

// NewCodeMap creates a new CodeMap tool instance.
func NewCodeMap(workDir string) *CodeMap {
	return &CodeMap{workDir: workDir}
}

func (t *CodeMap) Name() string {
	return "CodeMap"
}

func (t *CodeMap) Description() string {
	return "Query the codebase intelligence: find where symbols are defined, what files import/export, " +
		"and discover relevant files. Modes: upstream (what a file depends on), downstream (what depends on it), " +
		"define (where a symbol is defined), references (where a symbol is used), relevant (find files related to a query)."
}

func (t *CodeMap) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

func (t *CodeMap) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"query": {
				"type": "string",
				"description": "A file path or symbol name to query"
			},
			"mode": {
				"type": "string",
				"enum": ["upstream", "downstream", "define", "references", "relevant", "symbols"],
				"description": "Query mode: upstream (file dependencies), downstream (file dependents), define (symbol definition location), references (symbol usage), relevant (find related files), symbols (list symbols in a file)"
			},
			"depth": {
				"type": "integer",
				"description": "Traversal depth for upstream/downstream (default 2, max 5)",
				"minimum": 1,
				"maximum": 5
			}
		},
		"required": ["query", "mode"]
	}`
}

func (t *CodeMap) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	query, ok := input.Params["query"].(string)
	if !ok || query == "" {
		return types.ToolResult{}, fmt.Errorf("%w: missing or empty parameter: query", m31errors.ErrToolExecution)
	}

	mode, ok := input.Params["mode"].(string)
	if !ok || mode == "" {
		return types.ToolResult{}, fmt.Errorf("%w: missing or empty parameter: mode", m31errors.ErrToolExecution)
	}

	depth := 2
	if d, ok := input.Params["depth"].(float64); ok {
		depth = int(d)
		if depth < 1 {
			depth = 1
		}
		if depth > 5 {
			depth = 5
		}
	}

	idx, err := t.getIndexer(ctx)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: codeintel: %w", m31errors.ErrToolExecution, err)
	}

	var output string
	switch mode {
	case "upstream":
		files := idx.Upstream(query, depth)
		output = formatFileList("Upstream dependencies of "+query, files)

	case "downstream":
		files := idx.Downstream(query, depth)
		output = formatFileList("Files that depend on "+query, files)

	case "define":
		locs := idx.Define(query)
		output = formatSymbolLocations("Definition of "+query, locs)

	case "references":
		locs := idx.Define(query)
		if len(locs) == 0 {
			matches := idx.SymbolsMatching(query)
			output = fmt.Sprintf("No exact definition found for %q.\n\nSimilar symbols:\n", query)
			for _, m := range matches {
				defLocs := idx.Define(m)
				for _, loc := range defLocs {
					output += fmt.Sprintf("- **%s** (%s) in %s\n", m, loc.Kind, loc.File)
				}
			}
		} else {
			output = formatSymbolLocations("References to "+query, locs)
			for _, loc := range locs {
				downstream := idx.Downstream(loc.File, 1)
				if len(downstream) > 0 {
					output += fmt.Sprintf("\nFiles that import %s (may reference %s):\n", loc.File, query)
					for _, d := range downstream {
						output += fmt.Sprintf("- %s\n", d)
					}
				}
			}
		}

	case "relevant":
		scored := idx.RelevantFiles(nil, query, 15)
		if len(scored) == 0 {
			matches := idx.SymbolsMatching(query)
			if len(matches) > 0 {
				scored = idx.RelevantFiles(nil, strings.Join(matches, " "), 15)
			}
		}
		output = formatScoredFiles("Files relevant to: "+query, scored)

	case "symbols":
		syms := idx.FileSymbols(query)
		if len(syms) == 0 {
			output = fmt.Sprintf("No symbols found in %s. The file may not exist or may not be a supported language.", query)
		} else {
			output = fmt.Sprintf("Symbols in **%s** (%d total):\n\n", query, len(syms))
			for _, s := range syms {
				exported := ""
				if s.Exported {
					exported = " [exported]"
				}
				output += fmt.Sprintf("- **%s** (%s)%s\n", s.Name, s.Kind, exported)
			}
		}

	default:
		return types.ToolResult{}, fmt.Errorf("%w: unknown mode %q (valid: upstream, downstream, define, references, relevant, symbols)", m31errors.ErrToolExecution, mode)
	}

	elapsed := time.Since(start).Milliseconds()
	return types.ToolResult{
		Output:     output,
		DurationMs: elapsed,
	}, nil
}

func (t *CodeMap) getIndexer(ctx context.Context) (*codeintel.Indexer, error) {
	t.mu.Lock()
	defer t.mu.Unlock()
	// Rebuild if not built or if stale (older than 5 minutes)
	if t.built && time.Since(t.builtAt) < 5*time.Minute {
		return t.indexer, nil
	}
	idx := codeintel.NewIndexer(t.workDir)
	buildCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	if err := idx.Build(buildCtx); err != nil {
		return nil, err
	}
	t.indexer = idx
	t.built = true
	t.builtAt = time.Now()
	return idx, nil
}

func formatFileList(title string, files []string) string {
	if len(files) == 0 {
		return title + ": none found."
	}
	var sb strings.Builder
	fmt.Fprintf(&sb, "%s (%d files):\n\n", title, len(files))
	for _, f := range files {
		fmt.Fprintf(&sb, "- %s\n", f)
	}
	return sb.String()
}

func formatSymbolLocations(title string, locs []codeintel.SymbolLocation) string {
	if len(locs) == 0 {
		return title + ": not found in the codebase."
	}
	var sb strings.Builder
	fmt.Fprintf(&sb, "%s (%d locations):\n\n", title, len(locs))
	for _, loc := range locs {
		fmt.Fprintf(&sb, "- **%s** in %s\n", loc.Kind, loc.File)
	}
	return sb.String()
}

func formatScoredFiles(title string, files []codeintel.ScoredFile) string {
	if len(files) == 0 {
		return title + ": no relevant files found."
	}
	var sb strings.Builder
	fmt.Fprintf(&sb, "%s (%d files):\n\n", title, len(files))
	for _, sf := range files {
		fmt.Fprintf(&sb, "- **%s** (score: %.1f)", sf.Path, sf.Score)
		if len(sf.Reasons) > 0 {
			sb.WriteString(" — " + sf.Reasons[0])
		}
		sb.WriteString("\n")
	}
	return sb.String()
}
