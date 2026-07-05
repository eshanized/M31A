package tools

import (
	"encoding/json"
	"testing"
)

func TestParameterSchema_AllTools(t *testing.T) {
	t.Parallel()

	// All tools that implement ParameterSchema
	schemas := []struct {
		name   string
		schema string
	}{
		{"Bash", NewBash(".", 1800).ParameterSchema()},
		{"FileRead", NewFileRead(".").ParameterSchema()},
		{"FileWrite", NewFileWrite(".", ".").ParameterSchema()},
		{"Glob", NewGlob(".").ParameterSchema()},
		{"Grep", NewGrep(".").ParameterSchema()},
		{"Edit", NewEdit(".", ".").ParameterSchema()},
		{"WebFetch", NewWebFetch(".", false, 3, 100).ParameterSchema()},
		{"TodoWrite", NewTodoWrite(".", "test").ParameterSchema()},
	}

	for _, s := range schemas {
		t.Run(s.name, func(t *testing.T) {
			t.Parallel()
			if s.schema == "" {
				t.Errorf("%s: ParameterSchema() returned empty string", s.name)
				return
			}
			// Verify it's valid JSON
			var raw json.RawMessage
			if err := json.Unmarshal([]byte(s.schema), &raw); err != nil {
				t.Errorf("%s: ParameterSchema() returned invalid JSON: %v", s.name, err)
			}
		})
	}
}
