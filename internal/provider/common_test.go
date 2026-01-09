package provider

import (
	"strings"
	"testing"
)

func TestMaskAPIKeys(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "standard OpenAI sk- key",
			input:    "sk-abc123def456ghi7",
			expected: "sk-a****ghi7",
		},
		{
			name:     "key- prefix",
			input:    "key-abc123def456ghi7",
			expected: "key-****ghi7",
		},
		{
			name:     "api_key= assignment",
			input:    "api_key=abc123def456ghi7",
			expected: "api_****ghi7",
		},
		{
			name:     "api-key= assignment (dash variant)",
			input:    "api-key=abc123def456ghi7",
			expected: "api-****ghi7",
		},
		{
			name:     "Bearer JWT token does not match",
			input:    "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
			expected: "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
		},
		{
			name:     "empty string",
			input:    "",
			expected: "",
		},
		{
			name:     "no keys present",
			input:    "hello world no api keys here",
			expected: "hello world no api keys here",
		},
		{
			name:     "multiple keys in one string",
			input:    "sk-abc123def456ghi7 and key-xyz789abc012def3",
			expected: "sk-a****ghi7 and key-****def3",
		},
		{
			name:     "short key below minimum length does not match",
			input:    "sk-abcd1234",
			expected: "sk-abcd1234",
		},
		{
			name:     "key in error message context",
			input:    "401 Unauthorized: invalid sk-abc123def456ghi7",
			expected: "401 Unauthorized: invalid sk-a****ghi7",
		},
		{
			name:     "case insensitive SK- prefix",
			input:    "SK-ABC123DEF456GHI7",
			expected: "SK-A****GHI7",
		},
		{
			name:     "api_key with colon and space",
			input:    "api_key: abc123def456ghi7",
			expected: "api_****ghi7",
		},
		{
			name:     "api_key with quotes",
			input:    `api_key="abc123def456ghi7"`,
			expected: `api_****ghi7"`,
		},
		{
			name:     "long key in JSON body",
			input:    `{"error":"Invalid api_key=sk-abc123def456ghi7 was rejected"}`,
			expected: `{"error":"Invalid api_key=sk-a****ghi7 was rejected"}`,
		},
		{
			name:     "three keys in sequence",
			input:    "sk-aaa111bbb222ccc3 key-zzz999yyy888xxx7 api_key=ppp555qqq666rrr8",
			expected: "sk-a****ccc3 key-****xxx7 api_****rrr8",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := maskAPIKeys(tt.input)
			if got != tt.expected {
				t.Errorf("maskAPIKeys(%q) = %q, want %q", tt.input, got, tt.expected)
			}
		})
	}
}

func TestMaskAPIKeys_Idempotent(t *testing.T) {
	input := "error: sk-abc123def456ghi7 failed"
	once := maskAPIKeys(input)
	twice := maskAPIKeys(once)
	if once != twice {
		t.Errorf("maskAPIKeys is not idempotent: first=%q, second=%q", once, twice)
	}
}

func TestMaskAPIKeys_PreservesNonKeyContent(t *testing.T) {
	input := "GET /v1/chat/completions returned 401 for model sk-abc123def456ghi7"
	got := maskAPIKeys(input)

	if strings.Contains(got, "sk-abc123def456ghi7") {
		t.Errorf("key was not redacted: %q", got)
	}
	if !strings.HasPrefix(got, "GET /v1/chat/completions returned 401 for model ") {
		t.Errorf("non-key content was altered: %q", got)
	}
}

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
