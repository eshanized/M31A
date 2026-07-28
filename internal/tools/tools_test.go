package tools

import (
	"encoding/json"
	"testing"

	"github.com/eshanized/M31A/internal/tools/fileops"
	"github.com/eshanized/M31A/internal/tools/search"
	"github.com/eshanized/M31A/internal/tools/todo"
)

func TestParameterSchema_AllTools(t *testing.T) {
	t.Parallel()

	// All tools that implement ParameterSchema
	schemas := []struct {
		name   string
		schema string
	}{
		{"Bash", NewBash(".", 1800, nil, nil).ParameterSchema()},
		{"FileRead", fileops.NewFileRead(".").ParameterSchema()},
		{"FileWrite", fileops.NewFileWrite(".", ".").ParameterSchema()},
		{"Glob", search.NewGlob(".").ParameterSchema()},
		{"Grep", search.NewGrep(".").ParameterSchema()},
		{"Edit", fileops.NewEdit(".", ".").ParameterSchema()},
		{"WebFetch", search.NewWebFetch(".", 3, nil).ParameterSchema()},
		{"TodoWrite", todo.NewTodoWrite(".", "test").ParameterSchema()},
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
