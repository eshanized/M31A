package workflow

import (
	"testing"
)

func TestParseOutline(t *testing.T) {
	tests := []struct {
		name       string
		content    string
		wantWaves  int
		wantTasks  int
		wantErr    bool
	}{
		{
			name: "valid outline",
			content: `{
				"title": "Build API",
				"waves": [
					{"wave": 1, "tasks": [{"id": 1, "action": "Create", "description": "Setup", "dependencies": [], "category": "Setup"}]},
					{"wave": 2, "tasks": [{"id": 2, "action": "Create", "description": "Routes", "dependencies": [1], "category": "Core"}]}
				]
			}`,
			wantWaves: 2,
			wantTasks: 2,
			wantErr:   false,
		},
		{
			name:    "no JSON",
			content: "This is just text with no JSON object",
			wantErr: true,
		},
		{
			name:    "empty waves",
			content: `{"title": "Empty", "waves": []}`,
			wantErr: true,
		},
		{
			name: "in code block",
			content: "```json\n{\"title\": \"Test\", \"waves\": [{\"wave\": 1, \"tasks\": [{\"id\": 1, \"action\": \"Create\", \"description\": \"A\", \"dependencies\": [], \"category\": \"X\"}]}]}\n```",
			wantWaves: 1,
			wantTasks: 1,
			wantErr:   false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			outline, err := parseOutline(tt.content)
			if (err != nil) != tt.wantErr {
				t.Errorf("parseOutline() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if tt.wantErr {
				return
			}
			if len(outline.Waves) != tt.wantWaves {
				t.Errorf("waves = %d, want %d", len(outline.Waves), tt.wantWaves)
			}
			if outline.TotalTasks != tt.wantTasks {
				t.Errorf("total tasks = %d, want %d", outline.TotalTasks, tt.wantTasks)
			}
		})
	}
}

func TestFindWave(t *testing.T) {
	outline := &PlanOutline{
		Waves: []WaveOutline{
			{Wave: 1, Tasks: []TaskStub{{ID: 1}}},
			{Wave: 2, Tasks: []TaskStub{{ID: 2}, {ID: 3}}},
		},
	}

	w := findWave(outline, 1)
	if w == nil || w.Wave != 1 {
		t.Error("expected to find wave 1")
	}

	w = findWave(outline, 2)
	if w == nil || len(w.Tasks) != 2 {
		t.Error("expected wave 2 with 2 tasks")
	}

	w = findWave(outline, 99)
	if w != nil {
		t.Error("expected nil for non-existent wave")
	}
}

func TestShouldChunk(t *testing.T) {
	dir := t.TempDir()
	e := &Engine{workDir: dir}

	// Simple goal should not chunk
	if e.shouldChunk("add a button") {
		t.Error("simple goal should not trigger chunking")
	}

	// Complex goal with few files should not chunk (need 30+ files)
	if e.shouldChunk("build and implement and integrate full-stack microservice") {
		t.Error("complex goal with no files should not trigger chunking")
	}
}

func TestChunkThreshold(t *testing.T) {
	e := &Engine{cfg: nil}
	if e.chunkThreshold() != 10 {
		t.Errorf("default chunk threshold = %d, want 10", e.chunkThreshold())
	}
}
