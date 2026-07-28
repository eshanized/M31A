package tools

import (
	"testing"
)

func TestExtractFromParams_Metrics(t *testing.T) {
	tests := []struct {
		name   string
		params map[string]any
		want   string
	}{
		{
			name:   "metrics with mode",
			params: map[string]any{"mode": "summary"},
			want:   "metrics (summary)",
		},
		{
			name:   "metrics without mode",
			params: map[string]any{},
			want:   "show metrics",
		},
		{
			name:   "metrics with empty mode",
			params: map[string]any{"mode": ""},
			want:   "show metrics",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := extractFromParams("Metrics", tt.params)
			if got != tt.want {
				t.Errorf("extractFromParams(\"Metrics\", %v) = %q, want %q", tt.params, got, tt.want)
			}
		})
	}
}

func TestExtractFromParams_Git(t *testing.T) {
	tests := []struct {
		name   string
		params map[string]any
		want   string
	}{
		{
			name:   "git with action",
			params: map[string]any{"action": "status"},
			want:   "git status",
		},
		{
			name:   "git without action",
			params: map[string]any{},
			want:   "git operation",
		},
		{
			name:   "git with empty action",
			params: map[string]any{"action": ""},
			want:   "git operation",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := extractFromParams("Git", tt.params)
			if got != tt.want {
				t.Errorf("extractFromParams(\"Git\", %v) = %q, want %q", tt.params, got, tt.want)
			}
		})
	}
}
