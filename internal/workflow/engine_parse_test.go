package workflow

import "testing"

func TestNormalizeToolName(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		// Edit variants (CR-01)
		{"edit lowercase", "edit", "Edit"},
		{"search_replace", "search_replace", "Edit"},
		{"fileedit", "fileedit", "Edit"},
		{"file_edit", "file_edit", "Edit"},
		{"Edit mixed case", "Edit", "Edit"},
		{"SEARCH_REPLACE upper", "SEARCH_REPLACE", "Edit"},

		// TodoWrite variants (CR-01)
		{"todowrite lowercase", "todowrite", "TodoWrite"},
		{"todo_write", "todo_write", "TodoWrite"},

		// AskUserQuestion variants (CR-01)
		{"askuserquestion lowercase", "askuserquestion", "AskUserQuestion"},
		{"ask_user_question", "ask_user_question", "AskUserQuestion"},
		{"ask_user", "ask_user", "AskUserQuestion"},

		// Existing mappings preserved
		{"bash", "bash", "Bash"},
		{"shell", "shell", "Bash"},
		{"fileread", "fileread", "FileRead"},
		{"read_file", "read_file", "FileRead"},
		{"filewrite", "filewrite", "FileWrite"},
		{"write_file", "write_file", "FileWrite"},
		{"glob", "glob", "Glob"},
		{"find_files", "find_files", "Glob"},
		{"grep", "grep", "Grep"},
		{"search", "search", "Grep"},
		{"web_fetch", "web_fetch", "WebFetch"},
		{"fetch", "fetch", "WebFetch"},

		// Unknown tool passes through unchanged
		{"unknown tool", "unknown_tool", "unknown_tool"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := normalizeToolName(tt.input)
			if got != tt.expected {
				t.Errorf("normalizeToolName(%q) = %q, want %q", tt.input, got, tt.expected)
			}
		})
	}
}
