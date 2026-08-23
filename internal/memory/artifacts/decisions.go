package artifacts

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
)

func WriteDecision(m31aDir string, decision *types.Decision) error {
	if decision == nil {
		return errors.New("decision is nil")
	}

	decisionsDir := filepath.Join(m31aDir, "decisions")
	if err := os.MkdirAll(decisionsDir, types.DirPermission); err != nil {
		return coreerrors.Wrap(err, "create decisions dir")
	}

	slug := slugify(decision.Title)
	filename := fmt.Sprintf("%s-%s.md", decision.ID.String()[:8], slug)
	path := filepath.Join(decisionsDir, filename)

	content := formatDecision(decision)

	// Atomic write
	if err := types.AtomicWrite(path, []byte(content)); err != nil {
		return coreerrors.Wrap(err, "write decision")
	}
	return nil
}

func ListDecisions(m31aDir string) ([]*types.Decision, error) {
	decisionsDir := filepath.Join(m31aDir, "decisions")
	entries, err := os.ReadDir(decisionsDir)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return []*types.Decision{}, nil
		}
		return nil, coreerrors.Wrap(err, "read decisions dir")
	}

	var decisions []*types.Decision
	for _, entry := range entries {
		if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".md") {
			path := filepath.Join(decisionsDir, entry.Name())
			decision, err := parseDecisionFile(path)
			if err != nil {
				// Log warning but continue
				continue
			}
			decisions = append(decisions, decision)
		}
	}

	// Sort by timestamp descending (newest first)
	sort.Slice(decisions, func(i, j int) bool {
		return decisions[i].Timestamp.After(decisions[j].Timestamp)
	})

	return decisions, nil
}


func formatDecision(d *types.Decision) string {
	var b strings.Builder
	b.WriteString(fmt.Sprintf("# Decision: %s\n\n", d.Title))
	b.WriteString(fmt.Sprintf("**ID:** %s\n\n", d.ID.String()))
	b.WriteString(fmt.Sprintf("**Status:** %s\n\n", d.Status))
	b.WriteString(fmt.Sprintf("**Timestamp:** %s\n\n", d.Timestamp.Format(time.RFC3339)))

	if d.Rationale != "" {
		b.WriteString("## Rationale\n\n")
		b.WriteString(d.Rationale + "\n\n")
	}

	if len(d.Alternatives) > 0 {
		b.WriteString("## Alternatives\n\n")
		for i, alt := range d.Alternatives {
			b.WriteString(fmt.Sprintf("### Alternative %d: %s\n\n", i+1, alt.Description))
			if len(alt.Pros) > 0 {
				b.WriteString("**Pros:**\n")
				for _, pro := range alt.Pros {
					b.WriteString(fmt.Sprintf("- %s\n", pro))
				}
				b.WriteString("\n")
			}
			if len(alt.Cons) > 0 {
				b.WriteString("**Cons:**\n")
				for _, con := range alt.Cons {
					b.WriteString(fmt.Sprintf("- %s\n", con))
				}
				b.WriteString("\n")
			}
		}
	}

	return b.String()
}

func parseDecisionFile(path string) (*types.Decision, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	// Simple parsing - in real implementation would use a proper markdown parser
	lines := strings.Split(string(content), "\n")
	var decision types.Decision
	var inRationale bool
	var currentAlt *types.Alternative
	var parsingPros, parsingCons bool

	for _, line := range lines {
		line = strings.TrimSpace(line)

		if strings.HasPrefix(line, "# Decision: ") {
			decision.Title = strings.TrimPrefix(line, "# Decision: ")
		} else if strings.HasPrefix(line, "**ID:** ") {
			// UUID parsing would go here
		} else if strings.HasPrefix(line, "**Status:** ") {
			decision.Status = types.DecisionStatus(strings.TrimPrefix(line, "**Status:** "))
		} else if strings.HasPrefix(line, "**Timestamp:** ") {
			tsStr := strings.TrimPrefix(line, "**Timestamp:** ")
			if ts, err := time.Parse(time.RFC3339, tsStr); err == nil {
				decision.Timestamp = ts
			}
		} else if line == "## Rationale" {
			inRationale = true
		} else if line == "## Alternatives" {
			inRationale = false
		} else if strings.HasPrefix(line, "### Alternative ") {
			if currentAlt != nil {
				decision.Alternatives = append(decision.Alternatives, *currentAlt)
			}
			currentAlt = &types.Alternative{}
			altDesc := strings.TrimPrefix(line, "### Alternative ")
			// Remove "N: " prefix
			if idx := strings.Index(altDesc, ": "); idx != -1 {
				altDesc = altDesc[idx+2:]
			}
			currentAlt.Description = altDesc
			parsingPros = false
			parsingCons = false
		} else if line == "**Pros:**" {
			parsingPros = true
			parsingCons = false
		} else if line == "**Cons:**" {
			parsingPros = false
			parsingCons = true
		} else if strings.HasPrefix(line, "- ") && inRationale {
			decision.Rationale += strings.TrimPrefix(line, "- ") + "\n"
		} else if strings.HasPrefix(line, "- ") && parsingPros && currentAlt != nil {
			currentAlt.Pros = append(currentAlt.Pros, strings.TrimPrefix(line, "- "))
		} else if strings.HasPrefix(line, "- ") && parsingCons && currentAlt != nil {
			currentAlt.Cons = append(currentAlt.Cons, strings.TrimPrefix(line, "- "))
		}
	}

	if currentAlt != nil {
		decision.Alternatives = append(decision.Alternatives, *currentAlt)
	}

	return &decision, nil
}