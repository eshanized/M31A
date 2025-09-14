package provider

import (
	"testing"
)

func TestIsContextExceeded_OperatorPrecedence(t *testing.T) {
	tests := []struct {
		name       string
		statusCode int
		body       string
		expected   bool
	}{
		{"context_length_exceeded", 400, `{"error":"context_length_exceeded"}`, true},
		{"maximum context length", 400, `{"error":"maximum context length is 128000"}`, true},
		{"request too large", 400, `{"error":"request too large for model"}`, true},
		{"context window exceeded", 400, `{"error":"context window exceeded"}`, true},
		{"context_length with exceed keyword", 400, `{"error":"context_length must not exceed limit"}`, true},
		{"context_length without exceed keyword", 400, `{"error":"context_length is required"}`, false},
		{"non-400 with context keyword", 500, `{"error":"context_length_exceeded"}`, false},
		{"400 without context keywords", 400, `{"error":"invalid parameter"}`, false},
		{"case insensitive", 400, `{"error":"CONTEXT_LENGTH_EXCEEDED"}`, true},
		{"mixed case exceed keyword", 400, `{"error":"context_length must not Exceed limit"}`, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := IsContextExceeded(tt.statusCode, tt.body)
			if got != tt.expected {
				t.Errorf("IsContextExceeded(%d, %q) = %v, want %v", tt.statusCode, tt.body, got, tt.expected)
			}
		})
	}
}
