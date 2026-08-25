// Package explain assembles deterministic evidence packs about code and
// synthesizes grounded answers over them with a single constrained LLM call
// (D-05). Collectors orchestrate read-only integration APIs (code graph,
// symbol index, git blame); the LLM narrates over the pack but never
// invents evidence — citation markers are validated structurally against
// pack IDs after synthesis.
package explain

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/tokens"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/integrations/git"
)

// ErrTargetNotFound reports that no indexable target (symbol, file, or
// topic match) exists for the query. Callers should map it to a structured
// not-found result listing the searched scope. ErrSourceUnreachable stays
// reserved for external sources failing during collection.
var ErrTargetNotFound = errors.New("explain: target not found")

// DefaultMaxTotalTokens is the total evidence-pack token budget applied
// when CollectorDeps.MaxTotalTokens is zero (discretion area per D-05).
const DefaultMaxTotalTokens = 6000

// ExplainMode selects which collector strategy resolves a query. Only
// ModeSymbol is implemented in this plan; file/topic modes land in plan 04-03.
type ExplainMode string

const (
	// ModeSymbol explains a code symbol by name (function, type, ...).
	ModeSymbol ExplainMode = "symbol"
	// ModeFile explains why a file exists and who consumes it.
	ModeFile ExplainMode = "file"
	// ModeTopic explains a free-text topic such as "auth middleware".
	ModeTopic ExplainMode = "topic"
)

// CollectorDeps carries the injected integrations a collector reads from.
// Mirroring AnalyzeImpact's discipline, collectors never construct indexers
// internally; all slices they fill start non-nil. A zero MaxTotalTokens
// applies DefaultMaxTotalTokens.
type CollectorDeps struct {
	Graph          *codeintel.CodeGraph
	Index          *codeintel.SymbolIndex
	GitClient      *git.Git
	WorkDir        string
	MaxTotalTokens int
}

// kindPriority orders sections for deterministic output and budget
// trimming. Lower value = higher priority = dropped last. Trim order
// follows RESEARCH Pattern 1: tests drop first, then blame; source survives
// longest.
var kindPriority = map[types.EvidenceKind]int{
	types.EvidenceSource:  0,
	types.EvidenceCallers: 1,
	types.EvidenceBlame:   2,
	types.EvidenceCommit:  2,
	types.EvidenceADR:     3,
	types.EvidenceTest:    4,
}

// Collect builds an EvidencePack for the query in the given mode. It never
// returns a nil pack alongside an error when collection started — unknown
// targets return a zero-section pack plus an error wrapping
// ErrTargetNotFound so callers can render structured not-found output.
func Collect(ctx context.Context, deps CollectorDeps, query string, mode ExplainMode, modelID string) (*types.EvidencePack, error) {
	pack := types.NewEvidencePack(query)

	switch mode {
	case ModeSymbol:
		return collectSymbol(ctx, deps, query, modelID, pack)
	case ModeFile, ModeTopic:
		return nil, fmt.Errorf("explain: mode %q not implemented yet", mode)
	default:
		return nil, fmt.Errorf("explain: unknown mode %q", mode)
	}
}

// collectSymbol runs the symbol-mode stages in numbered order, mirroring
// AnalyzeImpact's orchestration shape.
func collectSymbol(ctx context.Context, deps CollectorDeps, symbol string, modelID string, pack *types.EvidencePack) (*types.EvidencePack, error) {
	if err := ctx.Err(); err != nil {
		return pack, fmt.Errorf("explain: %w", err)
	}

	// Stage 1: resolve definition locations via the symbol index.
	locs := deps.Index.Define(symbol)
	if len(locs) == 0 {
		return pack, fmt.Errorf("explain: no symbol %q found in workspace index (searched indexed symbols): %w", symbol, ErrTargetNotFound)
	}
	primary := locs[0]

	// Stage 2: source excerpt around the primary definition location.
	if snippet, line, err := readExcerpt(deps.WorkDir, primary.File, symbol); err == nil {
		pack.Add(types.EvidenceSource,
			fmt.Sprintf("%s:%d", primary.File, line),
			snippet)
	}

	// Stage 3: direct callers from the call graph, mapped to file:line refs.
	for _, edge := range deps.Graph.Callers(symbol) {
		pack.Add(types.EvidenceCallers,
			fmt.Sprintf("%s:%d", edge.CallerFile, edge.CallerLine),
			fmt.Sprintf("%s calls %s", edge.CallerName, symbol))
	}

	// Stage 4: blame attribution reduced to ONE last-touch item — the
	// commit with the maximum author time among rows touching the file.
	if deps.GitClient != nil && primary.File != "" {
		if last := lastTouchCommit(deps.GitClient, primary.File); last != nil {
			pack.Add(types.EvidenceBlame, last.SHA,
				fmt.Sprintf("last touched by %s on %s: %s",
					last.Author,
					last.AuthorTime.Format("2006-01-02"),
					last.Summary))
		}
	}

	// Stage 5: related tests from impact analysis. AffectedTests originates
	// from set iteration, so sort for determinism before insertion.
	impact := codeintel.AnalyzeImpact(deps.Graph, deps.Index, symbol, 1)
	tests := append([]string{}, impact.AffectedTests...)
	sort.Strings(tests)
	for _, tf := range tests {
		pack.Add(types.EvidenceTest, tf, fmt.Sprintf("test file related to %s", symbol))
	}

	// Stage 6: collapse duplicate kind+ref pairs (first wins) and renumber.
	pack.Sections = dedupeSections(pack.Sections)

	// Stage 7: enforce the token budget by dropping whole lowest-priority
	// sections; snippets are never mid-truncated.
	budgetTrim(pack, deps.MaxTotalTokens, modelID)

	return pack, nil
}

// readExcerpt returns a line-numbered excerpt of relFile centered on the
// first line mentioning symbol, plus that anchor line number (1-indexed).
func readExcerpt(workDir, relFile, symbol string) (string, int, error) {
	data, err := os.ReadFile(filepath.Join(workDir, relFile))
	if err != nil {
		return "", 0, err
	}
	lines := strings.Split(strings.TrimRight(string(data), "\n"), "\n")
	if len(lines) == 0 {
		return "", 0, fmt.Errorf("empty file")
	}

	anchor := 1
	for i, ln := range lines {
		if strings.Contains(ln, symbol) {
			anchor = i + 1
			break
		}
	}

	const ctxLines = 6
	start := anchor - ctxLines
	if start < 1 {
		start = 1
	}
	end := anchor + ctxLines
	if end > len(lines) {
		end = len(lines)
	}

	var sb strings.Builder
	for i := start; i <= end; i++ {
		fmt.Fprintf(&sb, "%4d | %s\n", i, lines[i-1])
	}
	return strings.TrimRight(sb.String(), "\n"), anchor, nil
}

// lastTouchCommit reduces blame output to the single CommitMeta with the
// latest author time (the file's last-touch evidence). Returns nil when
// blame yields nothing usable.
func lastTouchCommit(g *git.Git, relFile string) *git.CommitMeta {
	_, commits, err := g.BlamePorcelain(relFile)
	if err != nil || len(commits) == 0 {
		return nil
	}
	last := &commits[0]
	for i := range commits {
		if commits[i].AuthorTime.After(last.AuthorTime) {
			last = &commits[i]
		}
	}
	return last
}

// dedupeSections collapses duplicate kind+ref pairs keeping the first
// occurrence, then renumbers IDs sequentially so citation markers stay
// contiguous [1..n].
func dedupeSections(sections []types.Evidence) []types.Evidence {
	if sections == nil {
		return sections
	}
	seen := make(map[string]struct{}, len(sections))
	out := make([]types.Evidence, 0, len(sections))
	for _, s := range sections {
		key := string(s.Kind) + "\x00" + s.Ref
		if _, dup := seen[key]; dup {
			continue
		}
		seen[key] = struct{}{}
		out = append(out, s)
	}
	for i := range out {
		out[i].ID = i + 1
	}
	return out
}

// budgetTrim drops whole lowest-priority sections until the estimated token
// total fits maxTokens (zero applies DefaultMaxTotalTokens). At least one
// section always survives — an oversized top-priority item beats an empty
// pack. Snippets are never mid-truncated. Sets Truncated on any drop.
func budgetTrim(pack *types.EvidencePack, maxTokens int, modelID string) {
	if maxTokens <= 0 {
		maxTokens = DefaultMaxTotalTokens
	}
	est := tokens.NewEstimator(modelID)

	total := 0
	for _, s := range pack.Sections {
		total += est.Estimate(s.Snippet)
	}
	if total <= maxTokens {
		return
	}

	for total > maxTokens && len(pack.Sections) > 1 {
		worst := -1
		worstPrio := -1
		for i, s := range pack.Sections {
			p := kindPriority[s.Kind]
			// >= so later items of equal priority drop first, keeping
			// earlier markers stable across renumbering.
			if p >= worstPrio {
				worstPrio = p
				worst = i
			}
		}
		if worst < 0 {
			break
		}
		total -= est.Estimate(pack.Sections[worst].Snippet)
		pack.Sections = append(pack.Sections[:worst], pack.Sections[worst+1:]...)
		pack.Truncated = true
	}

	for i := range pack.Sections {
		pack.Sections[i].ID = i + 1
	}
}
