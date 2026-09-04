package investigate

import (
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/intelligence/explain"
	"github.com/google/uuid"
)

// AffectedComponent represents a component affected by the culprit change.
type AffectedComponent struct {
	Name  string `json:"name"`
	Depth int    `json:"depth"`
}

// RootCauseReport is the two-tier root-cause report per REGRESS-02/03.
type RootCauseReport struct {
	Symptom             string               `json:"symptom"`
	Baseline            string               `json:"baseline"`
	Head                string               `json:"head"`
	WindowSize          int                  `json:"window_size"`
	Steps               []BisectStep         `json:"steps"`
	Outcome             string               `json:"outcome"`
	CulpritSHA          string               `json:"culprit_sha"`
	CulpritConfidence   types.Confidence     `json:"culprit_confidence"`
	CulpritSubject      string               `json:"culprit_subject"`
	Mechanism           string               `json:"mechanism"`
	MechanismConfidence types.Confidence     `json:"mechanism_confidence"`
	AffectedComponents  []AffectedComponent  `json:"affected_components"`
	FixDirection        string               `json:"fix_direction"`
	Evidence            []string             `json:"evidence"`
}

// ReportDeps holds dependencies for building the root cause report.
type ReportDeps struct {
	Manager    *WorktreeManager
	Runner     InvestigateGitRunner
	Graph      *codeintel.CodeGraph
	Index      *codeintel.SymbolIndex
	GitClient  *git.Git
	Synth      explain.Synthesizer
	ModelID    string
	MaxCommits int
}

// InvestigationStartedPayload is the payload for InvestigationStarted event.
type InvestigationStartedPayload struct {
	Symptom    string
	Baseline   string
	Head       string
	WindowSize int
}

// InvestigationCompletedPayload is the payload for InvestigationCompleted event.
type InvestigationCompletedPayload struct {
	Outcome              string
	CulpritSHA           string
	CulpritConfidence    string
	MechanismConfidence  string
}

// BuildRootCauseReport orchestrates the full investigation: pre-checks,
// candidates, FindCulprit, confirmation run, culprit diff analysis,
// affected components via AnalyzeImpact, and optional mechanism synthesis.
func BuildRootCauseReport(
	ctx context.Context,
	deps ReportDeps,
	symptom, baseline, head, reproCmd string,
) (*RootCauseReport, error) {
	report := &RootCauseReport{
		Symptom:    symptom,
		Baseline:   baseline,
		Head:       head,
		WindowSize: deps.MaxCommits,
		Steps:      []BisectStep{},
		Outcome:    "unattributable",
		Evidence:   []string{},
		AffectedComponents: []AffectedComponent{},
	}

	// Emit investigation started event
	_ = EmitInvestigationStarted(ctx, nil, InvestigationStartedPayload{
		Symptom:    symptom,
		Baseline:   baseline,
		Head:       head,
		WindowSize: deps.MaxCommits,
	})

	// Create worktree at head (bad) commit
	if _, err := deps.Manager.Create(head); err != nil {
		report.Outcome = "unattributable"
		report.Evidence = append(report.Evidence, "worktree create failed: "+err.Error())
		_ = EmitInvestigationCompleted(ctx, nil, InvestigationCompletedPayload{
			Outcome:             report.Outcome,
			CulpritConfidence:   string(types.ConfidenceSpeculative),
			MechanismConfidence: string(types.ConfidenceSpeculative),
		})
		return report, nil
	}
	// Ensure cleanup on all paths
	defer deps.Manager.Remove()

	// Resolve repro command
	resolvedRepro, err := ResolveReproCommand("", "", deps.GitClient.WorkDir())
	if err != nil {
		// If no repro command can be resolved, use the provided one
		if reproCmd != "" {
			resolvedRepro = reproCmd
		} else {
			report.Outcome = "unattributable"
			report.Evidence = append(report.Evidence, "no repro command available: "+err.Error())
			_ = EmitInvestigationCompleted(ctx, nil, InvestigationCompletedPayload{
				Outcome:             report.Outcome,
				CulpritConfidence:   string(types.ConfidenceSpeculative),
				MechanismConfidence: string(types.ConfidenceSpeculative),
			})
			return report, nil
		}
	}
	if reproCmd != "" {
		resolvedRepro = reproCmd
	}

	// Compile symptom regex from the symptom text (simple approach: use as-is)
	// In practice, the symptom text could be used to generate a regex
	symptomRegex := regexp.MustCompile(regexp.QuoteMeta(symptom))

	// Define the check function that runs the repro command in the worktree
	checkFn := func(ctx context.Context, sha string) bool {
		// Checkout to the commit
		if err := deps.Manager.CheckoutTo(sha); err != nil {
			return false
		}
		// Execute repro
		wtDir := deps.Manager.WorktreeDir()
		result, err := ExecuteRepro(ctx, wtDir, resolvedRepro, symptomRegex)
		if err != nil {
			return false
		}
		// Return true if the test passes (exit code 0 and no symptom match)
		// Return false if the test fails (non-zero exit or symptom matched)
		if result.ExitCode != 0 {
			return false
		}
		if symptomRegex != nil && symptomRegex.MatchString(result.Output) {
			return false
		}
		return true
	}

	// Run FindCulprit
	culpritSHA, steps, err := FindCulprit(ctx, deps.Manager, deps.Runner, baseline, head, deps.MaxCommits, checkFn, func(format string, args ...any) {
		report.Evidence = append(report.Evidence, fmt.Sprintf(format, args...))
	})
	report.Steps = steps

	if err != nil {
		// Check if it's a NotReproducibleInWindowError
		if IsNotReproducibleInWindow(err) {
			report.Outcome = "not-reproducible-in-window"
			report.CulpritConfidence = types.ConfidenceSpeculative
			report.MechanismConfidence = types.ConfidenceSpeculative
			if nrwe, ok := err.(*NotReproducibleInWindowError); ok {
				report.Evidence = append(report.Evidence, fmt.Sprintf("sentinel: checked %s, observed %s (window size %d)", nrwe.CheckedSHA, nrwe.ObservedResult, nrwe.CandidateWindow))
			}
			_ = EmitInvestigationCompleted(ctx, nil, InvestigationCompletedPayload{
				Outcome:             report.Outcome,
				CulpritConfidence:   string(report.CulpritConfidence),
				MechanismConfidence: string(report.MechanismConfidence),
			})
			return report, nil
		}
		// Other error (iteration cap, etc.)
		report.Outcome = "unattributable"
		report.CulpritConfidence = types.ConfidenceSpeculative
		report.MechanismConfidence = types.ConfidenceSpeculative
		report.Evidence = append(report.Evidence, "error: "+err.Error())
		_ = EmitInvestigationCompleted(ctx, nil, InvestigationCompletedPayload{
			Outcome:             report.Outcome,
			CulpritConfidence:   string(report.CulpritConfidence),
			MechanismConfidence: string(report.MechanismConfidence),
		})
		return report, nil
	}

	// Confirmation passed - culprit fails, parent passes
	report.Outcome = "attributed"
	report.CulpritSHA = culpritSHA
	report.CulpritConfidence = types.ConfidenceVerified

	// Get culprit subject
	if deps.GitClient != nil {
		out, _ := deps.GitClient.Run("log", "-1", "--format=%s", culpritSHA)
		report.CulpritSubject = strings.TrimSpace(out)
	}

	// Get culprit diff and analyze affected components
	if deps.GitClient != nil && deps.Graph != nil && deps.Index != nil {
		// Get parent SHA for diff
		candidates, _ := Candidates(deps.Runner, baseline, head, deps.MaxCommits)
		var parentSHA string
		for i, c := range candidates {
			if c == culpritSHA {
				if i > 0 {
					parentSHA = candidates[i-1]
				} else {
					parentSHA = baseline
				}
				break
			}
		}

		diffOut, _ := deps.GitClient.Run("diff", parentSHA+".."+culpritSHA)
		report.Evidence = append(report.Evidence, fmt.Sprintf("culprit diff (%s..%s): %d lines", parentSHA, culpritSHA, len(strings.Split(diffOut, "\n"))))

		// Extract symbols from diff
		symbols := extractSymbolsFromDiff(diffOut, deps.Index)
		report.Evidence = append(report.Evidence, fmt.Sprintf("symbols in diff: %v", symbols))

		// Analyze impact for each symbol
		affectedMap := make(map[string]AffectedComponent) // name -> component (keep minimal depth)
		for _, sym := range symbols {
			impact := codeintel.AnalyzeImpact(deps.Graph, deps.Index, sym, 3)
			if impact != nil {
				// Add direct callers
				for _, caller := range impact.DirectCallers {
					key := caller.File
					if existing, ok := affectedMap[key]; !ok || caller.Line < existing.Depth {
						affectedMap[key] = AffectedComponent{Name: key, Depth: caller.Line}
					}
				}
				// Add indirect dependents
				for _, dep := range impact.Indirect {
					if existing, ok := affectedMap[dep]; !ok || 1 < existing.Depth {
						affectedMap[dep] = AffectedComponent{Name: dep, Depth: 1}
					}
				}
				// Add affected tests
				for _, test := range impact.AffectedTests {
					if existing, ok := affectedMap[test]; !ok || 0 < existing.Depth {
						affectedMap[test] = AffectedComponent{Name: test, Depth: 0}
					}
				}
			}
		}

		// Convert map to slice and sort by depth then name
		for _, comp := range affectedMap {
			report.AffectedComponents = append(report.AffectedComponents, comp)
		}
		// Sort by depth ascending, then name ascending
		for i := 0; i < len(report.AffectedComponents); i++ {
			for j := i + 1; j < len(report.AffectedComponents); j++ {
				if report.AffectedComponents[j].Depth < report.AffectedComponents[i].Depth ||
					(report.AffectedComponents[j].Depth == report.AffectedComponents[i].Depth &&
						report.AffectedComponents[j].Name < report.AffectedComponents[i].Name) {
					report.AffectedComponents[i], report.AffectedComponents[j] = report.AffectedComponents[j], report.AffectedComponents[i]
				}
			}
		}

		// Synthesize mechanism if synthesizer is available
		if deps.Synth != nil {
			// Build evidence pack for synthesis
			pack := types.NewEvidencePack(symptom)
			pack.Add(types.EvidenceCommit, culpritSHA, report.CulpritSubject)
			if deps.GitClient != nil {
				diffOut, _ := deps.GitClient.Run("diff", parentSHA+".."+culpritSHA)
				pack.Add(types.EvidenceSource, "diff", diffOut)
			}
			// Add affected components as evidence
			for _, comp := range report.AffectedComponents {
				pack.Add(types.EvidenceSource, comp.Name, fmt.Sprintf("depth=%d", comp.Depth))
			}

			ans, err := explain.Synthesize(ctx, pack, deps.Synth, deps.ModelID, nil)
			if err != nil {
				report.Evidence = append(report.Evidence, "synthesis failed: "+err.Error())
				report.MechanismConfidence = types.ConfidenceSpeculative
			} else {
				report.Mechanism = ans.Prose
				// Extract fix direction from inference or prose
				if len(ans.Inference) > 0 {
					report.FixDirection = ans.Inference[0]
				} else if strings.Contains(ans.Prose, "Fix:") {
					parts := strings.Split(ans.Prose, "Fix:")
					if len(parts) > 1 {
						report.FixDirection = strings.TrimSpace(parts[1])
					}
				}
				report.MechanismConfidence = ans.Confidence
				if report.MechanismConfidence == types.ConfidenceVerified {
					report.MechanismConfidence = types.ConfidenceLikely // ceiling per D-12
				}
			}
		} else {
			report.MechanismConfidence = types.ConfidenceSpeculative
			report.Evidence = append(report.Evidence, "no synthesizer provided")
		}
	}

	_ = EmitInvestigationCompleted(ctx, nil, InvestigationCompletedPayload{
		Outcome:              report.Outcome,
		CulpritSHA:           report.CulpritSHA,
		CulpritConfidence:    string(report.CulpritConfidence),
		MechanismConfidence:  string(report.MechanismConfidence),
	})

	return report, nil
}

// extractSymbolsFromDiff extracts Go symbols from a diff output.
func extractSymbolsFromDiff(diff string, index *codeintel.SymbolIndex) []string {
	var symbols []string
	seen := make(map[string]bool)

	// Look for function definitions in diff: func Name(...)
	funcRe := regexp.MustCompile(`func\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(`)
	for _, match := range funcRe.FindAllStringSubmatch(diff, -1) {
		if len(match) > 1 {
			name := match[1]
			if !seen[name] && index.Define(name) != nil {
				symbols = append(symbols, name)
				seen[name] = true
			}
		}
	}

	// Look for type definitions: type Name ...
	typeRe := regexp.MustCompile(`type\s+([A-Za-z_][A-Za-z0-9_]*)\s+(struct|interface|func)`)
	for _, match := range typeRe.FindAllStringSubmatch(diff, -1) {
		if len(match) > 1 {
			name := match[1]
			if !seen[name] && index.Define(name) != nil {
				symbols = append(symbols, name)
				seen[name] = true
			}
		}
	}

	return symbols
}

// EmitInvestigationStarted emits the InvestigationStarted event.
func EmitInvestigationStarted(ctx context.Context, store types.EventStore, payload InvestigationStartedPayload) error {
	if store == nil {
		return nil
	}
	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	evt := types.Event{
		ID:        uuid.New(),
		Type:      types.EventInvestigationStarted,
		Timestamp: time.Now(),
		Payload:   data,
		Metadata: types.EventMetadata{
			SchemaVersion: 1,
			Source:        "intelligence",
		},
	}
	return store.Append(ctx, evt)
}

// EmitInvestigationCompleted emits the InvestigationCompleted event.
func EmitInvestigationCompleted(ctx context.Context, store types.EventStore, payload InvestigationCompletedPayload) error {
	if store == nil {
		return nil
	}
	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	evt := types.Event{
		ID:        uuid.New(),
		Type:      types.EventInvestigationCompleted,
		Timestamp: time.Now(),
		Payload:   data,
		Metadata: types.EventMetadata{
			SchemaVersion: 1,
			Source:        "intelligence",
		},
	}
	return store.Append(ctx, evt)
}