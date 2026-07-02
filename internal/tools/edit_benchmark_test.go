package tools

import (
	"fmt"
	"strings"
	"testing"
)

// BenchmarkCascadingReplace benchmarks the cascading replace algorithm
// across all 5 strategies with varying content sizes.
func BenchmarkCascadingReplace(b *testing.B) {
	sizes := []struct {
		name    string
		lines   int
		lineLen int
	}{
		{"tiny_10x50", 10, 50},
		{"small_100x80", 100, 80},
		{"medium_1000x100", 1000, 100},
		{"large_5000x120", 5000, 120},
		{"xlarge_10000x150", 10000, 150},
	}

	for _, size := range sizes {
		b.Run(size.name, func(b *testing.B) {
			content := generateContent(size.lines, size.lineLen)
			oldString := extractMiddle(content, 5)
			newString := "replacement text for benchmarking"

			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				cascadingReplace(content, oldString, newString, false, 0.8)
			}
		})
	}
}

// BenchmarkCascadingReplace_Strategies benchmarks each strategy individually.
func BenchmarkCascadingReplace_Strategies(b *testing.B) {
	content := generateContent(1000, 100)
	lines := strings.Split(content, "\n")

	b.Run("exact_match", func(b *testing.B) {
		oldString := lines[500]
		newString := "replaced_line"
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			strings.Replace(content, oldString, newString, 1)
		}
	})

	b.Run("trimmed", func(b *testing.B) {
		// Add leading/trailing spaces to old string to force trimmed match
		oldString := "  " + lines[500] + "  "
		newString := "replaced_line"
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			trimmedReplace(content, lines, oldString, newString)
		}
	})

	b.Run("normalized", func(b *testing.B) {
		// Use double spaces to force normalization
		oldString := strings.ReplaceAll(lines[500], " ", "  ")
		newString := "replaced_line"
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			normalizedReplace(content, lines, oldString, newString)
		}
	})

	b.Run("anchor", func(b *testing.B) {
		// Use first and last lines of a 10-line block
		startIdx := 495
		endIdx := 504
		oldLines := make([]string, 0, 10)
		for i := startIdx; i <= endIdx; i++ {
			oldLines = append(oldLines, lines[i])
		}
		oldString := strings.Join(oldLines, "\n")
		newString := "replaced_block"
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			anchorReplace(content, lines, oldString, newString)
		}
	})
}

// BenchmarkLevenshteinDistance benchmarks the Levenshtein distance algorithm.
func BenchmarkLevenshteinDistance(b *testing.B) {
	pairs := []struct {
		name string
		a, b string
	}{
		{"empty", "", ""},
		{"one_empty", "hello", ""},
		{"short_same", "abc", "abc"},
		{"short_diff", "abc", "xyz"},
		{"medium_similar", "algorithm", "altruistic"},
		{"medium_different", "hello", "world"},
		{"long_similar", "The quick brown fox jumps over the lazy dog", "The quick brown fox jumps over the lazy cat"},
		{"long_different", "The quick brown fox jumps over the lazy dog", "Lorem ipsum dolor sit amet consectetur"},
	}

	for _, pair := range pairs {
		b.Run(pair.name, func(b *testing.B) {
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				levenshteinDistance(pair.a, pair.b)
			}
		})
	}
}

// BenchmarkLevenshteinBuf benchmarks the buffer-reusing Levenshtein variant.
func BenchmarkLevenshteinBuf(b *testing.B) {
	pairs := []struct {
		name string
		a, b string
	}{
		{"short_similar", "abc", "abd"},
		{"medium_similar", "algorithm", "altruistic"},
		{"long_similar", "The quick brown fox jumps over the lazy dog", "The quick brown fox jumps over the lazy cat"},
	}

	for _, pair := range pairs {
		b.Run(pair.name, func(b *testing.B) {
			maxLen := len(pair.a)
			if len(pair.b) > maxLen {
				maxLen = len(pair.b)
			}
			prev := make([]int, maxLen+1)
			curr := make([]int, maxLen+1)
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				levenshteinBuf(pair.a, pair.b, prev, curr)
			}
		})
	}
}

// BenchmarkDiffSummary benchmarks the LCS-based diff generation.
func BenchmarkDiffSummary(b *testing.B) {
	sizes := []struct {
		name    string
		lines   int
		changes int
	}{
		{"small_10_2changes", 10, 2},
		{"medium_100_10changes", 100, 10},
		{"large_1000_50changes", 1000, 50},
		{"xlarge_5000_200changes", 5000, 200},
	}

	for _, size := range sizes {
		b.Run(size.name, func(b *testing.B) {
			oldContent := generateContent(size.lines, 80)
			newContent := applyChanges(oldContent, size.changes)
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				generateDiffSummary("test.go", oldContent, newContent)
			}
		})
	}
}

// BenchmarkReplaceByLineRange benchmarks line-range replacement.
func BenchmarkReplaceByLineRange(b *testing.B) {
	sizes := []struct {
		name  string
		lines int
	}{
		{"small_100", 100},
		{"medium_1000", 1000},
		{"large_5000", 5000},
		{"xlarge_10000", 10000},
	}

	for _, size := range sizes {
		b.Run(size.name, func(b *testing.B) {
			content := generateContent(size.lines, 80)
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				replaceByLineRange(content, 100, 200, "replacement_content")
			}
		})
	}
}

// BenchmarkFormatNumber benchmarks the number formatting function.
func BenchmarkFormatNumber(b *testing.B) {
	numbers := []int{0, 1, 999, 1000, 999999, 1000000, 1234567890}
	for _, n := range numbers {
		b.Run(fmt.Sprintf("n=%d", n), func(b *testing.B) {
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				formatNumber(n)
			}
		})
	}
}

// BenchmarkLeadingWhitespace benchmarks the leading whitespace extraction.
func BenchmarkLeadingWhitespace(b *testing.B) {
	strings_to_test := []struct {
		name  string
		input string
	}{
		{"none", "hello"},
		{"spaces_4", "    hello"},
		{"tabs_2", "\t\thello"},
		{"mixed", "\t  \thello"},
		{"long", strings.Repeat(" ", 100) + "hello"},
	}

	for _, tc := range strings_to_test {
		b.Run(tc.name, func(b *testing.B) {
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				leadingWhitespace(tc.input)
			}
		})
	}
}

// Helper functions for benchmarks

// generateContent creates a string with the specified number of lines
// and approximate line length.
func generateContent(lines, lineLen int) string {
	var sb strings.Builder
	words := []string{"func", "return", "if", "else", "for", "range", "var", "type", "struct", "interface", "package", "import", "error", "nil", "true", "false", "const", "map", "string", "int"}

	for i := 0; i < lines; i++ {
		// Generate a line with mixed content
		lineLenRemaining := lineLen
		for lineLenRemaining > 0 {
			word := words[i%len(words)]
			if len(word) > lineLenRemaining {
				break
			}
			sb.WriteString(word)
			lineLenRemaining -= len(word)
			if lineLenRemaining > 0 {
				sb.WriteByte(' ')
				lineLenRemaining--
			}
		}
		if i < lines-1 {
			sb.WriteByte('\n')
		}
	}
	return sb.String()
}

// extractMiddle extracts a multi-line block from the middle of content.
func extractMiddle(content string, numLines int) string {
	lines := strings.Split(content, "\n")
	mid := len(lines) / 2
	start := mid - numLines/2
	if start < 0 {
		start = 0
	}
	end := start + numLines
	if end > len(lines) {
		end = len(lines)
	}
	return strings.Join(lines[start:end], "\n")
}

// applyChanges applies random changes to content for diff benchmarks.
func applyChanges(content string, numChanges int) string {
	lines := strings.Split(content, "\n")
	step := len(lines) / (numChanges + 1)
	for i := 1; i <= numChanges && i*step < len(lines); i++ {
		lines[i*step] = "MODIFIED_LINE_FOR_BENCHMARK"
	}
	return strings.Join(lines, "\n")
}
