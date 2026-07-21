package workflow

import (
	"strings"
	"testing"
)

func TestExtractTitle_NoTitle(t *testing.T) {
	got := extractTitle("## Summary\nSome content")
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}
}

func TestExtractTitle_WithTitle(t *testing.T) {
	got := extractTitle("# My Plan\n## Summary\nContent")
	if got != "My Plan" {
		t.Errorf("expected 'My Plan', got %q", got)
	}
}

func TestExtractTitle_ExtraWhitespace(t *testing.T) {
	got := extractTitle("#   Spaced Title   \n## Summary")
	if got != "Spaced Title" {
		t.Errorf("expected 'Spaced Title', got %q", got)
	}
}

func TestExtractTitle_EmptyMarkdown(t *testing.T) {
	got := extractTitle("")
	if got != "" {
		t.Errorf("expected empty for empty markdown, got %q", got)
	}
}

func TestExtractSection_MissingHeader(t *testing.T) {
	got := extractSection("# Plan\nSome content", "Nonexistent")
	if got != "" {
		t.Errorf("expected empty for missing header, got %q", got)
	}
}

func TestExtractSection_WithContent(t *testing.T) {
	md := "# Plan\n## Summary\nThis is the summary\n## Task List\nMore stuff"
	got := extractSection(md, "Summary")
	if !strings.Contains(got, "This is the summary") {
		t.Errorf("expected content, got %q", got)
	}
	if strings.Contains(got, "Task List") {
		t.Errorf("should not contain next section, got %q", got)
	}
}

func TestExtractSection_LastSection(t *testing.T) {
	md := "# Plan\n## Summary\nLast section content"
	got := extractSection(md, "Summary")
	if !strings.Contains(got, "Last section content") {
		t.Errorf("expected content, got %q", got)
	}
}

func TestExtractSection_CaseInsensitive(t *testing.T) {
	md := "# Plan\n## summary\nCase insensitive"
	got := extractSection(md, "summary")
	if !strings.Contains(got, "Case insensitive") {
		t.Errorf("expected case-insensitive match, got %q", got)
	}
}

func TestExtractReviewNotes_Empty(t *testing.T) {
	notes := extractReviewNotes("")
	if notes != nil {
		t.Errorf("expected nil for empty, got %v", notes)
	}
}

func TestExtractReviewNotes_WithImportant(t *testing.T) {
	section := `> [!IMPORTANT]
> This needs review
> Second line`
	notes := extractReviewNotes(section)
	if len(notes) != 1 {
		t.Fatalf("expected 1 note, got %d", len(notes))
	}
	if notes[0].Level != "IMPORTANT" {
		t.Errorf("expected IMPORTANT, got %q", notes[0].Level)
	}
	if !strings.Contains(notes[0].Text, "This needs review") {
		t.Errorf("expected note text, got %q", notes[0].Text)
	}
}

func TestExtractReviewNotes_WithWarning(t *testing.T) {
	section := `> [!WARNING]
> Danger zone`
	notes := extractReviewNotes(section)
	if len(notes) != 1 {
		t.Fatalf("expected 1 note, got %d", len(notes))
	}
	if notes[0].Level != "WARNING" {
		t.Errorf("expected WARNING, got %q", notes[0].Level)
	}
}

func TestExtractReviewNotes_Multiple(t *testing.T) {
	section := `> [!IMPORTANT]
> First note

> [!WARNING]
> Second note`
	notes := extractReviewNotes(section)
	if len(notes) != 2 {
		t.Errorf("expected 2 notes, got %d", len(notes))
	}
}

func TestExtractOpenQuestions_Empty(t *testing.T) {
	questions := extractOpenQuestions("")
	if questions != nil {
		t.Errorf("expected nil, got %v", questions)
	}
}

func TestExtractOpenQuestions_NoOpenQuestions(t *testing.T) {
	questions := extractOpenQuestions("No open questions in this plan")
	if questions != nil {
		t.Errorf("expected nil for 'No open questions', got %v", questions)
	}
}

func TestExtractOpenQuestions_WithSuggestions(t *testing.T) {
	section := `1. What framework — Gin is recommended
2. What database — PostgreSQL default`
	questions := extractOpenQuestions(section)
	if len(questions) != 2 {
		t.Fatalf("expected 2 questions, got %d", len(questions))
	}
	if questions[0].Suggestion != "Gin is recommended" {
		t.Errorf("expected suggestion 'Gin is recommended', got %q", questions[0].Suggestion)
	}
}

func TestExtractOpenQuestions_ShortFiltered(t *testing.T) {
	section := `1. Q`
	questions := extractOpenQuestions(section)
	if len(questions) != 0 {
		t.Errorf("expected 0 questions (too short), got %d", len(questions))
	}
}

func TestExtractOpenQuestions_WithoutSuggestion(t *testing.T) {
	section := `1. What language should we use for this project`
	questions := extractOpenQuestions(section)
	if len(questions) != 1 {
		t.Fatalf("expected 1 question, got %d", len(questions))
	}
	if questions[0].Suggestion != "" {
		t.Errorf("expected empty suggestion, got %q", questions[0].Suggestion)
	}
}

func TestExtractProposedChanges_Empty(t *testing.T) {
	groups := extractProposedChanges("")
	if groups != nil {
		t.Errorf("expected nil, got %v", groups)
	}
}

func TestExtractProposedChanges_WithChanges(t *testing.T) {
	section := `### Setup & Configuration
#### [NEW] go.mod
- Initialize Go module
#### [MODIFY] config.toml
- Update settings

### Core Components
#### [NEW] main.go
- Entry point`
	groups := extractProposedChanges(section)
	if len(groups) != 2 {
		t.Fatalf("expected 2 groups, got %d", len(groups))
	}
	if groups[0].Category != "Setup & Configuration" {
		t.Errorf("expected 'Setup & Configuration', got %q", groups[0].Category)
	}
	if len(groups[0].Changes) != 2 {
		t.Fatalf("expected 2 changes in first group, got %d", len(groups[0].Changes))
	}
	if groups[0].Changes[0].Action != "NEW" {
		t.Errorf("expected NEW action, got %q", groups[0].Changes[0].Action)
	}
	if groups[0].Changes[0].File != "go.mod" {
		t.Errorf("expected 'go.mod', got %q", groups[0].Changes[0].File)
	}
	if !strings.Contains(groups[0].Changes[0].Description, "Initialize Go module") {
		t.Errorf("expected description, got %q", groups[0].Changes[0].Description)
	}
	if groups[0].Changes[1].Action != "MODIFY" {
		t.Errorf("expected MODIFY action, got %q", groups[0].Changes[1].Action)
	}
}

func TestExtractVerificationPlan_Empty(t *testing.T) {
	vp := extractVerificationPlan("")
	if vp.Automated != nil || vp.Manual != nil {
		t.Errorf("expected empty verification plan, got %+v", vp)
	}
}

func TestExtractVerificationPlan_WithContent(t *testing.T) {
	section := `### Automated
- Run go build
- Run go test
### Manual
- Check UI renders correctly`
	vp := extractVerificationPlan(section)
	if len(vp.Automated) != 2 {
		t.Errorf("expected 2 automated items, got %d", len(vp.Automated))
	}
	if len(vp.Manual) != 1 {
		t.Errorf("expected 1 manual item, got %d", len(vp.Manual))
	}
	if vp.Automated[0] != "Run go build" {
		t.Errorf("expected 'Run go build', got %q", vp.Automated[0])
	}
}

func TestExtractSubsection_Missing(t *testing.T) {
	got := extractSubsection("### Automated\n- item", "Nonexistent")
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}
}

func TestExtractSubsection_WithContent(t *testing.T) {
	section := `### Automated
- Run tests
### Manual
- Check UI`
	got := extractSubsection(section, "Automated")
	if !strings.Contains(got, "Run tests") {
		t.Errorf("expected 'Run tests', got %q", got)
	}
	if strings.Contains(got, "Check UI") {
		t.Errorf("should not contain next subsection, got %q", got)
	}
}

func TestExtractSubsection_LastSubsection(t *testing.T) {
	section := `### Manual
- Last subsection content`
	got := extractSubsection(section, "Manual")
	if !strings.Contains(got, "Last subsection content") {
		t.Errorf("expected content, got %q", got)
	}
}

func TestExtractBulletList_Empty(t *testing.T) {
	items := extractBulletList("")
	if items != nil {
		t.Errorf("expected nil, got %v", items)
	}
}

func TestExtractBulletList_WithItems(t *testing.T) {
	text := `- item one
- item two
- item three`
	items := extractBulletList(text)
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(items))
	}
	if items[0] != "item one" {
		t.Errorf("expected 'item one', got %q", items[0])
	}
}

func TestExtractBulletList_NonBulletLinesIgnored(t *testing.T) {
	text := `- bullet
non-bullet
- another bullet`
	items := extractBulletList(text)
	if len(items) != 2 {
		t.Errorf("expected 2 items, got %d", len(items))
	}
}

func TestExtractTasksFromPlan_NoTaskSection(t *testing.T) {
	md := "# Plan\n## Summary\nSome content"
	tasks, err := extractTasksFromPlan(md)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tasks != nil {
		t.Errorf("expected nil tasks, got %v", tasks)
	}
}

func TestExtractTasksFromPlan_WithTasks(t *testing.T) {
	md := "# Plan\n## Task List\n```json\n[{\"id\":1,\"action\":\"Create\",\"description\":\"test\",\"dependencies\":[],\"files\":[],\"acceptance_criteria\":[]}]\n```"
	tasks, err := extractTasksFromPlan(md)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(tasks) != 1 {
		t.Fatalf("expected 1 task, got %d", len(tasks))
	}
}

func TestExtractFileDescription_WithBullet(t *testing.T) {
	chunk := `#### [NEW] test.go
- This is a file description
#### [MODIFY] other.go
- Other desc`
	got := extractFileDescription(chunk, 0)
	if got != "This is a file description" {
		t.Errorf("expected description, got %q", got)
	}
}

func TestExtractFileDescription_NoBullet(t *testing.T) {
	chunk := `#### [NEW] test.go
some text without bullet prefix`
	got := extractFileDescription(chunk, 0)
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}
}

func TestExtractFileDescription_NextHeaderBreaks(t *testing.T) {
	chunk := `#### [NEW] test.go
#### [MODIFY] other.go
- Should not reach`
	got := extractFileDescription(chunk, 0)
	if got != "" {
		t.Errorf("expected empty (hit next header), got %q", got)
	}
}

func TestParsePlan_FullDocument(t *testing.T) {
	md := `# Implementation Plan

## Summary
This plan builds a web server.

## User Review Required
> [!IMPORTANT]
> Review the API design

## Open Questions
1. What port — default 8080
2. What framework

## Proposed Changes
### Core
#### [NEW] main.go
- Entry point with HTTP server

## Verification Plan
### Automated
- Run go build
### Manual
- Test in browser

## Task List
` + "```" + `json
[{"id":1,"action":"Create","description":"Create main.go","dependencies":[],"files":["main.go"],"acceptance_criteria":["compiles"]}]
` + "```"

	plan, err := ParsePlan(md)
	if err != nil {
		t.Fatalf("ParsePlan failed: %v", err)
	}
	if plan.Title != "Implementation Plan" {
		t.Errorf("expected title 'Implementation Plan', got %q", plan.Title)
	}
	if !strings.Contains(plan.Summary, "web server") {
		t.Errorf("expected summary about web server, got %q", plan.Summary)
	}
	if len(plan.ReviewNotes) != 1 {
		t.Errorf("expected 1 review note, got %d", len(plan.ReviewNotes))
	}
	if len(plan.OpenQuestions) != 2 {
		t.Errorf("expected 2 open questions, got %d", len(plan.OpenQuestions))
	}
	if plan.OpenQuestions[0].Suggestion != "default 8080" {
		t.Errorf("expected suggestion 'default 8080', got %q", plan.OpenQuestions[0].Suggestion)
	}
	if len(plan.ProposedChanges) != 1 {
		t.Errorf("expected 1 proposed change group, got %d", len(plan.ProposedChanges))
	}
	if len(plan.Verification.Automated) != 1 {
		t.Errorf("expected 1 automated step, got %d", len(plan.Verification.Automated))
	}
	if len(plan.Verification.Manual) != 1 {
		t.Errorf("expected 1 manual step, got %d", len(plan.Verification.Manual))
	}
	if len(plan.Tasks) != 1 {
		t.Errorf("expected 1 task, got %d", len(plan.Tasks))
	}
	if plan.Version != 1 {
		t.Errorf("expected version 1, got %d", plan.Version)
	}
}

func TestParsePlan_EmptyDocument(t *testing.T) {
	plan, err := ParsePlan("")
	if err != nil {
		t.Fatalf("ParsePlan failed: %v", err)
	}
	if plan.Title != "" {
		t.Errorf("expected empty title, got %q", plan.Title)
	}
	if plan.Summary != "" {
		t.Errorf("expected empty summary, got %q", plan.Summary)
	}
	if plan.Version != 1 {
		t.Errorf("expected version 1, got %d", plan.Version)
	}
}

func TestParsePlan_NoOpenQuestionsSentinel(t *testing.T) {
	md := `# Plan

## Open Questions
No open questions in this plan.
`
	plan, err := ParsePlan(md)
	if err != nil {
		t.Fatalf("ParsePlan failed: %v", err)
	}
	if len(plan.OpenQuestions) != 0 {
		t.Errorf("expected 0 questions, got %d", len(plan.OpenQuestions))
	}
}

func TestExtractProposedChanges_NoH4UnderH3(t *testing.T) {
	section := `### Empty Category
Some text but no file entries`
	groups := extractProposedChanges(section)
	if len(groups) != 0 {
		t.Errorf("expected 0 groups (no H4 entries), got %d", len(groups))
	}
}

func TestExtractSection_ExtraHeader(t *testing.T) {
	md := "# Plan\n## User Review Required\nReview this\n## Next Section"
	got := extractSection(md, "User Review Required")
	if !strings.Contains(got, "Review this") {
		t.Errorf("expected content, got %q", got)
	}
}
