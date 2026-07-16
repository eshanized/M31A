package skills

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSkill_IsValid(t *testing.T) {
	tests := []struct {
		name  string
		skill Skill
		want  bool
	}{
		{"valid", Skill{Name: "test", Content: "content"}, true},
		{"missing name", Skill{Content: "content"}, false},
		{"missing content", Skill{Name: "test"}, false},
		{"empty", Skill{}, false},
		{"name only", Skill{Name: "test"}, false},
		{"content only", Skill{Content: "content"}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.skill.IsValid(); got != tt.want {
				t.Errorf("IsValid() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestFrontmatterField(t *testing.T) {
	tests := []struct {
		name        string
		frontmatter string
		key         string
		want        string
	}{
		{"simple", "name: test\n description: hello", "name", "test"},
		{"quoted double", `name: "test name"`, "name", "test name"},
		{"quoted single", "name: 'test name'", "name", "test name"},
		{"missing key", "name: test", "description", ""},
		{"empty frontmatter", "", "name", ""},
		{"whitespace", "  name:   test  ", "name", "test"},
		{"multi-line", "name: test\ndescription: hello\nslash: true", "description", "hello"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := frontmatterField(tt.frontmatter, tt.key)
			if got != tt.want {
				t.Errorf("frontmatterField() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestFrontmatterBool(t *testing.T) {
	tests := []struct {
		name        string
		frontmatter string
		key         string
		want        bool
	}{
		{"true", "slash: true", "slash", true},
		{"yes", "slash: yes", "slash", true},
		{"one", "slash: 1", "slash", true},
		{"false", "slash: false", "slash", false},
		{"no", "slash: no", "slash", false},
		{"missing", "", "slash", false},
		{"case insensitive", "slash: TRUE", "slash", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := frontmatterBool(tt.frontmatter, tt.key); got != tt.want {
				t.Errorf("frontmatterBool() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestLoadFile(t *testing.T) {
	dir := t.TempDir()

	// Create a valid skill file
	skillContent := `---
name: test-skill
description: A test skill
slash: true
---
# Test Skill

This is a test skill content.
`
	skillPath := filepath.Join(dir, "test.md")
	if err := os.WriteFile(skillPath, []byte(skillContent), 0644); err != nil {
		t.Fatal(err)
	}

	skill, err := LoadFile(skillPath)
	if err != nil {
		t.Fatalf("LoadFile() error = %v", err)
	}

	if skill.Name != "test-skill" {
		t.Errorf("Name = %q, want %q", skill.Name, "test-skill")
	}
	if skill.Description != "A test skill" {
		t.Errorf("Description = %q, want %q", skill.Description, "A test skill")
	}
	if !skill.Slash {
		t.Error("Slash = false, want true")
	}
	if skill.Location != skillPath {
		t.Errorf("Location = %q, want %q", skill.Location, skillPath)
	}
	if skill.Content == "" {
		t.Error("Content is empty")
	}
}

func TestLoadFile_NoFrontmatter(t *testing.T) {
	dir := t.TempDir()

	skillContent := `# Just a markdown file

No frontmatter here.
`
	skillPath := filepath.Join(dir, "plain.md")
	if err := os.WriteFile(skillPath, []byte(skillContent), 0644); err != nil {
		t.Fatal(err)
	}

	skill, err := LoadFile(skillPath)
	if err != nil {
		t.Fatalf("LoadFile() error = %v", err)
	}

	// Name should be derived from filename
	if skill.Name != "plain" {
		t.Errorf("Name = %q, want %q", skill.Name, "plain")
	}
	if !skill.Slash {
		t.Error("Slash should default to true when not in frontmatter")
	}
}

func TestLoadFile_SKILLDirectory(t *testing.T) {
	dir := t.TempDir()

	skillContent := `---
name: dir-skill
description: From directory
---
Content here.
`
	subdir := filepath.Join(dir, "my-skill")
	if err := os.MkdirAll(subdir, 0755); err != nil {
		t.Fatal(err)
	}
	skillPath := filepath.Join(subdir, "SKILL.md")
	if err := os.WriteFile(skillPath, []byte(skillContent), 0644); err != nil {
		t.Fatal(err)
	}

	skill, err := LoadFile(skillPath)
	if err != nil {
		t.Fatalf("LoadFile() error = %v", err)
	}

	if skill.Name != "dir-skill" {
		t.Errorf("Name = %q, want %q", skill.Name, "dir-skill")
	}
}

func TestLoadFile_SlashFalse(t *testing.T) {
	dir := t.TempDir()

	skillContent := `---
name: no-slash
slash: false
---
Content.
`
	skillPath := filepath.Join(dir, "noslash.md")
	if err := os.WriteFile(skillPath, []byte(skillContent), 0644); err != nil {
		t.Fatal(err)
	}

	skill, err := LoadFile(skillPath)
	if err != nil {
		t.Fatalf("LoadFile() error = %v", err)
	}

	if skill.Slash {
		t.Error("Slash = true, want false")
	}
}

func TestLoadFile_NotFound(t *testing.T) {
	_, err := LoadFile("/nonexistent/path/skill.md")
	if err == nil {
		t.Error("expected error for nonexistent file")
	}
}

func TestDiscover_EmptyDirs(t *testing.T) {
	skills := Discover("", "")
	if len(skills) != 0 {
		t.Errorf("Discover with empty dirs returned %d skills, want 0", len(skills))
	}
}

func TestDiscover_NonexistentDirs(t *testing.T) {
	skills := Discover("/nonexistent/base", "/nonexistent/project")
	if len(skills) != 0 {
		t.Errorf("Discover with nonexistent dirs returned %d skills, want 0", len(skills))
	}
}

func TestDiscover_ProjectSkills(t *testing.T) {
	dir := t.TempDir()

	// Create project skills directory
	projectSkillsDir := filepath.Join(dir, "project", ".m31a", "skills")
	if err := os.MkdirAll(projectSkillsDir, 0755); err != nil {
		t.Fatal(err)
	}

	skillContent := `---
name: project-skill
---
Project content.
`
	skillPath := filepath.Join(projectSkillsDir, "project-skill.md")
	if err := os.WriteFile(skillPath, []byte(skillContent), 0644); err != nil {
		t.Fatal(err)
	}

	skills := Discover("", filepath.Join(dir, "project"))
	if len(skills) != 1 {
		t.Fatalf("Discover returned %d skills, want 1", len(skills))
	}
	if skills[0].Name != "project-skill" {
		t.Errorf("skill Name = %q, want %q", skills[0].Name, "project-skill")
	}
}

func TestDiscover_GlobalSkills(t *testing.T) {
	dir := t.TempDir()

	// Create global skills directory
	globalSkillsDir := filepath.Join(dir, "global", "skills")
	if err := os.MkdirAll(globalSkillsDir, 0755); err != nil {
		t.Fatal(err)
	}

	skillContent := `---
name: global-skill
---
Global content.
`
	skillPath := filepath.Join(globalSkillsDir, "global-skill.md")
	if err := os.WriteFile(skillPath, []byte(skillContent), 0644); err != nil {
		t.Fatal(err)
	}

	skills := Discover(filepath.Join(dir, "global"), "")
	if len(skills) != 1 {
		t.Fatalf("Discover returned %d skills, want 1", len(skills))
	}
	if skills[0].Name != "global-skill" {
		t.Errorf("skill Name = %q, want %q", skills[0].Name, "global-skill")
	}
}

func TestDiscover_Deduplication(t *testing.T) {
	dir := t.TempDir()

	// Create project skills
	projectDir := filepath.Join(dir, "project", ".m31a", "skills")
	if err := os.MkdirAll(projectDir, 0755); err != nil {
		t.Fatal(err)
	}

	skillContent := `---
name: shared-skill
---
Project version.
`
	if err := os.WriteFile(filepath.Join(projectDir, "shared.md"), []byte(skillContent), 0644); err != nil {
		t.Fatal(err)
	}

	// Create global skills with same name
	globalDir := filepath.Join(dir, "global", "skills")
	if err := os.MkdirAll(globalDir, 0755); err != nil {
		t.Fatal(err)
	}

	globalContent := `---
name: shared-skill
---
Global version.
`
	if err := os.WriteFile(filepath.Join(globalDir, "shared.md"), []byte(globalContent), 0644); err != nil {
		t.Fatal(err)
	}

	skills := Discover(filepath.Join(dir, "global"), filepath.Join(dir, "project"))
	if len(skills) != 1 {
		t.Fatalf("Discover returned %d skills, want 1 (deduplication)", len(skills))
	}

	// Project should take precedence
	if skills[0].Content == "" {
		t.Error("expected non-empty content")
	}
}

func TestDiscover_Subdirectory(t *testing.T) {
	dir := t.TempDir()

	// Create skills directory with subdirectory containing SKILL.md
	skillsDir := filepath.Join(dir, "skills")
	subDir := filepath.Join(skillsDir, "sub-skill")
	if err := os.MkdirAll(subDir, 0755); err != nil {
		t.Fatal(err)
	}

	skillContent := `---
name: sub-skill
---
Sub content.
`
	if err := os.WriteFile(filepath.Join(subDir, "SKILL.md"), []byte(skillContent), 0644); err != nil {
		t.Fatal(err)
	}

	skills := Discover(dir, "")
	if len(skills) != 1 {
		t.Fatalf("Discover returned %d skills, want 1", len(skills))
	}
	if skills[0].Name != "sub-skill" {
		t.Errorf("skill Name = %q, want %q", skills[0].Name, "sub-skill")
	}
}

func TestScanDir_Nonexistent(t *testing.T) {
	skills := scanDir("/nonexistent/dir")
	if len(skills) != 0 {
		t.Errorf("scanDir returned %d skills, want 0", len(skills))
	}
}

func TestScanDir_EmptyDir(t *testing.T) {
	dir := t.TempDir()
	skills := scanDir(dir)
	if len(skills) != 0 {
		t.Errorf("scanDir returned %d skills, want 0", len(skills))
	}
}

func TestScanDir_IgnoresNonMarkdown(t *testing.T) {
	dir := t.TempDir()

	// Create non-markdown files
	if err := os.WriteFile(filepath.Join(dir, "readme.txt"), []byte("text"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "data.json"), []byte("{}"), 0644); err != nil {
		t.Fatal(err)
	}

	skills := scanDir(dir)
	if len(skills) != 0 {
		t.Errorf("scanDir returned %d skills, want 0", len(skills))
	}
}
