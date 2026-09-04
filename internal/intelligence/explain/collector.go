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

// ExplainMode selects which collector strategy resolves a query.
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

// CandidateADR represents a matching ADR file found by ScanADRs.
type CandidateADR struct {
	Title   string // first heading
	Path    string // relative path from workDir
	Snippet string // first heading + first paragraph, capped
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
	case ModeFile:
		return collectFile(ctx, deps, query, modelID, pack)
	case ModeTopic:
		return collectTopic(ctx, deps, query, modelID, pack)
	default:
		return nil, fmt.Errorf("explain: unknown mode %q", mode)
	}
}

// ResolveMode implements the documented disambiguation precedence:
// 1. File mode if path exists relative to workDir
// 2. Symbol mode if exact match in Index.Define
// 3. Topic mode as fallback
func ResolveMode(query, workDir string, index *codeintel.SymbolIndex) ExplainMode {
	// Check if file exists relative to workDir
	if workDir != "" {
		absPath := filepath.Join(workDir, query)
		if _, err := os.Stat(absPath); err == nil {
			return ModeFile
		}
	}

	// Check for exact symbol match
	if index != nil {
		locs := index.Define(query)
		if len(locs) > 0 {
			return ModeSymbol
		}
	}

	// Default to topic mode for free-text queries
	return ModeTopic
}

// ScanADRs scans the .m31a/decisions/ directory for ADR files matching the query.
// Returns title/path/snippet triples filtered by token overlap.
// Degrades silently when the decisions directory is absent (A1 contract).
func ScanADRs(workDir, query string) []CandidateADR {
	decisionsDir := filepath.Join(workDir, ".m31a", "decisions")
	entries, err := os.ReadDir(decisionsDir)
	if err != nil {
		// Degrade silently - no ADR directory or not readable
		return nil
	}

	queryTokens := tokenizeQuery(query)
	var adrs []CandidateADR

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if !strings.HasSuffix(entry.Name(), ".md") {
			continue
		}

		adrPath := filepath.Join(decisionsDir, entry.Name())
		content, err := os.ReadFile(adrPath)
		if err != nil {
			continue // skip unreadable files
		}

		// Check token overlap
		if !hasTokenOverlap(string(content), queryTokens) {
			continue
		}

		// Extract title (first heading) and snippet (first heading + first paragraph)
		title, snippet := extractADRContent(string(content))
		if title == "" {
			title = strings.TrimSuffix(entry.Name(), ".md")
		}

		adrs = append(adrs, CandidateADR{
			Title:   title,
			Path:    filepath.Join(".m31a", "decisions", entry.Name()),
			Snippet: snippet,
		})
	}

	return adrs
}

// tokenizeQuery splits query into lowercase tokens for matching.
func tokenizeQuery(query string) []string {
	query = strings.ToLower(query)
	// Simple tokenization: split on non-alphanumeric
	var tokens []string
	var current strings.Builder
	for _, r := range query {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') {
			current.WriteRune(r)
		} else {
			if current.Len() > 0 {
				tokens = append(tokens, current.String())
				current.Reset()
			}
		}
	}
	if current.Len() > 0 {
		tokens = append(tokens, current.String())
	}
	return tokens
}

// hasTokenOverlap checks if any query token appears in the content.
func hasTokenOverlap(content string, queryTokens []string) bool {
	lower := strings.ToLower(content)
	for _, token := range queryTokens {
		if len(token) >= 3 && strings.Contains(lower, token) { // minimum token length 3
			return true
		}
	}
	return false
}

// extractADRContent extracts the first heading and first paragraph from ADR content.
// Returns (title, snippet) where snippet is capped at heading + first paragraph.
func extractADRContent(content string) (string, string) {
	lines := strings.Split(content, "\n")
	var title string
	var snippetLines []string
	inFirstParagraph := false
	paragraphStarted := false

	for _, line := range lines {
		trimmed := strings.TrimSpace(line)

		// Find first heading
		if title == "" && strings.HasPrefix(trimmed, "#") {
			title = strings.TrimLeft(trimmed, "# ")
			snippetLines = append(snippetLines, trimmed)
			inFirstParagraph = true
			continue
		}

		if inFirstParagraph {
			if trimmed == "" {
				if paragraphStarted {
					// End of first paragraph
					break
				}
				continue
			}
			paragraphStarted = true
			snippetLines = append(snippetLines, trimmed)

			// Cap at reasonable length (heading + ~3 lines)
			if len(snippetLines) > 5 {
				break
			}
		}
	}

	snippet := strings.Join(snippetLines, "\n")
	return title, snippet
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

	// Stage 6: ADR scanning for symbol mode
	if deps.WorkDir != "" {
		adrs := ScanADRs(deps.WorkDir, symbol)
		for _, adr := range adrs {
			pack.Add(types.EvidenceADR, adr.Path, adr.Snippet)
		}
	}

	// Stage 7: collapse duplicate kind+ref pairs (first wins) and renumber.
	pack.Sections = dedupeSections(pack.Sections)

	// Stage 8: enforce the token budget by dropping whole lowest-priority
	// sections; snippets are never mid-truncated.
	budgetTrim(pack, deps.MaxTotalTokens, modelID)

	return pack, nil
}

// collectFile implements file-existence archaeology mode (EXPLAIN-04).
// Returns pack containing: introduction commit, source excerpt, consumers,
// removal impact (affected tests + downstream dependents).
func collectFile(ctx context.Context, deps CollectorDeps, filePath string, modelID string, pack *types.EvidencePack) (*types.EvidencePack, error) {
	if err := ctx.Err(); err != nil {
		return pack, fmt.Errorf("explain: %w", err)
	}

	// Verify file exists
	absPath := filepath.Join(deps.WorkDir, filePath)
	if _, err := os.Stat(absPath); err != nil {
		return pack, fmt.Errorf("explain: file %q not found: %w", filePath, ErrTargetNotFound)
	}

	// Stage 1: Introduction commit via git log --diff-filter=A
	introCommit, introSubject, err := introductionCommit(deps.GitClient, filePath)
	if err == nil && introCommit != "" {
		pack.Add(types.EvidenceCommit, introCommit,
			fmt.Sprintf("introduction commit: %s", introSubject))
	}

	// Stage 2: Source excerpt (whole file or key sections)
	if snippet, _, err := readExcerpt(deps.WorkDir, filePath, ""); err == nil {
		pack.Add(types.EvidenceSource, filePath+":1", snippet)
	}

	// Stage 3: Consumers - find primary symbol in file and get callers
	primarySymbol := primarySymbolInFile(deps.Index, filePath)
	if primarySymbol != "" {
		for _, edge := range deps.Graph.Callers(primarySymbol) {
			pack.Add(types.EvidenceCallers,
				fmt.Sprintf("%s:%d", edge.CallerFile, edge.CallerLine),
				fmt.Sprintf("%s calls %s", edge.CallerName, primarySymbol))
		}
	} else {
		// Fallback: use graph Downstream on the file node
		downstream := deps.Graph.Downstream(filePath, 1)
		for _, dep := range downstream {
			pack.Add(types.EvidenceCallers, dep, fmt.Sprintf("file depends on %s", filePath))
		}
	}

	// Stage 4: Removal impact - affected tests from impact analysis
	if primarySymbol != "" {
		impact := codeintel.AnalyzeImpact(deps.Graph, deps.Index, primarySymbol, 1)
		tests := append([]string{}, impact.AffectedTests...)
		sort.Strings(tests)
		for _, tf := range tests {
			pack.Add(types.EvidenceTest, tf, fmt.Sprintf("test affected by removal of %s", filePath))
		}
	}

	// Stage 5: ADR scanning for file mode
	if deps.WorkDir != "" {
		adrs := ScanADRs(deps.WorkDir, filePath)
		for _, adr := range adrs {
			pack.Add(types.EvidenceADR, adr.Path, adr.Snippet)
		}
	}

	// Stage 6: collapse duplicate kind+ref pairs (first wins) and renumber.
	pack.Sections = dedupeSections(pack.Sections)

	// Stage 7: enforce the token budget
	budgetTrim(pack, deps.MaxTotalTokens, modelID)

	return pack, nil
}

// collectTopic implements free-text topic mode.
// Produces pack with source excerpts ranked by term-match count,
// symbol hits, and up to 5 recent commits per matched file.
func collectTopic(ctx context.Context, deps CollectorDeps, query string, modelID string, pack *types.EvidencePack) (*types.EvidencePack, error) {
	if err := ctx.Err(); err != nil {
		return pack, fmt.Errorf("explain: %w", err)
	}

	queryTokens := tokenizeQuery(query)
	if len(queryTokens) == 0 {
		return pack, fmt.Errorf("explain: empty topic query: %w", ErrTargetNotFound)
	}

	// Get all tracked files from the graph
	allFiles := deps.Graph.AllPaths()
	if len(allFiles) == 0 {
		return pack, fmt.Errorf("explain: no indexed files for topic search: %w", ErrTargetNotFound)
	}

	// Score files by token occurrences in filename + content
	type scoredFile struct {
		path  string
		score int
	}
	var scored []scoredFile

	for _, file := range allFiles {
		score := scoreFile(file, queryTokens, deps.WorkDir)
		if score > 0 {
			scored = append(scored, scoredFile{path: file, score: score})
		}
	}

	// Sort by score descending
	sort.Slice(scored, func(i, j int) bool {
		return scored[i].score > scored[j].score
	})

	// Take top 3 files
	topFiles := scored
	if len(topFiles) > 3 {
		topFiles = topFiles[:3]
	}

	// For each top file: excerpt around matches, symbol hits, recent commits
	for _, sf := range topFiles {
		// Source excerpt around first match
		if snippet, line, err := readExcerptAroundMatches(deps.WorkDir, sf.path, queryTokens); err == nil {
			pack.Add(types.EvidenceSource,
				fmt.Sprintf("%s:%d", sf.path, line),
				snippet)
		}

		// Symbol hits from index prefix lookup
		// Find symbols defined in this file that match query tokens
		fileSymbols := deps.Index.FileSymbols(sf.path)
		for _, sym := range fileSymbols {
			for _, token := range queryTokens {
				if len(token) >= 3 && strings.Contains(strings.ToLower(sym.Name), token) {
					pack.Add(types.EvidenceSource,
						fmt.Sprintf("%s:%s", sf.path, sym.Name),
						fmt.Sprintf("symbol %s matches topic", sym.Name))
					break
				}
			}
		}

		// Recent commits (up to 5) touching this file
		if deps.GitClient != nil {
			commits, err := recentCommitsForFile(deps.GitClient, sf.path, 5)
			if err == nil {
				for _, c := range commits {
					pack.Add(types.EvidenceCommit, c.SHA,
						fmt.Sprintf("%s on %s: %s",
							c.Author,
							c.AuthorTime.Format("2006-01-02"),
							c.Summary))
				}
			}
		}
	}

	// ADR scanning for topic mode
	if deps.WorkDir != "" {
		adrs := ScanADRs(deps.WorkDir, query)
		for _, adr := range adrs {
			pack.Add(types.EvidenceADR, adr.Path, adr.Snippet)
		}
	}

	// Dedupe and budget trim
	pack.Sections = dedupeSections(pack.Sections)
	budgetTrim(pack, deps.MaxTotalTokens, modelID)

	return pack, nil
}

// introductionCommit finds the first commit that added the file using
// git log --diff-filter=A --format=%H%x00%s -1 -- path
func introductionCommit(g *git.Git, filePath string) (string, string, error) {
	out, err := g.Run("log", "--diff-filter=A", "--format=%H%x00%s", "-1", "--", filePath)
	if err != nil {
		return "", "", err
	}
	out = strings.TrimSpace(out)
	if out == "" {
		return "", "", nil
	}
	parts := strings.SplitN(out, "\x00", 2)
	if len(parts) < 2 {
		return parts[0], "", nil
	}
	return parts[0], parts[1], nil
}

// primarySymbolInFile finds the "main" symbol defined in a file.
// Returns the first exported function/type, or empty string.
func primarySymbolInFile(index *codeintel.SymbolIndex, filePath string) string {
	symbols := index.FileSymbols(filePath)
	for _, s := range symbols {
		if s.Exported && (s.Kind == "func" || s.Kind == "type" || s.Kind == "struct" || s.Kind == "interface") {
			return s.Name
		}
	}
	// Fallback: first symbol
	if len(symbols) > 0 {
		return symbols[0].Name
	}
	return ""
}

// scoreFile scores a file by token occurrences in filename and content.
func scoreFile(filePath string, queryTokens []string, workDir string) int {
	score := 0
	lowerPath := strings.ToLower(filePath)

	// Filename matches
	for _, token := range queryTokens {
		if strings.Contains(lowerPath, token) {
			score += 10
		}
	}

	// Content scan (cheap - read first 200 lines)
	absPath := filepath.Join(workDir, filePath)
	content, err := os.ReadFile(absPath)
	if err != nil {
		return score
	}
	lines := strings.Split(string(content), "\n")
	if len(lines) > 200 {
		lines = lines[:200]
	}
	lowerContent := strings.ToLower(string(content))
	for _, token := range queryTokens {
		if len(token) >= 3 {
			// Count occurrences
			count := strings.Count(lowerContent, token)
			score += count
		}
	}

	return score
}

// readExcerptAroundMatches returns an excerpt around the first line matching any query token.
func readExcerptAroundMatches(workDir, filePath string, queryTokens []string) (string, int, error) {
	data, err := os.ReadFile(filepath.Join(workDir, filePath))
	if err != nil {
		return "", 0, err
	}
	lines := strings.Split(strings.TrimRight(string(data), "\n"), "\n")
	if len(lines) == 0 {
		return "", 0, fmt.Errorf("empty file")
	}

	anchor := 1
	for i, ln := range lines {
		lower := strings.ToLower(ln)
		for _, token := range queryTokens {
			if len(token) >= 3 && strings.Contains(lower, token) {
				anchor = i + 1
				goto found
			}
		}
	}
found:

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

// recentCommitsForFile returns up to N recent commits touching the file.
func recentCommitsForFile(g *git.Git, filePath string, limit int) ([]git.CommitMeta, error) {
	commits, err := g.Log(limit)
	if err != nil {
		return nil, err
	}
	var metas []git.CommitMeta
	for _, c := range commits {
		metas = append(metas, git.CommitMeta{
			SHA:        c.Hash,
			Author:     c.Author,
			Summary:    c.Message,
			AuthorTime: c.Timestamp,
		})
	}
	return metas, nil
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
	if symbol != "" {
		for i, ln := range lines {
			if strings.Contains(ln, symbol) {
				anchor = i + 1
				break
			}
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