package workflow

import (
	"testing"
)

func TestIsYesNoQuestion(t *testing.T) {
	tests := []struct {
		question string
		want     bool
	}{
		{"Should we use React? — I suggest React with Next.js.", true},
		{"Do we need authentication? — JWT tokens.", true},
		{"Is the API public? — Yes.", true},
		{"What framework should be used? — React.", false},
		{"How should the authentication flow work? — JWT.", false},
		{"Can we use a CDN? — Cloudflare.", true},
		{"What is the expected data volume? — 10K users.", false},
		{"Would it make sense to cache? — Redis.", true},
	}

	for _, tt := range tests {
		t.Run(tt.question, func(t *testing.T) {
			got := isYesNoQuestion(tt.question)
			if got != tt.want {
				t.Errorf("isYesNoQuestion(%q) = %v, want %v", tt.question, got, tt.want)
			}
		})
	}
}

func TestIsVagueQuestion(t *testing.T) {
	tests := []struct {
		question string
		want     bool
	}{
		{"Any preferences? — No.", true},
		{"What about the design? — Dark theme.", true},
		{"What specific database schema should be used for user authentication? — PostgreSQL with bcrypt.", false},
		{"How should the API endpoint handle pagination? — Cursor-based.", false},
		{"Style? — Minimal.", true},
		{"What component framework and state management approach? — React + Redux.", false},
	}

	for _, tt := range tests {
		t.Run(tt.question, func(t *testing.T) {
			got := isVagueQuestion(tt.question)
			if got != tt.want {
				t.Errorf("isVagueQuestion(%q) = %v, want %v", tt.question, got, tt.want)
			}
		})
	}
}

func TestWordOverlap(t *testing.T) {
	tests := []struct {
		a, b string
		min  float64
		max  float64
	}{
		{"What framework for the frontend", "What framework for the backend", 0.2, 0.5},
		{"How should auth work", "What color palette", 0.0, 0.1},
		{"Database schema design", "Database schema design", 0.9, 1.1},
		{"", "something", 0.0, 0.0},
	}

	for _, tt := range tests {
		overlap := wordOverlap(tt.a, tt.b)
		if overlap < tt.min || overlap > tt.max {
			t.Errorf("wordOverlap(%q, %q) = %f, want [%f, %f]",
				tt.a, tt.b, overlap, tt.min, tt.max)
		}
	}
}

func TestCheckQuestionQuality(t *testing.T) {
	questions := []string{
		"What framework should be used for the API? — Express.js with TypeScript.",
		"How should the database schema handle user sessions? — Redis with JWT tokens.",
		"What framework should be used for the API? — Express.js with TypeScript.", // duplicate of #1
	}

	issues := checkQuestionQuality(questions)

	// Should find a duplicate between questions 1 and 3
	hasDuplicate := false
	for _, issue := range issues {
		if issue.Category == "duplicate" && issue.Severity == "blocker" {
			hasDuplicate = true
		}
	}
	if !hasDuplicate {
		t.Error("expected duplicate detection between questions 1 and 3")
	}
}

func TestCheckAnswerCompleteness(t *testing.T) {
	questions := []string{
		"What framework?",
		"How should auth work?",
		"What database?",
	}
	answers := map[int]string{
		0: "Use Next.js with TypeScript",
		1: "", // skipped
		2: "PostgreSQL with Prisma ORM",
	}
	goal := "build authentication API with database"

	result := checkAnswerCompleteness(questions, answers, goal)

	if result.Answered != 2 {
		t.Errorf("answered = %d, want 2", result.Answered)
	}
	if result.Skipped != 1 {
		t.Errorf("skipped = %d, want 1", result.Skipped)
	}
	if result.Score >= 100 {
		t.Errorf("score should be penalized for skipped answer, got %d", result.Score)
	}
}

func TestWordSet(t *testing.T) {
	set := wordSet("the quick brown fox jumps over the lazy dog")
	// Stop words should be filtered, short words too
	if set["the"] {
		t.Error("'the' should be filtered as stop word")
	}
	if !set["quick"] {
		t.Error("'quick' should be in the set")
	}
	if !set["brown"] {
		t.Error("'brown' should be in the set")
	}
}
