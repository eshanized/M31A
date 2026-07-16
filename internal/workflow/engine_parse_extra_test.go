package workflow

import (
	"testing"

	m31types "github.com/eshanized/M31A/pkg/types"
)

func TestStripJSONComments(t *testing.T) {
	tests := []struct {
		name string
		in   string
		want string
	}{
		{
			name: "no comments",
			in:   `{"key": "value"}`,
			want: `{"key": "value"}`,
		},
		{
			name: "line comment removed",
			in: `{
				// this is a comment
				"key": "value"
			}`,
			want: `{
				
				"key": "value"
			}`,
		},
		{
			name: "block comment on single line removed",
			in: `{
				/* block comment */
				"key": "value"
			}`,
			want: `{
				
				"key": "value"
			}`,
		},
		{
			name: "comment inside string preserved",
			in:   `{"url": "http://example.com//path"}`,
			want: `{"url": "http://example.com//path"}`,
		},
		{
			name: "block comment inside string preserved",
			in:   `{"code": "/* not a comment */"}`,
			want: `{"code": "/* not a comment */"}`,
		},
		{
			name: "empty string",
			in:   "",
			want: "",
		},
		{
			name: "only line comment",
			in:   "// just a comment\n",
			want: "\n",
		},
		{
			name: "only block comment",
			in:   "/* block comment */",
			want: "",
		},
		{
			name: "multi-line block comment",
			in: `{
				/*
				line 1
				line 2
				*/
				"key": "value"
			}`,
			want: `{
				
				"key": "value"
			}`,
		},
		{
			name: "multiple line comments",
			in: `// first
{
	"key": "value" // inline
}`,
			want: `
{
	"key": "value" 
}`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := stripJSONComments(tt.in)
			if got != tt.want {
				t.Errorf("stripJSONComments() =\n%q\nwant:\n%q", got, tt.want)
			}
		})
	}
}

func TestNormalizeTrailingCommas(t *testing.T) {
	tests := []struct {
		name string
		in   string
		want string
	}{
		{
			name: "no trailing commas",
			in:   `{"key": "value"}`,
			want: `{"key": "value"}`,
		},
		{
			name: "trailing comma before closing brace",
			in:   `{"key": "value",}`,
			want: `{"key": "value"}`,
		},
		{
			name: "trailing comma before closing bracket",
			in:   `[1, 2, 3,]`,
			want: `[1, 2, 3]`,
		},
		{
			name: "nested trailing commas",
			in:   `{"arr": [1, 2,], "obj": {"a": 1,},}`,
			want: `{"arr": [1, 2], "obj": {"a": 1}}`,
		},
		{
			name: "comma inside string preserved",
			in:   `{"text": "hello, world,"}`,
			want: `{"text": "hello, world,"}`,
		},
		{
			name: "comma between values preserved",
			in:   `{"a": 1, "b": 2}`,
			want: `{"a": 1, "b": 2}`,
		},
		{
			name: "empty object",
			in:   `{}`,
			want: `{}`,
		},
		{
			name: "empty array",
			in:   `[]`,
			want: `[]`,
		},
		{
			name: "trailing comma with newline before bracket",
			in:   "{\n\t\"a\": 1,\n}",
			want: "{\n\t\"a\": 1\n}",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := normalizeTrailingCommas(tt.in)
			if got != tt.want {
				t.Errorf("normalizeTrailingCommas(%q) =\n  %q\nwant:\n  %q", tt.in, got, tt.want)
			}
		})
	}
}

func TestParseQuestions(t *testing.T) {
	tests := []struct {
		name    string
		content string
		want    []string
	}{
		{
			name:    "numbered questions with question marks",
			content: "1. What framework should be used?\n2. How should auth work?\n3. What database?",
			want:    []string{"What framework should be used?", "How should auth work?", "What database?"},
		},
		{
			name:    "numbered items without question marks",
			content: "1. Use React for the frontend\n2. PostgreSQL for the database",
			want:    []string{"Use React for the frontend", "PostgreSQL for the database"},
		},
		{
			name:    "fallback to lines with question marks",
			content: "Some preamble text\nWhat is the best approach?\nHow should we implement it?",
			want:    []string{"What is the best approach?", "How should we implement it?"},
		},
		{
			name:    "cap at 4 questions",
			content: "1. Q1?\n2. Q2?\n3. Q3?\n4. Q4?\n5. Q5?\n6. Q6?",
			want:    []string{"Q1?", "Q2?", "Q3?", "Q4?"},
		},
		{
			name:    "empty content",
			content: "",
			want:    nil,
		},
		{
			name:    "no questions at all",
			content: "This is just a plain statement.",
			want:    nil,
		},
		{
			name:    "question with em-dash suggestion truncated",
			content: "1. What framework? — I suggest React.",
			want:    []string{"What framework?"},
		},
		{
			name:    "multiline question",
			content: "1. What framework should be used for the frontend?\n2. What database?",
			want:    []string{"What framework should be used for the frontend?", "What database?"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseQuestions(tt.content)
			if len(got) != len(tt.want) {
				t.Errorf("parseQuestions() returned %d questions, want %d: got=%v", len(got), len(tt.want), got)
				return
			}
			for i, q := range got {
				if q != tt.want[i] {
					t.Errorf("parseQuestions()[%d] =\n  %q\nwant:\n  %q", i, q, tt.want[i])
				}
			}
		})
	}
}

func TestParseTasksFromJSON(t *testing.T) {
	t.Run("valid JSON", func(t *testing.T) {
		input := `[{"id": 1, "description": "Task 1", "action": "create", "acceptance_criteria": ["works"]}]`
		tasks, err := parseTasksFromJSON(input)
		if err != nil {
			t.Fatalf("parseTasksFromJSON() error = %v", err)
		}
		if len(tasks) != 1 {
			t.Errorf("parseTasksFromJSON() returned %d tasks, want 1", len(tasks))
		}
		if tasks[0].ID != 1 {
			t.Errorf("tasks[0].ID = %d, want 1", tasks[0].ID)
		}
	})

	t.Run("in code block", func(t *testing.T) {
		input := "```json\n[{\"id\": 1, \"description\": \"Task 1\", \"action\": \"create\", \"acceptance_criteria\": [\"works\"]}]\n```"
		tasks, err := parseTasksFromJSON(input)
		if err != nil {
			t.Fatalf("parseTasksFromJSON() error = %v", err)
		}
		if len(tasks) != 1 {
			t.Errorf("parseTasksFromJSON() returned %d tasks, want 1", len(tasks))
		}
	})

	t.Run("with trailing commas", func(t *testing.T) {
		input := `[{"id": 1, "description": "Task 1", "action": "create", "acceptance_criteria": ["works"],}]`
		tasks, err := parseTasksFromJSON(input)
		if err != nil {
			t.Fatalf("parseTasksFromJSON() error = %v", err)
		}
		if len(tasks) != 1 {
			t.Errorf("parseTasksFromJSON() returned %d tasks, want 1", len(tasks))
		}
	})

	t.Run("with comments", func(t *testing.T) {
		input := `// Task list
[
  {"id": 1, "description": "Task 1", "action": "create", "acceptance_criteria": ["works"]}
]`
		tasks, err := parseTasksFromJSON(input)
		if err != nil {
			t.Fatalf("parseTasksFromJSON() error = %v", err)
		}
		if len(tasks) != 1 {
			t.Errorf("parseTasksFromJSON() returned %d tasks, want 1", len(tasks))
		}
	})

	t.Run("no JSON array", func(t *testing.T) {
		input := "no json here"
		_, err := parseTasksFromJSON(input)
		if err == nil {
			t.Error("parseTasksFromJSON() expected error for non-JSON input")
		}
	})

	t.Run("multiple tasks", func(t *testing.T) {
		input := `[
			{"id": 1, "description": "Task 1", "action": "create", "acceptance_criteria": ["works"]},
			{"id": 2, "description": "Task 2", "action": "test", "acceptance_criteria": ["passes"], "dependencies": [1]}
		]`
		tasks, err := parseTasksFromJSON(input)
		if err != nil {
			t.Fatalf("parseTasksFromJSON() error = %v", err)
		}
		if len(tasks) != 2 {
			t.Errorf("parseTasksFromJSON() returned %d tasks, want 2", len(tasks))
		}
		if tasks[1].ID != 2 {
			t.Errorf("tasks[1].ID = %d, want 2", tasks[1].ID)
		}
	})
}

func TestExtractJSONArray(t *testing.T) {
	tests := []struct {
		name      string
		input     string
		want      string
		wantEmpty bool
	}{
		{
			name:      "starts with array",
			input:     `[{"a": 1}]`,
			want:      `[{"a": 1}]`,
			wantEmpty: false,
		},
		{
			name:      "array embedded in text",
			input:     `Here is the JSON: [{"a": 1}] and more text`,
			want:      `[{"a": 1}]`,
			wantEmpty: false,
		},
		{
			name:      "nested arrays",
			input:     `[{"a": [1, 2]}]`,
			want:      `[{"a": [1, 2]}]`,
			wantEmpty: false,
		},
		{
			name:      "no array",
			input:     `no json here`,
			want:      "",
			wantEmpty: true,
		},
		{
			name:      "incomplete array",
			input:     `[{"a": 1`,
			want:      "",
			wantEmpty: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := extractJSONArray(tt.input)
			if tt.wantEmpty {
				if got != "" {
					t.Errorf("extractJSONArray(%q) = %q, want empty", tt.input, got)
				}
			} else {
				if got != tt.want {
					t.Errorf("extractJSONArray(%q) = %q, want %q", tt.input, got, tt.want)
				}
			}
		})
	}
}

func TestHasCycle(t *testing.T) {
	tests := []struct {
		name  string
		tasks []m31types.Task
		want  bool
	}{
		{
			name:  "no dependencies",
			tasks: []m31types.Task{{ID: 1, Description: "t", Action: "a"}, {ID: 2, Description: "t", Action: "a"}},
			want:  false,
		},
		{
			name:  "linear chain no cycle",
			tasks: []m31types.Task{{ID: 1, Description: "t", Action: "a"}, {ID: 2, Description: "t", Action: "a", Dependencies: []int{1}}, {ID: 3, Description: "t", Action: "a", Dependencies: []int{2}}},
			want:  false,
		},
		{
			name: "simple 2-node cycle",
			tasks: []m31types.Task{
				{ID: 1, Description: "t", Action: "a", Dependencies: []int{2}},
				{ID: 2, Description: "t", Action: "a", Dependencies: []int{1}},
			},
			want: true,
		},
		{
			name: "3-node cycle",
			tasks: []m31types.Task{
				{ID: 1, Description: "t", Action: "a", Dependencies: []int{2}},
				{ID: 2, Description: "t", Action: "a", Dependencies: []int{3}},
				{ID: 3, Description: "t", Action: "a", Dependencies: []int{1}},
			},
			want: true,
		},
		{
			name: "self reference",
			tasks: []m31types.Task{
				{ID: 1, Description: "t", Action: "a", Dependencies: []int{1}},
			},
			want: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := hasCycle(tt.tasks)
			if got != tt.want {
				t.Errorf("hasCycle() = %v, want %v", got, tt.want)
			}
		})
	}
}
