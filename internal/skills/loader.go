package skills

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// frontmatterField extracts a YAML frontmatter field value from a simple
// key: value format. Does not require a full YAML parser for the simple
// frontmatter used by skills.
func frontmatterField(frontmatter, key string) string {
	prefix := key + ":"
	for _, line := range strings.Split(frontmatter, "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, prefix) {
			val := strings.TrimSpace(strings.TrimPrefix(line, prefix))
			if len(val) >= 2 && ((val[0] == '"' && val[len(val)-1] == '"') ||
				(val[0] == '\'' && val[len(val)-1] == '\'')) {
				val = val[1 : len(val)-1]
			}
			return val
		}
	}
	return ""
}

// frontmatterBool extracts a boolean field from frontmatter.
func frontmatterBool(frontmatter, key string) bool {
	val := strings.ToLower(frontmatterField(frontmatter, key))
	return val == "true" || val == "yes" || val == "1"
}

// LoadFile reads a skill file (Markdown with YAML frontmatter) and returns
// a Skill. The frontmatter is delimited by --- lines at the start of the file.
func LoadFile(path string) (Skill, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return Skill{}, fmt.Errorf("read skill file: %w", err)
	}

	content := string(data)

	frontmatter := ""
	body := content

	if strings.HasPrefix(content, "---") {
		parts := strings.SplitN(content, "---", 3)
		if len(parts) >= 3 {
			frontmatter = parts[1]
			body = strings.TrimSpace(parts[2])
		}
	}

	name := frontmatterField(frontmatter, "name")
	if name == "" {
		base := strings.TrimSuffix(filepath.Base(path), ".md")
		if base == "SKILL" {
			base = filepath.Base(filepath.Dir(path))
		}
		name = base
	}

	description := frontmatterField(frontmatter, "description")
	slash := frontmatterBool(frontmatter, "slash")
	if !strings.Contains(frontmatter, "slash:") {
		slash = true
	}

	return Skill{
		Name:        name,
		Description: description,
		Slash:       slash,
		Content:     body,
		Location:    path,
	}, nil
}
