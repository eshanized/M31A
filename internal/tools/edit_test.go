package tools

import (
	"testing"

	"github.com/eshanized/M31A/internal/tools/fileops"
)

func TestLevenshteinSimilarity_EmptyStrings(t *testing.T) {
	t.Parallel()
	if d := fileops.LevenshteinSimilarity("", ""); d != 1.0 {
		t.Errorf("similarity('', '') = %f, want 1.0", d)
	}
}

func TestLevenshteinSimilarity_OneEmpty_Edit(t *testing.T) {
	t.Parallel()
	if d := fileops.LevenshteinSimilarity("abc", ""); d != 0.0 {
		t.Errorf("similarity('abc', '') = %f, want 0", d)
	}
	if d := fileops.LevenshteinSimilarity("", "abc"); d != 0.0 {
		t.Errorf("similarity('', 'abc') = %f, want 0", d)
	}
}

func TestLevenshteinSimilarity_Identical(t *testing.T) {
	t.Parallel()
	if d := fileops.LevenshteinSimilarity("hello", "hello"); d != 1.0 {
		t.Errorf("similarity('hello', 'hello') = %f, want 1", d)
	}
}

func TestLevenshteinSimilarity_SingleCharDiff(t *testing.T) {
	t.Parallel()
	if d := fileops.LevenshteinSimilarity("cat", "bat"); d >= 1.0 {
		t.Errorf("similarity('cat', 'bat') = %f, want < 1", d)
	}
	if d := fileops.LevenshteinSimilarity("cat", "bat"); d <= 0.0 {
		t.Errorf("similarity('cat', 'bat') = %f, want > 0", d)
	}
}

func TestLevenshteinSimilarity_Different(t *testing.T) {
	t.Parallel()
	if d := fileops.LevenshteinSimilarity("abc", "xyz"); d != 0.0 {
		t.Errorf("similarity('abc', 'xyz') = %f, want 0", d)
	}
}

func TestLevenshteinSimilarity_KnownValues(t *testing.T) {
	t.Parallel()
	// "kitten" -> "sitting" has distance 3, max len 7, similarity = 1 - 3/7 = 4/7 ≈ 0.57
	if d := fileops.LevenshteinSimilarity("kitten", "sitting"); d > 0.6 || d < 0.5 {
		t.Errorf("similarity('kitten', 'sitting') = %f, want ~0.57", d)
	}
}
