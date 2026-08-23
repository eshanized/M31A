package workflow

import (
	"fmt"
	"strings"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

// securityKeywords are terms that indicate security-relevant work.
var securityKeywords = []string{
	"auth", "authentication", "authorization", "crypto", "encrypt", "decrypt",
	"token", "jwt", "oauth", "password", "secret", "credential", "api_key",
	"ssl", "tls", "https", "certificate", "permission", "role", "rbac",
	"middleware", "csrf", "xss", "sqli", "injection", "sanitiz", "cors",
	"session", "cookie", "hash", "salt", "sign", "verify",
}

// granularityGate checks task granularity — always runs regardless of config.
func granularityGate(plan *m31types.PlanDocument) []PlanIssue {
	var issues []PlanIssue

	for _, task := range plan.Tasks {
		if len(task.Files) > 3 {
			issues = append(issues, PlanIssue{
				Severity: "warning",
				Category: "granularity",
				Message:  fmt.Sprintf("Task has %d files (>3) — consider splitting", len(task.Files)),
				TaskID:   task.ID,
			})
		}

		wordCount := len(strings.Fields(task.Description))
		if wordCount > 80 {
			issues = append(issues, PlanIssue{
				Severity: "warning",
				Category: "granularity",
				Message:  fmt.Sprintf("Task description has %d words (>80) — consider simplifying", wordCount),
				TaskID:   task.ID,
			})
		}

		if len(task.Files) == 0 && len(task.AcceptanceCriteria) == 0 {
			issues = append(issues, PlanIssue{
				Severity: "blocker",
				Category: "granularity",
				Message:  "Task has no files and no acceptance criteria — unbounded task",
				TaskID:   task.ID,
			})
		}
	}

	return issues
}

// securityGate checks that security-relevant work has explicit coverage.
// Triggered by config flag or heuristic detection of security keywords.
func securityGate(plan *m31types.PlanDocument) []PlanIssue {
	var issues []PlanIssue

	securityFiles := findSecurityRelevantFiles(plan)
	if len(securityFiles) == 0 {
		return nil
	}

	// Check that security-relevant files have tasks with security-aware acceptance criteria
	hasSecurityTests := false
	hasInputValidation := false

	for _, task := range plan.Tasks {
		descLower := strings.ToLower(task.Description)
		for _, ac := range task.AcceptanceCriteria {
			acLower := strings.ToLower(ac)
			if containsAny(acLower, []string{"auth", "permission", "token", "valid", "sanitiz", "secure"}) {
				hasSecurityTests = true
			}
		}
		if containsAny(descLower, []string{"validat", "sanitiz", "input check", "escape"}) {
			hasInputValidation = true
		}
	}

	if !hasSecurityTests {
		issues = append(issues, PlanIssue{
			Severity: "warning",
			Category: "security",
			Message: fmt.Sprintf("Security-relevant files detected (%s) but no tasks include "+
				"security-specific acceptance criteria (auth, permission, token validation)",
				strings.Join(securityFiles[:min(3, len(securityFiles))], ", ")),
		})
	}

	if !hasInputValidation {
		issues = append(issues, PlanIssue{
			Severity: "warning",
			Category: "security",
			Message:  "Security-relevant work detected but no input validation tasks found",
		})
	}

	return issues
}

// gapAnalysisGate checks for structural coverage gaps in the plan.
func gapAnalysisGate(plan *m31types.PlanDocument, goal string) []PlanIssue {
	var issues []PlanIssue

	// Check: every file in Proposed Changes has a task
	taskFiles := make(map[string]bool)
	for _, task := range plan.Tasks {
		for _, f := range task.Files {
			taskFiles[f] = true
		}
	}

	for _, group := range plan.ProposedChanges {
		for _, change := range group.Changes {
			if !taskFiles[change.File] {
				issues = append(issues, PlanIssue{
					Severity: "warning",
					Category: "gap",
					Message:  fmt.Sprintf("File %s in Proposed Changes has no corresponding task", change.File),
				})
			}
		}
	}

	// Check: goal keywords appear in task descriptions
	goalKeywords := extractGoalKeywords(goal)
	for _, kw := range goalKeywords {
		found := false
		kwLower := strings.ToLower(kw)
		for _, task := range plan.Tasks {
			if strings.Contains(strings.ToLower(task.Description), kwLower) {
				found = true
				break
			}
			for _, ac := range task.AcceptanceCriteria {
				if strings.Contains(strings.ToLower(ac), kwLower) {
					found = true
					break
				}
			}
			if found {
				break
			}
		}
		if !found {
			issues = append(issues, PlanIssue{
				Severity: "warning",
				Category: "gap",
				Message:  fmt.Sprintf("Goal keyword %q not found in any task description or acceptance criterion", kw),
			})
		}
	}

	return issues
}

// requirementsCoverageGate checks that key goal concepts are covered by tasks.
func requirementsCoverageGate(plan *m31types.PlanDocument, goal string) []PlanIssue {
	var issues []PlanIssue

	phrases := extractKeyPhrases(goal)
	for _, phrase := range phrases {
		phraseLower := strings.ToLower(phrase)
		covered := false
		for _, task := range plan.Tasks {
			if strings.Contains(strings.ToLower(task.Description), phraseLower) {
				covered = true
				break
			}
		}
		if !covered {
			issues = append(issues, PlanIssue{
				Severity: "warning",
				Category: "coverage",
				Message:  fmt.Sprintf("Goal concept %q not covered by any task", phrase),
			})
		}
	}

	return issues
}

// hasSecurityKeywords returns true when the plan contains security-relevant content.
func hasSecurityKeywords(plan *m31types.PlanDocument) bool {
	for _, task := range plan.Tasks {
		lower := strings.ToLower(task.Description)
		if containsAny(lower, securityKeywords) {
			return true
		}
		for _, f := range task.Files {
			if containsAny(strings.ToLower(f), securityKeywords) {
				return true
			}
		}
	}
	for _, group := range plan.ProposedChanges {
		for _, change := range group.Changes {
			if containsAny(strings.ToLower(change.File), securityKeywords) {
				return true
			}
		}
	}
	return false
}

// findSecurityRelevantFiles returns file paths that match security keywords.
func findSecurityRelevantFiles(plan *m31types.PlanDocument) []string {
	var files []string
	seen := make(map[string]bool)

	for _, task := range plan.Tasks {
		for _, f := range task.Files {
			if !seen[f] && containsAny(strings.ToLower(f), securityKeywords) {
				files = append(files, f)
				seen[f] = true
			}
		}
	}
	for _, group := range plan.ProposedChanges {
		for _, change := range group.Changes {
			if !seen[change.File] && containsAny(strings.ToLower(change.File), securityKeywords) {
				files = append(files, change.File)
				seen[change.File] = true
			}
		}
	}
	return files
}

// containsAny reports whether s contains any of the given substrings.
func containsAny(s string, substrs []string) bool {
	for _, sub := range substrs {
		if strings.Contains(s, sub) {
			return true
		}
	}
	return false
}

// extractGoalKeywords extracts significant keywords from the goal string.
// Filters out common stop words and very short words.
func extractGoalKeywords(goal string) []string {
	words := strings.Fields(strings.ToLower(goal))
	var keywords []string
	stopWords := map[string]bool{
		"the": true, "a": true, "an": true, "and": true, "or": true, "to": true,
		"of": true, "in": true, "for": true, "with": true, "on": true, "at": true,
		"by": true, "is": true, "it": true, "as": true, "from": true, "that": true,
		"this": true, "be": true, "are": true, "was": true, "will": true, "can": true,
		"but": true, "not": true, "all": true, "if": true, "so": true, "do": true,
		"up": true, "out": true, "use": true, "add": true,
	}
	for _, w := range words {
		cleaned := strings.Trim(w, ".,;:!?\"'()[]{}")
		if len(cleaned) >= 4 && !stopWords[cleaned] {
			keywords = append(keywords, cleaned)
		}
	}
	// Cap at 8 keywords to avoid noise
	if len(keywords) > 8 {
		keywords = keywords[:8]
	}
	return keywords
}

// extractKeyPhrases extracts 2-word phrases from the goal for coverage checking.
func extractKeyPhrases(goal string) []string {
	words := strings.Fields(goal)
	if len(words) <= 2 {
		return []string{goal}
	}

	var phrases []string
	// Extract noun-verb and adjective-noun pairs
	for i := 0; i < len(words)-1; i++ {
		phrase := words[i] + " " + words[i+1]
		lower := strings.ToLower(phrase)
		// Skip phrases that are mostly stop words, unless they contain domain terms
		isStopPhrase := containsAny(lower, []string{"the ", "a ", "an ", "and ", "or ", "to "})
		isDomainTerm := strings.Contains(lower, "api") || strings.Contains(lower, "auth") ||
			strings.Contains(lower, "test") || strings.Contains(lower, "database")
		if !isStopPhrase || isDomainTerm {
			phrases = append(phrases, phrase)
		}
	}
	// Cap at 6 phrases
	if len(phrases) > 6 {
		phrases = phrases[:6]
	}
	return phrases
}
