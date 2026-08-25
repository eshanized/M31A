package explain

import (
	"encoding/json"
	"fmt"
	"io"

	"github.com/eshanized/M31A/internal/core/types"
)

// noEvidenceScopes names where collection looked, rendered when a pack
// reaches the renderer with zero evidence so output is never silently
// blank (EXPLAIN-02 empty edge).
const noEvidenceScopes = "Searched scopes: indexed workspace symbols; source files under the workspace root"

// RenderText writes the human-readable explanation following the impact.go
// CLI conventions: aligned Evidence listing plus an explicit Inference
// heading whenever demoted statements exist (D-06). A zero-evidence pack
// produces an explicit no-evidence report naming the searched scopes.
func RenderText(w io.Writer, ans *ExplainAnswer, pack *types.EvidencePack) error {
	if pack == nil || len(pack.Sections) == 0 {
		fmt.Fprintln(w, fmt.Sprintf("No evidence found for query %q.", ans.Query))
		fmt.Fprintln(w, noEvidenceScopes)
		fmt.Fprintln(w, "")
		fmt.Fprintln(w, "Nothing could be grounded in collected evidence — no explanation is offered.")
		return nil
	}

	fmt.Fprintf(w, "Explanation for: %s\n", ans.Query)
	if ans.Prose != "" {
		fmt.Fprintf(w, "\n%s\n", ans.Prose)
	}

	fmt.Fprintf(w, "\nEvidence:\n")
	fmt.Fprintf(w, "  %-6s %-9s %-34s %s\n", "MARK", "KIND", "REF", "NOTE")
	for _, c := range ans.Citations {
		fmt.Fprintf(w, "  [%-4d %-9s %-34s %s\n", c.Marker, c.Kind, c.Ref, c.Note)
	}

	if len(ans.Inference) > 0 {
		fmt.Fprintf(w, "\nInference (not directly backed by collected evidence):\n")
		for _, stmt := range ans.Inference {
			fmt.Fprintf(w, "  - %s\n", stmt)
		}
	}

	fmt.Fprintf(w, "\nConfidence: %s\n", ans.Confidence)
	return nil
}

// explainAnswerJSON mirrors ExplainAnswer for wire formatting; field order
// and snake_case keys follow the D-04 structured-output convention shared
// by all intelligence commands.
type explainAnswerJSON struct {
	Query      string           `json:"query"`
	Prose      string           `json:"prose"`
	Citations  []types.Citation `json:"citations"`
	Inference  []string         `json:"inference"`
	Confidence types.Confidence `json:"confidence"`
}

// RenderJSON writes the answer as an indented JSON object carrying query,
// prose, citations[], inference[], and confidence per D-04.
func RenderJSON(w io.Writer, ans *ExplainAnswer) error {
	out := explainAnswerJSON{
		Query:      ans.Query,
		Prose:      ans.Prose,
		Citations:  ans.Citations,
		Inference:  ans.Inference,
		Confidence: ans.Confidence,
	}
	if out.Citations == nil {
		out.Citations = []types.Citation{}
	}
	if out.Inference == nil {
		out.Inference = []string{}
	}
	enc := json.NewEncoder(w)
	enc.SetIndent("", "  ")
	if err := enc.Encode(out); err != nil {
		return fmt.Errorf("explain: encode JSON: %w", err)
	}
	return nil
}
