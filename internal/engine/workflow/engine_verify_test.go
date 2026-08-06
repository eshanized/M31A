package workflow

import (
	"testing"
)

func TestExtractReviewNotes_EmptySection(t *testing.T) {
	t.Parallel()
	notes := extractReviewNotes("")
	if notes != nil {
		t.Errorf("expected nil for empty section, got %v", notes)
	}
}

func TestExtractReviewNotes_WithNotes(t *testing.T) {
	t.Parallel()
	section := `> [!IMPORTANT]
> First note

> [!WARNING]
> Second note
`
	notes := extractReviewNotes(section)
	if len(notes) != 2 {
		t.Fatalf("expected 2 notes, got %d", len(notes))
	}
	if notes[0].Level != "IMPORTANT" {
		t.Errorf("expected IMPORTANT, got %q", notes[0].Level)
	}
	if notes[1].Level != "WARNING" {
		t.Errorf("expected WARNING, got %q", notes[1].Level)
	}
}

func TestFindRelevantFunction_WithDocComment(t *testing.T) {
	t.Parallel()
	lines := []string{
		"package main",
		"",
		"import \"fmt\"",
		"",
		"// Helper does stuff",
		"func Helper() {",
		"    fmt.Println(\"hello\")",
		"}",
	}
	idx := findRelevantFunction(lines, 0, nil)
	if idx < 0 {
		t.Error("expected to find a function")
	}
	// The function should find a func declaration somewhere
	if idx >= len(lines) {
		t.Errorf("index %d out of bounds", idx)
	}
}

func TestFindRelevantFunction_NoFunction(t *testing.T) {
	t.Parallel()
	lines := []string{
		"package main",
		"const x = 1",
	}
	idx := findRelevantFunction(lines, 0, nil)
	if idx != -1 {
		t.Errorf("expected -1, got %d", idx)
	}
}

func TestSectionHeaderRe_Cache(t *testing.T) {
	t.Parallel()
	re1 := sectionHeaderRe("TestSection")
	re2 := sectionHeaderRe("TestSection")
	if re1 != re2 {
		t.Error("expected same regex instance from cache")
	}
}

func TestSubsectionHeaderRe_Cache(t *testing.T) {
	t.Parallel()
	re1 := subsectionHeaderRe("TestSubsection")
	re2 := subsectionHeaderRe("TestSubsection")
	if re1 != re2 {
		t.Error("expected same regex instance from cache")
	}
}

func TestExtractSection_NotFound(t *testing.T) {
	t.Parallel()
	md := `# Title

## Section A
Content A
`
	got := extractSection(md, "Nonexistent")
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}
}

func TestExtractOpenQuestions_EmptyInput(t *testing.T) {
	t.Parallel()
	questions := extractOpenQuestions("")
	if questions != nil {
		t.Errorf("expected nil, got %v", questions)
	}
}

func TestExtractOpenQuestions_NoOpenQuestionsMarker(t *testing.T) {
	t.Parallel()
	questions := extractOpenQuestions("No open questions.")
	if questions != nil {
		t.Errorf("expected nil, got %v", questions)
	}
}

func TestComputeConfidence_AllPass(t *testing.T) {
	t.Parallel()
	result := VerificationResult{
		TaskID:     1,
		FilesExist: true,
		SyntaxOK:   true,
		TestsOK:    true,
		LintOK:     true,
	}
	score := computeConfidence(result, 0, 3, 3)
	if score < 0.9 {
		t.Errorf("expected >= 0.9 for all pass, got %f", score)
	}
}

func TestComputeConfidence_SomeFail(t *testing.T) {
	t.Parallel()
	result := VerificationResult{
		TaskID:     1,
		FilesExist: true,
		SyntaxOK:   false,
		TestsOK:    false,
		LintOK:     true,
		Warnings:   []string{"warning1"},
	}
	score := computeConfidence(result, 1, 2, 3)
	// 1.0 - 0.25 (syntax) - 0.25 (tests) - 0.05 (warning) - 0.1 (heal) = 0.35
	if score < 0.3 || score > 0.5 {
		t.Errorf("expected 0.3-0.5 for some failures, got %f", score)
	}
}

func TestComputeConfidence_AllFail(t *testing.T) {
	t.Parallel()
	result := VerificationResult{
		TaskID:     1,
		FilesExist: false,
		SyntaxOK:   false,
		TestsOK:    false,
		LintOK:     false,
		Warnings:   []string{"w1", "w2", "w3"},
	}
	score := computeConfidence(result, 3, 0, 3)
	if score > 0.1 {
		t.Errorf("expected near 0 for all fail, got %f", score)
	}
}

func TestComputeConfidence_Clamped(t *testing.T) {
	t.Parallel()
	// All pass + bonus = 1.1, should clamp to 1.0
	result := VerificationResult{
		TaskID:     1,
		FilesExist: true,
		SyntaxOK:   true,
		TestsOK:    true,
		LintOK:     true,
	}
	score := computeConfidence(result, 0, 3, 3)
	if score > 1.0 {
		t.Errorf("expected clamped to 1.0, got %f", score)
	}

	// All fail + many warnings + heals = very negative, should clamp to 0.0
	result2 := VerificationResult{
		TaskID:     1,
		FilesExist: false,
		SyntaxOK:   false,
		TestsOK:    false,
		LintOK:     false,
		Warnings:   []string{"w1", "w2", "w3", "w4", "w5", "w6"},
	}
	score2 := computeConfidence(result2, 5, 0, 3)
	if score2 < 0.0 {
		t.Errorf("expected clamped to 0.0, got %f", score2)
	}
}
