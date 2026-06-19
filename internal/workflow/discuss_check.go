package workflow

import (
	"context"
	"fmt"
	"strings"

	m31types "github.com/eshanized/M31A/internal/types"
)

// DiscussIssue represents a quality issue found in generated questions.
type DiscussIssue struct {
	Severity      string // "blocker" or "warning"
	Category      string // yes_no, vague, duplicate, redundant
	Message       string
	QuestionIndex int
}

// DiscussCompleteness holds the result of an answer completeness check.
type DiscussCompleteness struct {
	Score        int // 0-100
	Answered     int
	Skipped      int
	Total        int
	MissingAreas []string
}

// checkQuestionQuality validates parsed questions for quality issues.
func checkQuestionQuality(questions []string) []DiscussIssue {
	var issues []DiscussIssue

	for i, q := range questions {
		// Yes/no detection
		if isYesNoQuestion(q) {
			issues = append(issues, DiscussIssue{
				Severity:      "warning",
				Category:      "yes_no",
				Message:       fmt.Sprintf("Question %d can likely be answered with yes/no — consider rephrasing as 'how' or 'what'", i+1),
				QuestionIndex: i,
			})
		}

		// Vague detection
		if isVagueQuestion(q) {
			issues = append(issues, DiscussIssue{
				Severity:      "warning",
				Category:      "vague",
				Message:       fmt.Sprintf("Question %d is vague — consider adding more specifics", i+1),
				QuestionIndex: i,
			})
		}

		// Duplicate detection (against all other questions)
		for j := i + 1; j < len(questions); j++ {
			overlap := wordOverlap(q, questions[j])
			if overlap > 0.7 {
				issues = append(issues, DiscussIssue{
					Severity:      "blocker",
					Category:      "duplicate",
					Message:       fmt.Sprintf("Questions %d and %d have %.0f%% word overlap — likely duplicate", i+1, j+1, overlap*100),
					QuestionIndex: j,
				})
			}
		}
	}

	return issues
}

// isYesNoQuestion detects questions that can be answered with just yes or no.
func isYesNoQuestion(q string) bool {
	lower := strings.ToLower(strings.TrimSpace(q))
	// Strip trailing suggestion after em-dash
	if idx := strings.Index(lower, "—"); idx > 0 {
		lower = strings.TrimSpace(lower[:idx])
	}
	if idx := strings.Index(lower, " - "); idx > 0 {
		lower = strings.TrimSpace(lower[:idx])
	}

	// Remove question mark
	lower = strings.TrimRight(lower, "?")

	yesNoPrefixes := []string{
		"should ", "do ", "does ", "did ", "is ", "are ", "was ", "were ",
		"can ", "could ", "will ", "would ", "have ", "has ", "had ",
		"do you ", "should we ", "is it ", "are there ", "do we ",
		"would it ", "can we ", "will we ",
	}

	for _, prefix := range yesNoPrefixes {
		if strings.HasPrefix(lower, prefix) {
			return true
		}
	}
	return false
}

// isVagueQuestion detects questions that lack specificity.
func isVagueQuestion(q string) bool {
	// Strip trailing suggestion
	mainPart := q
	if idx := strings.Index(q, "—"); idx > 0 {
		mainPart = strings.TrimSpace(q[:idx])
	}
	if idx := strings.Index(q, " - "); idx > 0 {
		mainPart = strings.TrimSpace(q[:idx])
	}

	words := strings.Fields(mainPart)
	// Very short questions are likely vague
	if len(words) < 5 {
		return true
	}

	// Questions without any concrete technical terms
	technicalTerms := []string{
		"api", "database", "auth", "cache", "queue", "framework", "library",
		"component", "endpoint", "model", "schema", "test", "deploy",
		"config", "error", "format", "protocol", "pattern", "structure",
		"route", "middleware", "state", "storage", "file", "user",
	}

	lower := strings.ToLower(mainPart)
	hasTerm := false
	for _, term := range technicalTerms {
		if strings.Contains(lower, term) {
			hasTerm = true
			break
		}
	}

	return !hasTerm && len(words) < 8
}

// wordOverlap calculates the Jaccard similarity between two strings.
func wordOverlap(a, b string) float64 {
	wordsA := wordSet(strings.ToLower(a))
	wordsB := wordSet(strings.ToLower(b))

	if len(wordsA) == 0 || len(wordsB) == 0 {
		return 0
	}

	intersection := 0
	for w := range wordsA {
		if wordsB[w] {
			intersection++
		}
	}

	union := len(wordsA) + len(wordsB) - intersection
	if union == 0 {
		return 0
	}

	return float64(intersection) / float64(union)
}

// wordSet creates a set of words from a string, filtering stop words.
func wordSet(s string) map[string]bool {
	stopWords := map[string]bool{
		"the": true, "a": true, "an": true, "and": true, "or": true, "to": true,
		"of": true, "in": true, "for": true, "with": true, "on": true, "at": true,
		"by": true, "is": true, "it": true, "as": true, "from": true, "that": true,
		"this": true, "be": true, "are": true, "was": true, "what": true, "how": true,
		"should": true, "do": true, "we": true, "i": true, "you": true, "?": true,
	}
	words := strings.Fields(s)
	set := make(map[string]bool)
	for _, w := range words {
		cleaned := strings.Trim(w, ".,;:!?\"'()[]{}")
		if len(cleaned) >= 3 && !stopWords[cleaned] {
			set[cleaned] = true
		}
	}
	return set
}

// checkAnswerCompleteness evaluates how well the collected answers cover the goal.
func checkAnswerCompleteness(questions []string, answers map[int]string, goal string) DiscussCompleteness {
	result := DiscussCompleteness{
		Total: len(questions),
	}

	// Count answered vs skipped
	for i := range questions {
		if ans, ok := answers[i]; ok && strings.TrimSpace(ans) != "" {
			result.Answered++
		} else {
			result.Skipped++
		}
	}

	// Calculate base score from answer ratio
	if result.Total > 0 {
		result.Score = (result.Answered * 100) / result.Total
	}

	// Check goal keyword coverage in answers
	goalKeywords := extractGoalKeywords(goal)
	var allAnswerText strings.Builder
	for _, ans := range answers {
		allAnswerText.WriteString(strings.ToLower(ans))
		allAnswerText.WriteString(" ")
	}
	answerLower := allAnswerText.String()

	for _, kw := range goalKeywords {
		if !strings.Contains(answerLower, kw) {
			result.MissingAreas = append(result.MissingAreas, kw)
		}
	}

	// Penalize score for missing areas
	if len(result.MissingAreas) > 0 && result.Score > 0 {
		penalty := len(result.MissingAreas) * 10
		result.Score -= penalty
		if result.Score < 0 {
			result.Score = 0
		}
	}

	return result
}

// generateFollowUps creates targeted follow-up questions when answers are incomplete.
func (e *Engine) generateFollowUps(ctx context.Context, goal string, questions []string, answers map[int]string) ([]string, error) {
	e.emit(IntermediateProgressMsg{
		Phase:   "discuss",
		Message: "Generating follow-up questions...",
	})

	messages := e.buildFollowUpContext(goal, questions, answers)

	content, err := e.streamLLM(ctx, messages, false)
	if err != nil {
		return nil, fmt.Errorf("follow-up LLM call failed: %w", err)
	}

	followUps := parseQuestions(content)
	// Cap at 2 follow-ups
	if len(followUps) > 2 {
		followUps = followUps[:2]
	}

	return followUps, nil
}

// buildFollowUpContext assembles messages for follow-up question generation.
func (e *Engine) buildFollowUpContext(goal string, questions []string, answers map[int]string) []m31types.Message {
	var messages []m31types.Message

	systemPrompt := e.buildSystemPrompt(e.prompts.Discuss, e.prompts.DiscussFollowup)
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	var userCtx strings.Builder
	userCtx.WriteString(fmt.Sprintf("## Goal\n%s\n\n", goal))

	userCtx.WriteString("## Original Questions and Answers\n\n")
	for i, q := range questions {
		ans := "(skipped)"
		if a, ok := answers[i]; ok && strings.TrimSpace(a) != "" {
			ans = a
		}
		userCtx.WriteString(fmt.Sprintf("%d. **Q:** %s\n   **A:** %s\n\n", i+1, q, ans))
	}

	completeness := checkAnswerCompleteness(questions, answers, goal)
	if len(completeness.MissingAreas) > 0 {
		userCtx.WriteString("## Areas Not Yet Covered\n")
		for _, area := range completeness.MissingAreas {
			userCtx.WriteString(fmt.Sprintf("- %s\n", area))
		}
		userCtx.WriteString("\n")
	}

	userCtx.WriteString("Generate 1-2 targeted follow-up questions to fill the gaps above.")

	messages = append(messages, m31types.Message{Role: "user", Content: userCtx.String()})
	return messages
}

// shouldGenerateFollowUps returns true when answer completeness is low enough
// to warrant follow-up questions.
func (e *Engine) shouldGenerateFollowUps(completeness DiscussCompleteness) bool {
	followUpEnabled := e.cfg != nil && e.cfg.Features.DiscussFollowUps
	if !followUpEnabled {
		return false
	}
	// Trigger when score < 60% or more than half the answers are skipped
	return completeness.Score < 60 || completeness.Skipped > completeness.Total/2
}
