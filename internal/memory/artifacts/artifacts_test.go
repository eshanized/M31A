package artifacts

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/google/uuid"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestWriteProjectMD(t *testing.T) {
	dir := t.TempDir()
	m31aDir := filepath.Join(dir, ".m31a")
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	project := &types.Project{
		ID:        uuid.New(),
		RootPath:  "/test",
		Name:      "Test Project",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	err := WriteProjectMD(m31aDir, project)
	require.NoError(t, err)

	// Verify file exists
	path := filepath.Join(m31aDir, "project.md")
	_, err = os.Stat(path)
	require.NoError(t, err)
}

func TestReadProjectMD(t *testing.T) {
	dir := t.TempDir()
	m31aDir := filepath.Join(dir, ".m31a")
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	project := &types.Project{
		ID:        uuid.New(),
		RootPath:  "/test",
		Name:      "Test Project",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	err := WriteProjectMD(m31aDir, project)
	require.NoError(t, err)

	readProject, err := ReadProjectMD(m31aDir)
	require.NoError(t, err)
	assert.Equal(t, project.ID, readProject.ID)
	assert.Equal(t, project.Name, readProject.Name)
}

func TestWriteDecision(t *testing.T) {
	dir := t.TempDir()
	m31aDir := filepath.Join(dir, ".m31a")
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	decision := &types.Decision{
		ID:        uuid.New(),
		Title:     "Test Decision",
		Status:    types.DecisionStatusAccepted,
		Rationale: "This is the rationale",
		Timestamp: time.Now(),
		Alternatives: []types.Alternative{
			{
				Description: "Alternative 1",
				Pros:        []string{"Pro 1", "Pro 2"},
				Cons:        []string{"Con 1"},
			},
		},
	}

	err := WriteDecision(m31aDir, decision)
	require.NoError(t, err)

	// Verify file exists
	files, err := os.ReadDir(filepath.Join(m31aDir, "decisions"))
	require.NoError(t, err)
	assert.Len(t, files, 1)
}

func TestListDecisions(t *testing.T) {
	dir := t.TempDir()
	m31aDir := filepath.Join(dir, ".m31a")
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	// Write multiple decisions
	for i := 0; i < 3; i++ {
		decision := &types.Decision{
			ID:        uuid.New(),
			Title:     fmt.Sprintf("Decision %d", i),
			Status:    types.DecisionStatusAccepted,
			Rationale: fmt.Sprintf("Rationale %d", i),
			Timestamp: time.Now().Add(time.Duration(i) * time.Hour),
		}
		err := WriteDecision(m31aDir, decision)
		require.NoError(t, err)
	}

	decisions, err := ListDecisions(m31aDir)
	require.NoError(t, err)
	assert.Len(t, decisions, 3)
	// Should be sorted by timestamp descending
	assert.True(t, decisions[0].Timestamp.After(decisions[1].Timestamp))
}

func TestWriteResearch(t *testing.T) {
	dir := t.TempDir()
	m31aDir := filepath.Join(dir, ".m31a")
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	research := &types.Research{
		ID:        uuid.New(),
		Question:  "How to implement X?",
		Findings:  "Found that Y works best",
		Confidence: 0.9,
		Sources:   []string{"source1", "source2"},
		CreatedAt: time.Now(),
		CreatedBy: uuid.New(),
	}

	err := WriteResearch(m31aDir, research)
	require.NoError(t, err)

	// Verify file exists
	files, err := os.ReadDir(filepath.Join(m31aDir, "research"))
	require.NoError(t, err)
	assert.Len(t, files, 1)
}

func TestListResearch(t *testing.T) {
	dir := t.TempDir()
	m31aDir := filepath.Join(dir, ".m31a")
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	// Write multiple research entries
	for i := 0; i < 3; i++ {
		research := &types.Research{
			ID:        uuid.New(),
			Question:  fmt.Sprintf("Question %d", i),
			Findings:  fmt.Sprintf("Findings %d", i),
			Confidence: 0.8 + float64(i)*0.05,
			Sources:   []string{"source"},
			CreatedAt: time.Now().Add(time.Duration(i) * time.Hour),
			CreatedBy: uuid.New(),
		}
		err := WriteResearch(m31aDir, research)
		require.NoError(t, err)
	}

	researchList, err := ListResearch(m31aDir)
	require.NoError(t, err)
	assert.Len(t, researchList, 3)
	// Should be sorted by created_at descending
	assert.True(t, researchList[0].CreatedAt.After(researchList[1].CreatedAt))
}

func TestGitIgnorePolicy(t *testing.T) {
	dir := t.TempDir()
	m31aDir := filepath.Join(dir, ".m31a")
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	gitignorePath := filepath.Join(m31aDir, ".gitignore")
	content := `# Caches
caches/
*.cache
# Secrets (never committed)
*.key
*.pem
# Embeddings
embeddings/
# Transient indexes
indexes/
# Graph data (regenerated)
graphs/
`

	err := os.WriteFile(gitignorePath, []byte(content), 0644)
	require.NoError(t, err)

	// Verify content
	data, err := os.ReadFile(gitignorePath)
	require.NoError(t, err)
	contentStr := string(data)

	assert.Contains(t, contentStr, "caches/")
	assert.Contains(t, contentStr, "*.cache")
	assert.Contains(t, contentStr, "*.key")
	assert.Contains(t, contentStr, "*.pem")
	assert.Contains(t, contentStr, "embeddings/")
	assert.Contains(t, contentStr, "indexes/")
	assert.Contains(t, contentStr, "graphs/")
}