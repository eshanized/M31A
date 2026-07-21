package tools

import (
	"fmt"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/tools/fileops"
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
	}

	for _, size := range sizes {
		b.Run(size.name, func(b *testing.B) {
			content := generateContent(size.lines, size.lineLen)
			oldString := "old"
			newString := "new"
			// Insert target strings at various positions
			content = insertTarget(content, "old", 0.25)
			content = insertTarget(content, "old", 0.5)
			content = insertTarget(content, "old", 0.75)

			for _, strategy := range []string{"cascading", "trimmed", "normalized", "anchor", "levenshtein"} {
				b.Run(strategy, func(b *testing.B) {
					b.ReportAllocs()
					for i := 0; i < b.N; i++ {
						var result string
						var err error
						switch strategy {
						case "cascading":
							_, _, _, err = fileops.CascadingReplace(content, oldString, newString, false, 0.8)
						case "trimmed":
							_, err = fileops.TrimmedReplace(content, strings.Split(content, "\n"), oldString, newString)
						case "normalized":
							lines := strings.Split(content, "\n")
							_, err = fileops.NormalizedReplace(content, lines, oldString, newString)
						case "anchor":
							lines := strings.Split(content, "\n")
							_, err = fileops.AnchorReplace(content, lines, oldString, newString)
						case "levenshtein":
							err = fmt.Errorf("not implemented in benchmark")
						}
						if err != nil {
							b.Fatal(err)
						}
						_ = result
					}
				})
			}
		})
	}
}

// BenchmarkCascadingReplace_Strategies benchmarks each strategy individually.
func BenchmarkCascadingReplace_Strategies(b *testing.B) {
	content := generateContent(1000, 100)
	content = insertTarget(content, "target", 0.5)

	b.Run("cascading", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			_, _, _, _ = fileops.CascadingReplace(content, "target", "replacement", false, 0.8)
		}
	})

	b.Run("trimmed", func(b *testing.B) {
		lines := strings.Split(content, "\n")
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			_, _ = fileops.TrimmedReplace(content, lines, "target", "replacement")
		}
	})

	b.Run("normalized", func(b *testing.B) {
		lines := strings.Split(content, "\n")
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			_, _ = fileops.NormalizedReplace(content, lines, "target", "replacement")
		}
	})

	b.Run("anchor", func(b *testing.B) {
		lines := strings.Split(content, "\n")
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			_, _ = fileops.AnchorReplace(content, lines, "target", "replacement")
		}
	})

	b.Run("levenshtein", func(b *testing.B) {
		pairs := generateLevenshteinPairs(1000)
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			for _, pair := range pairs {
				_ = fileops.LevenshteinDistance(pair.a, pair.b)
			}
		}
	})
}

// Helper functions
func generateContent(lines, lineLen int) string {
	var b strings.Builder
	for i := 0; i < lines; i++ {
		b.WriteString(fmt.Sprintf("line %05d: %s\n", i, strings.Repeat("x", lineLen)))
	}
	return b.String()
}

func insertTarget(content, target string, position float64) string {
	lines := strings.Split(content, "\n")
	idx := int(position * float64(len(lines)))
	if idx >= len(lines) {
		idx = len(lines) - 1
	}
	if idx < 0 {
		idx = 0
	}
	lines[idx] = target
	return strings.Join(lines, "\n")
}

type pair struct{ a, b string }

func generateLevenshteinPairs(n int) []pair {
	pairs := make([]pair, n)
	for i := 0; i < n; i++ {
		pairs[i] = pair{
			a: fmt.Sprintf("string %05d", i),
			b: fmt.Sprintf("string %05d", i+1),
		}
	}
	return pairs
}
