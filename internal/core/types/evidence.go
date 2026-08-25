package types

// EvidenceKind identifies the provenance category of a collected evidence
// item. Intelligence collectors tag every item so citations can be rendered
// with their source class (source excerpt, callers, blame, commit, ADR, test).
type EvidenceKind string

const (
	// EvidenceSource marks an excerpt taken directly from a source file.
	EvidenceSource EvidenceKind = "source"
	// EvidenceCallers marks caller information derived from the symbol graph.
	EvidenceCallers EvidenceKind = "callers"
	// EvidenceBlame marks git blame attribution for a line or symbol.
	EvidenceBlame EvidenceKind = "blame"
	// EvidenceCommit marks commit metadata (SHA, subject, author, date).
	EvidenceCommit EvidenceKind = "commit"
	// EvidenceADR marks an architecture decision record excerpt.
	EvidenceADR EvidenceKind = "adr"
	// EvidenceTest marks a related test file or test function reference.
	EvidenceTest EvidenceKind = "test"
)

// Evidence is a single collected fact backing an intelligence answer. Items
// are assembled into an EvidencePack by deterministic collectors; the LLM
// narrates over them but never invents them (D-05). Ref is stored and emitted
// byte-for-byte as collected — no unicode normalization is applied (D-04
// verbatim-path backstop). JSON tags are snake_case for --format json output.
type Evidence struct {
	ID      int          `json:"id"`
	Kind    EvidenceKind `json:"kind"`
	Ref     string       `json:"ref"`
	Snippet string       `json:"snippet"`
}

// EvidencePack is the bounded set of evidence collected for one intelligence
// query. Collectors fill Sections via Add; Truncated reports whether any
// section was dropped to honor the token budget. An empty pack (no Sections)
// is the explicit no-evidence signal downstream commands render as such,
// never as a silently empty report (EXPLAIN-02 empty edge).
type EvidencePack struct {
	Query     string     `json:"query"`
	Sections  []Evidence `json:"sections"`
	Truncated bool       `json:"truncated"`
}

// NewEvidencePack constructs an EvidencePack for query with non-nil Sections
// so JSON marshaling always emits an array, never null.
func NewEvidencePack(query string) *EvidencePack {
	return &EvidencePack{
		Query:    query,
		Sections: []Evidence{},
	}
}

// Add appends an evidence item to the pack and returns it. IDs are assigned
// sequentially starting at 1 (computed before append) and match the [n]
// citation markers rendered in prose. The returned pointer addresses the
// stored item inside Sections.
func (p *EvidencePack) Add(kind EvidenceKind, ref, snippet string) *Evidence {
	item := Evidence{
		ID:      len(p.Sections) + 1,
		Kind:    kind,
		Ref:     ref,
		Snippet: snippet,
	}
	p.Sections = append(p.Sections, item)
	return &p.Sections[len(p.Sections)-1]
}

// Citation is a numbered reference listed in an answer's Evidence section.
// Prose carries [Marker] markers; each marker resolves to exactly one
// Citation with its full source reference (file:line, commit SHA, ADR path)
// per D-06. Statements without any backing marker belong under the explicit
// Inference heading. JSON tags are snake_case for --format json output.
type Citation struct {
	Marker int          `json:"marker"`
	Kind   EvidenceKind `json:"kind"`
	Ref    string       `json:"ref"`
	Note   string       `json:"note"`
}
