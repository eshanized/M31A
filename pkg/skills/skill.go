package skills

// Skill represents a composable slash command defined as a Markdown file
// with YAML frontmatter. Skills can be invoked by users as /skill-name
// or by the LLM as a tool.
type Skill struct {
	Name        string
	Description string
	Slash       bool
	Content     string
	Location    string
}

// IsValid returns true if the skill has the minimum required fields.
func (s Skill) IsValid() bool {
	return s.Name != "" && s.Content != ""
}
