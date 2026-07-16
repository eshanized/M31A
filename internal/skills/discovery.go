package skills

import (
	"os"
	"path/filepath"
)

// Discover scans directories for skill files (SKILL.md or *.md files with
// YAML frontmatter). Searches both the global config directory
// (~/.m31a/skills/) and project-local directory (<project>/.m31a/skills/).
// Returns deduplicated skills, with project-local skills taking precedence.
func Discover(baseDir, projectDir string) []Skill {
	seen := make(map[string]bool)
	var skills []Skill

	// Project-local skills take precedence
	if projectDir != "" {
		projectSkills := scanDir(filepath.Join(projectDir, ".m31a", "skills"))
		for _, s := range projectSkills {
			if !seen[s.Name] {
				seen[s.Name] = true
				skills = append(skills, s)
			}
		}
	}

	// Global skills
	if baseDir != "" {
		globalSkills := scanDir(filepath.Join(baseDir, "skills"))
		for _, s := range globalSkills {
			if !seen[s.Name] {
				seen[s.Name] = true
				skills = append(skills, s)
			}
		}
	}

	return skills
}

// scanDir scans a directory for skill files.
func scanDir(dir string) []Skill {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}

	var skills []Skill
	for _, entry := range entries {
		if entry.IsDir() {
			// Check for SKILL.md inside subdirectory
			skillPath := filepath.Join(dir, entry.Name(), "SKILL.md")
			if s, err := LoadFile(skillPath); err == nil && s.IsValid() {
				skills = append(skills, s)
			}
			continue
		}

		// Check *.md files in the directory
		if filepath.Ext(entry.Name()) == ".md" {
			skillPath := filepath.Join(dir, entry.Name())
			if s, err := LoadFile(skillPath); err == nil && s.IsValid() {
				skills = append(skills, s)
			}
		}
	}

	return skills
}
