package workflow

import (
	"regexp"
	"strings"
	"sync"

	m31types "github.com/eshanized/M31A/pkg/types"
)

// Pre-compiled regex patterns — avoids recompilation on every ParsePlan call.
var (
	reTitle      = regexp.MustCompile(`(?m)^#\s+(.+)$`)
	reNextH2     = regexp.MustCompile(`(?m)^##\s+`)
	reNextH3     = regexp.MustCompile(`(?m)^###\s+`)
	reBlockquote = regexp.MustCompile(`(?m)^>\s?`)
	reReviewNote = regexp.MustCompile(`(?m)>\s*\[!(\w+)\]\s*\n((?:>\s*.+\n?)+)`)
	reQuestion   = regexp.MustCompile(`(?m)^\d+\.\s+(.+?)(?:\s*[—-]\s*(.+))?$`)
	reH3         = regexp.MustCompile(`(?m)^###\s+(.+)$`)
	reH4         = regexp.MustCompile(`(?m)^####\s+\[(NEW|MODIFY)\]\s+(.+)$`)
)

// Cached compiled regexes for section/subsection headers.
// Keys are the section name strings; values are *regexp.Regexp.
var (
	sectionHeaderCache    sync.Map
	subsectionHeaderCache sync.Map
)

// sectionHeaderRe returns a compiled regex for matching an H2 header with the given name.
// Results are cached to avoid recompilation on every call.
func sectionHeaderRe(name string) *regexp.Regexp {
	if v, ok := sectionHeaderCache.Load(name); ok {
		if re, ok := v.(*regexp.Regexp); ok {
			return re
		}
	}
	re := regexp.MustCompile(`(?im)^##\s+` + regexp.QuoteMeta(name) + `\s*$`)
	sectionHeaderCache.Store(name, re)
	return re
}

// subsectionHeaderRe returns a compiled regex for matching an H3 header with the given name.
// Results are cached to avoid recompilation on every call.
func subsectionHeaderRe(name string) *regexp.Regexp {
	if v, ok := subsectionHeaderCache.Load(name); ok {
		if re, ok := v.(*regexp.Regexp); ok {
			return re
		}
	}
	re := regexp.MustCompile(`(?im)^###\s+` + regexp.QuoteMeta(name) + `\s*$`)
	subsectionHeaderCache.Store(name, re)
	return re
}

// ParsePlan extracts a structured Plan from rich markdown content.
// The markdown is expected to follow the format defined in plan-format.md.
// Sections that cannot be parsed are left empty rather than causing an error.
func ParsePlan(markdown string) (*m31types.Plan, error) {
	plan := &m31types.Plan{
		RawMarkdown: markdown,
		Version:     1,
	}

	plan.Title = extractTitle(markdown)
	plan.Summary = extractSection(markdown, "Summary")

	reviewSection := extractSection(markdown, "User Review Required")
	plan.ReviewNotes = extractReviewNotes(reviewSection)

	questionsSection := extractSection(markdown, "Open Questions")
	plan.OpenQuestions = extractOpenQuestions(questionsSection)

	changesSection := extractSection(markdown, "Proposed Changes")
	plan.ProposedChanges = extractProposedChanges(changesSection)

	verifySection := extractSection(markdown, "Verification Plan")
	plan.Verification = extractVerificationPlan(verifySection)

	tasks, err := extractTasksFromPlan(markdown)
	if err == nil {
		plan.Tasks = tasks
	}

	return plan, nil
}

// extractTitle returns the first H1 heading from the markdown.
func extractTitle(markdown string) string {
	m := reTitle.FindStringSubmatch(markdown)
	if len(m) > 1 {
		return strings.TrimSpace(m[1])
	}
	return ""
}

// extractSection returns the content between a given H2 header and the next H2 header.
func extractSection(markdown, header string) string {
	headerRe := sectionHeaderRe(header)
	loc := headerRe.FindStringIndex(markdown)
	if loc == nil {
		return ""
	}
	start := loc[1]

	nextLoc := reNextH2.FindStringIndex(markdown[start:])
	if nextLoc != nil {
		return strings.TrimSpace(markdown[start : start+nextLoc[0]])
	}
	return strings.TrimSpace(markdown[start:])
}

// extractReviewNotes parses [!IMPORTANT] and [!WARNING] admonition blocks.
func extractReviewNotes(section string) []m31types.ReviewNote {
	if section == "" {
		return nil
	}
	var notes []m31types.ReviewNote
	matches := reReviewNote.FindAllStringSubmatch(section, -1)
	for _, m := range matches {
		if len(m) > 2 {
			level := strings.TrimSpace(m[1])
			text := strings.TrimSpace(m[2])
			text = reBlockquote.ReplaceAllString(text, "")
			notes = append(notes, m31types.ReviewNote{
				Level: level,
				Text:  strings.TrimSpace(text),
			})
		}
	}
	return notes
}

// extractOpenQuestions parses numbered questions with optional suggested defaults.
func extractOpenQuestions(section string) []m31types.OpenQuestion {
	if section == "" {
		return nil
	}
	if strings.Contains(section, "No open questions") {
		return nil
	}
	var questions []m31types.OpenQuestion
	matches := reQuestion.FindAllStringSubmatch(section, -1)
	for _, m := range matches {
		if len(m) > 1 {
			q := strings.TrimSpace(m[1])
			suggestion := ""
			if len(m) > 2 {
				suggestion = strings.TrimSpace(m[2])
			}
			if len(q) > 5 {
				questions = append(questions, m31types.OpenQuestion{
					Question:   q,
					Suggestion: suggestion,
				})
			}
		}
	}
	return questions
}

// extractProposedChanges parses category groups and [NEW]/[MODIFY] file entries.
func extractProposedChanges(section string) []m31types.ProposedChangeGroup {
	if section == "" {
		return nil
	}
	var groups []m31types.ProposedChangeGroup

	h3Matches := reH3.FindAllStringSubmatchIndex(section, -1)

	for i, h3Match := range h3Matches {
		category := strings.TrimSpace(section[h3Match[2]:h3Match[3]])
		start := h3Match[1]
		end := len(section)
		if i+1 < len(h3Matches) {
			end = h3Matches[i+1][0]
		}
		chunk := section[start:end]

		var changes []m31types.ProposedChange
		h4IndexMatches := reH4.FindAllStringSubmatchIndex(chunk, -1)
		for _, h4Idx := range h4IndexMatches {
			if len(h4Idx) >= 6 {
				action := strings.TrimSpace(chunk[h4Idx[2]:h4Idx[3]])
				file := strings.TrimSpace(chunk[h4Idx[4]:h4Idx[5]])
				changes = append(changes, m31types.ProposedChange{
					Action:      action,
					File:        file,
					Description: extractFileDescription(chunk, h4Idx[0]),
				})
			}
		}

		if len(changes) > 0 {
			groups = append(groups, m31types.ProposedChangeGroup{
				Category: category,
				Changes:  changes,
			})
		}
	}
	return groups
}

// extractFileDescription gets the first bullet-point description after a file header.
func extractFileDescription(chunk string, headerPos int) string {
	after := chunk[headerPos:]
	lines := strings.SplitN(after, "\n", 5)
	for _, line := range lines[1:] {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "- ") {
			return strings.TrimPrefix(trimmed, "- ")
		}
		if strings.HasPrefix(trimmed, "####") {
			break
		}
	}
	return ""
}

// extractVerificationPlan parses automated and manual verification subsections.
func extractVerificationPlan(section string) m31types.VerificationPlan {
	if section == "" {
		return m31types.VerificationPlan{}
	}
	return m31types.VerificationPlan{
		Automated: extractBulletList(extractSubsection(section, "Automated")),
		Manual:    extractBulletList(extractSubsection(section, "Manual")),
	}
}

// extractSubsection returns content under an H3 header within a section.
func extractSubsection(section, header string) string {
	headerRe := subsectionHeaderRe(header)
	loc := headerRe.FindStringIndex(section)
	if loc == nil {
		return ""
	}
	start := loc[1]

	nextLoc := reNextH3.FindStringIndex(section[start:])
	if nextLoc != nil {
		return strings.TrimSpace(section[start : start+nextLoc[0]])
	}
	return strings.TrimSpace(section[start:])
}

// extractBulletList parses bullet-pointed items from text.
func extractBulletList(text string) []string {
	if text == "" {
		return nil
	}
	var items []string
	for _, line := range strings.Split(text, "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "- ") {
			items = append(items, strings.TrimPrefix(trimmed, "- "))
		}
	}
	return items
}

// extractTasksFromPlan finds the JSON task array embedded in the plan's
// "Task List" section and delegates to the existing JSON parser.
func extractTasksFromPlan(markdown string) ([]m31types.Task, error) {
	taskSection := extractSection(markdown, "Task List")
	if taskSection == "" {
		return nil, nil
	}
	jsonStr := extractJSONArray(taskSection)
	if jsonStr == "" {
		return nil, nil
	}
	return parseTasksFromJSON(jsonStr)
}
