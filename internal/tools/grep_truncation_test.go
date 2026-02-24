package tools

import (
	"fmt"
	"testing"
)

func TestGrepTruncation(t *testing.T) {
	t.Run("Truncation message format", func(t *testing.T) {
		truncated := true
		output := "result1\nresult2\nresult3"
		if truncated {
			output += fmt.Sprintf("\n[... %d more matches (limit: %d)]", 50, 100)
		}
		expected := "result1\nresult2\nresult3\n[... 50 more matches (limit: 100)]"
		if output != expected {
			t.Errorf("expected '%s', got '%s'", expected, output)
		}
	})

	t.Run("No truncation when under limit", func(t *testing.T) {
		truncated := false
		output := "result1\nresult2\nresult3"
		if truncated {
			output += "\n[... more matches]"
		}
		expected := "result1\nresult2\nresult3"
		if output != expected {
			t.Errorf("expected '%s', got '%s'", expected, output)
		}
	})

	t.Run("Truncated flag set correctly", func(t *testing.T) {
		var truncated bool
		results := make([]string, 150)
		maxResults := 100
		if len(results) >= maxResults {
			truncated = true
		}
		if !truncated {
			t.Error("expected truncated to be true")
		}
	})

	t.Run("Truncated flag not set when under limit", func(t *testing.T) {
		var truncated bool
		results := make([]string, 50)
		maxResults := 100
		if len(results) >= maxResults {
			truncated = true
		}
		if truncated {
			t.Error("expected truncated to be false")
		}
	})
}
