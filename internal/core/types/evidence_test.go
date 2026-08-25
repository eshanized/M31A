package types

import (
	"encoding/json"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestEvidencePackAddAssignsSequentialIDs(t *testing.T) {
	pack := NewEvidencePack("why does auth middleware exist")

	first := pack.Add(EvidenceSource, "internal/middleware/auth.go:10", "func Auth()")
	second := pack.Add(EvidenceCallers, "internal/handler/user.go:42", "Create calls Auth")
	third := pack.Add(EvidenceBlame, "abc1234", "introduce auth middleware")

	assert.Equal(t, 1, first.ID)
	assert.Equal(t, 2, second.ID)
	assert.Equal(t, 3, third.ID)

	// Kind/Ref/Snippet preserved on each item.
	assert.Equal(t, EvidenceSource, first.Kind)
	assert.Equal(t, "internal/middleware/auth.go:10", first.Ref)
	assert.Equal(t, "func Auth()", first.Snippet)

	require.Len(t, pack.Sections, 3)
	assert.Equal(t, "why does auth middleware exist", pack.Query)
	assert.False(t, pack.Truncated)
}

func TestNewEvidencePackNonNilSections(t *testing.T) {
	pack := NewEvidencePack("query")
	require.NotNil(t, pack.Sections)
	assert.Empty(t, pack.Sections)
	assert.Equal(t, "query", pack.Query)
}

func TestEvidenceJSONSnakeCaseKeys(t *testing.T) {
	ev := Evidence{ID: 1, Kind: EvidenceSource, Ref: "a.go:10", Snippet: "x"}
	data, err := json.Marshal(ev)
	require.NoError(t, err)

	var decoded map[string]any
	require.NoError(t, json.Unmarshal(data, &decoded))
	for _, key := range []string{"id", "kind", "ref", "snippet"} {
		assert.Contains(t, decoded, key, "expected snake_case key %q in %s", key, data)
	}
}

func TestEvidencePackJSONTags(t *testing.T) {
	pack := NewEvidencePack("q")
	pack.Add(EvidenceTest, "auth_test.go:1", "TestAuth")
	pack.Truncated = true

	data, err := json.Marshal(pack)
	require.NoError(t, err)

	var decoded map[string]any
	require.NoError(t, json.Unmarshal(data, &decoded))
	for _, key := range []string{"query", "sections", "truncated"} {
		assert.Contains(t, decoded, key, "expected snake_case key %q in %s", key, data)
	}
	assert.Equal(t, true, decoded["truncated"])
}

func TestCitationJSONSnakeCaseKeys(t *testing.T) {
	c := Citation{Marker: 1, Kind: EvidenceCommit, Ref: "abc1234", Note: "introduced symbol"}
	data, err := json.Marshal(c)
	require.NoError(t, err)

	var decoded map[string]any
	require.NoError(t, json.Unmarshal(data, &decoded))
	for _, key := range []string{"marker", "kind", "ref", "note"} {
		assert.Contains(t, decoded, key, "expected snake_case key %q in %s", key, data)
	}
}

func TestIntelligenceEventConstants(t *testing.T) {
	assert.Equal(t, EventType("InvestigationStarted"), EventInvestigationStarted)
	assert.Equal(t, EventType("InvestigationCompleted"), EventInvestigationCompleted)
	assert.Equal(t, EventType("DependencyChecked"), EventDependencyChecked)
}
