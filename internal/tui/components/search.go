package components

import (
	"strings"
	"unicode"

	"github.com/charmbracelet/lipgloss"
)

// RenderSearchBar renders a search input with a label.
func RenderSearchBar(label, query string, width int, brandColor, textColor lipgloss.Color) string {
	labelStyle := lipgloss.NewStyle().
		Foreground(brandColor).
		Bold(true)
	prompt := labelStyle.Render(label)

	input := lipgloss.NewStyle().
		Foreground(textColor).
		Render(query)

	cursor := lipgloss.NewStyle().
		Foreground(brandColor).
		Render("│")

	searchRow := lipgloss.JoinHorizontal(lipgloss.Center, prompt, input, cursor)

	style := lipgloss.NewStyle().
		Foreground(textColor).
		Padding(0, 1).
		Width(width - 2)

	return style.Render(searchRow)
}

// FuzzyHighlight highlights matched characters from a query in a text string.
// Matched characters are rendered in bold with the brand color.
// Uses case-insensitive sequential character matching.
func FuzzyHighlight(query, text string, brandColor lipgloss.Color) string {
	if query == "" || text == "" {
		return text
	}

	queryLower := strings.ToLower(query)
	textLower := strings.ToLower(text)

	matched := make(map[int]bool, len(queryLower))
	qi := 0
	for ti := 0; ti < len(text) && qi < len(queryLower); ti++ {
		if ti < len(textLower) && textLower[ti] == queryLower[qi] {
			matched[ti] = true
			qi++
		}
	}

	// If not all query characters matched, return plain text
	if qi < len(queryLower) {
		return text
	}

	highlightStyle := lipgloss.NewStyle().Foreground(brandColor).Bold(true)

	var b strings.Builder
	runes := []rune(text)
	for i, r := range runes {
		if matched[i] {
			b.WriteString(highlightStyle.Render(string(r)))
		} else {
			b.WriteRune(r)
		}
	}

	return b.String()
}

// FuzzyMatch returns true if all characters in query appear in text
// in sequential order (case-insensitive).
func FuzzyMatch(query, text string) bool {
	if query == "" {
		return true
	}

	queryLower := strings.ToLower(query)
	textLower := strings.ToLower(text)

	qi := 0
	for _, r := range textLower {
		if qi < len(queryLower) && byte(r) == queryLower[qi] {
			qi++
		}
	}
	return qi == len(queryLower)
}

// FuzzyScore returns a match score (higher = better match).
// Returns -1 if no match. Score is based on:
//   - consecutive matches bonus
//   - start-of-word matches bonus
func FuzzyScore(query, text string) int {
	if query == "" {
		return 0
	}

	queryLower := strings.ToLower(query)
	textRunes := []rune(strings.ToLower(text))

	score := 0
	qi := 0
	prevMatched := false
	for ti, r := range textRunes {
		if qi < len(queryLower) && byte(r) == queryLower[qi] {
			score++
			// Consecutive match bonus
			if prevMatched {
				score += 2
			}
			// Start-of-word bonus
			if ti == 0 || (ti > 0 && (unicode.IsSpace(textRunes[ti-1]) || textRunes[ti-1] == '_' || textRunes[ti-1] == '-' || textRunes[ti-1] == '/')) {
				score += 3
			}
			prevMatched = true
			qi++
		} else {
			prevMatched = false
		}
	}

	if qi < len(queryLower) {
		return -1
	}
	return score
}
