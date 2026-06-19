package workflow

import (
	"testing"
)

func TestIsResearchWorthy(t *testing.T) {
	// Create a minimal engine with a temp workDir
	dir := t.TempDir()
	e := &Engine{workDir: dir}

	tests := []struct {
		name string
		goal string
		want bool
	}{
		{"complex multi-step", "build authentication system with OAuth and integrate payment pipeline", true},
		{"single complex indicator", "implement a landing page", false},
		{"two complex indicators", "implement and integrate microservice pipeline", true},
		{"trivial goal", "add a button", false},
		{"moderate goal", "update the login page", false},
		{"long complex goal", "design and architect a distributed caching layer with invalidation and integrate it into the existing API gateway", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := e.isResearchWorthy(tt.goal)
			if got != tt.want {
				t.Errorf("isResearchWorthy(%q) = %v, want %v", tt.goal, got, tt.want)
			}
		})
	}
}
