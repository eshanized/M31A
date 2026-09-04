package deps

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/memory/eventstore"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func mustMarshal(v any) json.RawMessage {
	data, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	return data
}

// newTestDepsClients creates clients pointed at the test servers.
func newTestDepsClients(depsDevSrv, osvSrv, githubSrv *httptest.Server) (*DepsDevClient, *OSVClient, *GitHubEnricher) {
	return NewDepsDevClient(depsDevSrv.URL, nil),
		NewOSVClient(osvSrv.URL, nil),
		NewGitHubEnricher(githubSrv.URL, nil, "test-token")
}

func TestAssembleVerdict_AllSourcesOK(t *testing.T) {
	// Setup test servers
	// Use a recent date so age calculation works correctly
	recentDate := time.Now().UTC().Add(-30 * 24 * time.Hour).Format(time.RFC3339)

	depsDevSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Path is not URL-escaped in httptest request URL
		assert.Contains(t, r.URL.Path, "github.com/test/mod")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"versions":[{"versionKey":{"system":"GO","name":"github.com/test/mod","version":"v1.2.3"},"publishedAt":"` + recentDate + `","isDefault":true,"isDeprecated":false,"deprecatedReason":"","licenses":["MIT"],"advisoryKeys":[]}]}`))
	}))
	defer depsDevSrv.Close()

	osvSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		assert.Equal(t, "/v1/query", r.URL.Path)
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"vulns":[]}`))
	}))
	defer osvSrv.Close()

	githubSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		assert.Equal(t, "/repos/test/mod", r.URL.Path)
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"pushed_at":"` + recentDate + `","archived":false,"open_issues_count":5,"stargazers_count":100}`))
	}))
	defer githubSrv.Close()

	depsDev, osv, github := newTestDepsClients(depsDevSrv, osvSrv, githubSrv)

	ctx := context.Background()
	verdict, err := AssembleVerdict(ctx, Deps{DepsDev: depsDev, OSV: osv, GitHub: github}, "github.com/test/mod", "v1.2.3", DefaultDepsRiskConfig(), "policy-hash-123")
	require.NoError(t, err)

	assert.Equal(t, "github.com/test/mod", verdict.Module)
	assert.Equal(t, "v1.2.3", verdict.Version)
	assert.True(t, verdict.Exists)
	assert.True(t, verdict.AgeDays >= 29 && verdict.AgeDays <= 31) // approx 30 days
	assert.Contains(t, verdict.LatestRelease, "T")
	assert.Equal(t, "active", verdict.MaintenanceClass)
	assert.Equal(t, "MIT", verdict.License)
	assert.Empty(t, verdict.Vulnerabilities)
	assert.NotNil(t, verdict.Sources)
	// Should have 3 sources (depsdev, osv, github) - graph is nil so no graph source added
	assert.Len(t, verdict.Sources, 3)
	for _, src := range verdict.Sources {
		assert.Equal(t, "ok", src.Status)
	}
	assert.Equal(t, types.ConfidenceVerified, verdict.Confidence)
}

func TestAssembleVerdict_OSVUnreachable_CapsConfidenceSpeculative(t *testing.T) {
	depsDevSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"versions":[{"versionKey":{"system":"GO","name":"github.com/test/mod","version":"v1.2.3"},"publishedAt":"2024-01-15T10:00:00Z","isDefault":true,"isDeprecated":false,"deprecatedReason":"","licenses":["MIT"],"advisoryKeys":[]}]}`))
	}))
	defer depsDevSrv.Close()

	// OSV server returns 500
	osvSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer osvSrv.Close()

	githubSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"pushed_at":"2024-01-10T10:00:00Z","archived":false,"open_issues_count":5,"stargazers_count":100}`))
	}))
	defer githubSrv.Close()

	depsDev, osv, github := newTestDepsClients(depsDevSrv, osvSrv, githubSrv)

	ctx := context.Background()
	verdict, err := AssembleVerdict(ctx, Deps{DepsDev: depsDev, OSV: osv, GitHub: github}, "github.com/test/mod", "v1.2.3", DefaultDepsRiskConfig(), "policy-hash-123")
	require.NoError(t, err)

	assert.Equal(t, types.ConfidenceSpeculative, verdict.Confidence)
	// Find OSV source - should be unavailable
	osvSource := findSource(verdict.Sources, "osv")
	assert.NotNil(t, osvSource)
	assert.Equal(t, "unavailable", osvSource.Status)
}

func TestAssembleVerdict_DepsDevUnreachable_CapsConfidenceSpeculative(t *testing.T) {
	// DepsDev server returns 500
	depsDevSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer depsDevSrv.Close()

	osvSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"vulns":[]}`))
	}))
	defer osvSrv.Close()

	githubSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"pushed_at":"2024-01-10T10:00:00Z","archived":false,"open_issues_count":5,"stargazers_count":100}`))
	}))
	defer githubSrv.Close()

	depsDev, osv, github := newTestDepsClients(depsDevSrv, osvSrv, githubSrv)

	ctx := context.Background()
	verdict, err := AssembleVerdict(ctx, Deps{DepsDev: depsDev, OSV: osv, GitHub: github}, "github.com/test/mod", "v1.2.3", DefaultDepsRiskConfig(), "policy-hash-123")
	require.NoError(t, err)

	assert.Equal(t, types.ConfidenceSpeculative, verdict.Confidence)
	depsDevSource := findSource(verdict.Sources, "depsdev")
	assert.NotNil(t, depsDevSource)
	assert.Equal(t, "unavailable", depsDevSource.Status)
}

func TestAssembleVerdict_GitHubUnreachable_CapsConfidenceSpeculative(t *testing.T) {
	depsDevSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"versions":[{"versionKey":{"system":"GO","name":"github.com/test/mod","version":"v1.2.3"},"publishedAt":"2024-01-15T10:00:00Z","isDefault":true,"isDeprecated":false,"deprecatedReason":"","licenses":["MIT"],"advisoryKeys":[]}]}`))
	}))
	defer depsDevSrv.Close()

	osvSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"vulns":[]}`))
	}))
	defer osvSrv.Close()

	// GitHub server returns 500
	githubSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer githubSrv.Close()

	depsDev, osv, github := newTestDepsClients(depsDevSrv, osvSrv, githubSrv)

	ctx := context.Background()
	verdict, err := AssembleVerdict(ctx, Deps{DepsDev: depsDev, OSV: osv, GitHub: github}, "github.com/test/mod", "v1.2.3", DefaultDepsRiskConfig(), "policy-hash-123")
	require.NoError(t, err)

	assert.Equal(t, types.ConfidenceSpeculative, verdict.Confidence)
	githubSource := findSource(verdict.Sources, "github")
	assert.NotNil(t, githubSource)
	assert.Equal(t, "unavailable", githubSource.Status)
}

func TestAssembleVerdict_EmptyVulnsFromReachableOSV_QueriedOK(t *testing.T) {
	depsDevSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"versions":[{"versionKey":{"system":"GO","name":"github.com/test/mod","version":"v1.2.3"},"publishedAt":"2024-01-15T10:00:00Z","isDefault":true,"isDeprecated":false,"deprecatedReason":"","licenses":["MIT"],"advisoryKeys":[]}]}`))
	}))
	defer depsDevSrv.Close()

	// OSV returns empty vulns but successfully
	osvSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"vulns":[]}`))
	}))
	defer osvSrv.Close()

	githubSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"pushed_at":"2024-01-10T10:00:00Z","archived":false,"open_issues_count":5,"stargazers_count":100}`))
	}))
	defer githubSrv.Close()

	depsDev, osv, github := newTestDepsClients(depsDevSrv, osvSrv, githubSrv)

	ctx := context.Background()
	verdict, err := AssembleVerdict(ctx, Deps{DepsDev: depsDev, OSV: osv, GitHub: github}, "github.com/test/mod", "v1.2.3", DefaultDepsRiskConfig(), "policy-hash-123")
	require.NoError(t, err)

	// Empty vulns from reachable OSV should have status ok
	osvSource := findSource(verdict.Sources, "osv")
	assert.NotNil(t, osvSource)
	assert.Equal(t, "ok", osvSource.Status)
	assert.Empty(t, verdict.Vulnerabilities)
}

func TestAssembleVerdict_TransitiveImpactWithGraph(t *testing.T) {
	depsDevSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"versions":[{"versionKey":{"system":"GO","name":"github.com/test/mod","version":"v1.2.3"},"publishedAt":"2024-01-15T10:00:00Z","isDefault":true,"isDeprecated":false,"deprecatedReason":"","licenses":["MIT"],"advisoryKeys":[]}]}`))
	}))
	defer depsDevSrv.Close()

	osvSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"vulns":[]}`))
	}))
	defer osvSrv.Close()

	githubSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"pushed_at":"2024-01-10T10:00:00Z","archived":false,"open_issues_count":5,"stargazers_count":100}`))
	}))
	defer githubSrv.Close()

	depsDev, osv, github := newTestDepsClients(depsDevSrv, osvSrv, githubSrv)

	// Create a simple graph with downstream dependents
	graph := codeintel.NewCodeGraph()
	graph.AddNode("github.com/test/mod", nil, "go")
	graph.AddNode("github.com/consumer/a", []string{"github.com/test/mod"}, "go")
	graph.AddNode("github.com/consumer/b", []string{"github.com/test/mod"}, "go")

	ctx := context.Background()
	verdict, err := AssembleVerdict(ctx, Deps{DepsDev: depsDev, OSV: osv, GitHub: github}, "github.com/test/mod", "v1.2.3", DefaultDepsRiskConfig(), "policy-hash-123", graph)
	require.NoError(t, err)

	// TransitiveImpact should be populated from graph
	assert.NotEmpty(t, verdict.TransitiveImpact)
}

func TestAssembleVerdict_TransitiveImpactNilGraph_HonestOmission(t *testing.T) {
	depsDevSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"versions":[{"versionKey":{"system":"GO","name":"github.com/test/mod","version":"v1.2.3"},"publishedAt":"2024-01-15T10:00:00Z","isDefault":true,"isDeprecated":false,"deprecatedReason":"","licenses":["MIT"],"advisoryKeys":[]}]}`))
	}))
	defer depsDevSrv.Close()

	osvSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"vulns":[]}`))
	}))
	defer osvSrv.Close()

	githubSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"pushed_at":"2024-01-10T10:00:00Z","archived":false,"open_issues_count":5,"stargazers_count":100}`))
	}))
	defer githubSrv.Close()

	depsDev, osv, github := newTestDepsClients(depsDevSrv, osvSrv, githubSrv)

	ctx := context.Background()
	verdict, err := AssembleVerdict(ctx, Deps{DepsDev: depsDev, OSV: osv, GitHub: github}, "github.com/test/mod", "v1.2.3", DefaultDepsRiskConfig(), "policy-hash-123", nil)
	require.NoError(t, err)

	// TransitiveImpact should be empty with flag noting absence
	assert.Empty(t, verdict.TransitiveImpact)
	// Check for the flag in sources or a specific field
}

func TestVerdictCache_LookupHit(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	// Create a cached verdict event
	ctx := context.Background()
	verdict := Verdict{
		Module:           "github.com/test/mod",
		Version:          "v1.2.3",
		Exists:           true,
		AgeDays:          30,
		LatestRelease:    "2024-01-15T10:00:00Z",
		MaintenanceClass: "active",
		License:          "MIT",
		Vulnerabilities:  []Vuln{},
		RiskClass:        "low",
		Confidence:       types.ConfidenceVerified,
		Sources: []SourceSnapshot{
			{Name: "depsdev", Status: "ok", FetchedAt: time.Now().UTC().Format(time.RFC3339)},
		},
	}

	cache := NewVerdictCache(store)
	err := cache.Store(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	// Lookup should hit
	cached, hit, err := cache.Lookup(ctx, "github.com/test/mod", "v1.2.3", "policy-hash-123")
	require.NoError(t, err)
	assert.True(t, hit)
	assert.Equal(t, "github.com/test/mod", cached.Module)
	assert.Equal(t, "v1.2.3", cached.Version)
}

func TestVerdictCache_LookupMiss_VersionChange(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		Exists:     true,
		RiskClass:  "low",
		Confidence: types.ConfidenceVerified,
	}
	cache := NewVerdictCache(store)
	err := cache.Store(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	// Lookup with different version should miss
	cached, hit, err := cache.Lookup(ctx, "github.com/test/mod", "v1.2.4", "policy-hash-123")
	require.NoError(t, err)
	assert.False(t, hit)
	assert.Nil(t, cached)
}

func TestVerdictCache_LookupMiss_PolicyHashChange(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		Exists:     true,
		RiskClass:  "low",
		Confidence: types.ConfidenceVerified,
	}
	cache := NewVerdictCache(store)
	err := cache.Store(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	// Lookup with different policy hash should miss
	cached, hit, err := cache.Lookup(ctx, "github.com/test/mod", "v1.2.3", "policy-hash-456")
	require.NoError(t, err)
	assert.False(t, hit)
	assert.Nil(t, cached)
}

func TestVerdictCache_DoubleStoreNoDuplicateEvent(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		Exists:     true,
		RiskClass:  "low",
		Confidence: types.ConfidenceVerified,
	}
	cache := NewVerdictCache(store)
	err := cache.Store(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)
	err = cache.Store(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	// Query events directly - should have only one DependencyChecked event
	eventType := types.EventDependencyChecked
	events, err := store.Query(ctx, types.Query{Type: &eventType})
	require.NoError(t, err)
	assert.Len(t, events, 1)
}

func TestVerdictCache_RFC3339TimestampFormat(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		Exists:     true,
		RiskClass:  "low",
		Confidence: types.ConfidenceVerified,
		Sources: []SourceSnapshot{
			{Name: "depsdev", Status: "ok", FetchedAt: "2024-01-15T10:00:00Z"},
		},
	}
	cache := NewVerdictCache(store)
	err := cache.Store(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	// Verify timestamp format in stored event
	eventType := types.EventDependencyChecked
	events, err := store.Query(ctx, types.Query{Type: &eventType})
	require.NoError(t, err)
	require.Len(t, events, 1)

	var payload map[string]interface{}
	err = json.Unmarshal(events[0].Payload, &payload)
	require.NoError(t, err)
	// Check that timestamps are RFC3339
	for _, src := range payload["sources"].([]interface{}) {
		srcMap := src.(map[string]interface{})
		fetchedAt := srcMap["fetched_at"].(string)
		_, err := time.Parse(time.RFC3339, fetchedAt)
		require.NoError(t, err, "timestamp %s should be RFC3339", fetchedAt)
	}
}

func findSource(sources []SourceSnapshot, name string) *SourceSnapshot {
	for i := range sources {
		if strings.EqualFold(sources[i].Name, name) {
			return &sources[i]
		}
	}
	return nil
}

func DefaultDepsRiskConfig() config.DepsRiskConfig {
	return config.DepsRiskConfig{
		StaleMonths:      12,
		YoungMonths:      6,
		LicenseAllowlist: []string{"MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "MPL-2.0"},
	}
}

// newTestEventStore creates an in-memory event store for testing
func newTestEventStore(t *testing.T) (types.EventStore, func()) {
	dir := t.TempDir()
	dbPath := dir + "/events.db"
	store, err := eventstore.NewEventStore(dbPath)
	require.NoError(t, err)
	return store, func() { store.Close() }
}
