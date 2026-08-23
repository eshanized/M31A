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

func WriteResearch(m31aDir string, research *types.Research) error {
	if research == nil {
		return errors.New("research is nil")
	}

	researchDir := filepath.Join(m31aDir, "research")
	if err := os.MkdirAll(researchDir, types.DirPermission); err != nil {
		return coreerrors.Wrap(err, "create research dir")
	}

	slug := slugify(research.Question)
	filename := fmt.Sprintf("%s-%s.md", research.ID.String()[:8], slug)
	path := filepath.Join(researchDir, filename)

	content := formatResearch(research)

	// Atomic write
	if err := types.AtomicWrite(path, []byte(content)); err != nil {
		return coreerrors.Wrap(err, "write research")
	}
	return nil
}

func ListResearch(m31aDir string) ([]*types.Research, error) {
	researchDir := filepath.Join(m31aDir, "research")
	entries, err := os.ReadDir(researchDir)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return []*types.Research{}, nil
		}
		return nil, coreerrors.Wrap(err, "read research dir")
	}

	var researchList []*types.Research
	for _, entry := range entries {
		if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".md") {
			path := filepath.Join(researchDir, entry.Name())
			research, err := parseResearchFile(path)
			if err != nil {
				// Log warning but continue
				continue
			}
			researchList = append(researchList, research)
		}
	}

	// Sort by created_at descending (newest first)
	sort.Slice(researchList, func(i, j int) bool {
		return researchList[i].CreatedAt.After(researchList[j].CreatedAt)
	})

	return researchList, nil
}


func formatResearch(r *types.Research) string {
	var b strings.Builder
	b.WriteString(fmt.Sprintf("# Research: %s\n\n", r.Question))
	b.WriteString(fmt.Sprintf("**ID:** %s\n\n", r.ID.String()))
	b.WriteString(fmt.Sprintf("**Confidence:** %.2f\n\n", r.Confidence))
	b.WriteString(fmt.Sprintf("**Created:** %s\n\n", r.CreatedAt.Format(time.RFC3339)))
	b.WriteString(fmt.Sprintf("**Created By:** %s\n\n", r.CreatedBy.String()))

	if len(r.Sources) > 0 {
		b.WriteString("## Sources\n\n")
		for _, src := range r.Sources {
			b.WriteString(fmt.Sprintf("- %s\n", src))
		}
		b.WriteString("\n")
	}

	b.WriteString("## Findings\n\n")
	b.WriteString(r.Findings + "\n")

	return b.String()
}

func parseResearchFile(path string) (*types.Research, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	lines := strings.Split(string(content), "\n")
	var research types.Research
	var inFindings bool
	var findings strings.Builder

	for _, line := range lines {
		line = strings.TrimSpace(line)

		if strings.HasPrefix(line, "# Research: ") {
			research.Question = strings.TrimPrefix(line, "# Research: ")
		} else if strings.HasPrefix(line, "**ID:** ") {
			// UUID parsing would go here
		} else if strings.HasPrefix(line, "**Confidence:** ") {
			confStr := strings.TrimPrefix(line, "**Confidence:** ")
			fmt.Sscanf(confStr, "%f", &research.Confidence)
		} else if strings.HasPrefix(line, "**Created:** ") {
			tsStr := strings.TrimPrefix(line, "**Created:** ")
			if ts, err := time.Parse(time.RFC3339, tsStr); err == nil {
				research.CreatedAt = ts
			}
		} else if strings.HasPrefix(line, "**Created By:** ") {
			// UUID parsing would go here
		} else if line == "## Sources" {
			inFindings = false
		} else if strings.HasPrefix(line, "- ") && !inFindings {
			research.Sources = append(research.Sources, strings.TrimPrefix(line, "- "))
		} else if line == "## Findings" {
			inFindings = true
		} else if inFindings {
			findings.WriteString(line + "\n")
		}
	}

	research.Findings = strings.TrimSpace(findings.String())
	return &research, nil
}