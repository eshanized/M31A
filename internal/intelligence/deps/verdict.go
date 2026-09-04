package deps

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/google/uuid"
)

// Verdict is the complete dependency check result containing all source
// data, risk classification, and metadata for rendering and caching.
type Verdict struct {
	Module           string           `json:"module"`
	Version          string           `json:"version"`
	Exists           bool             `json:"exists"`
	AgeDays          int              `json:"age_days"`
	LatestRelease    string           `json:"latest_release"`
	MaintenanceClass string           `json:"maintenance_class"`
	License          string           `json:"license"`
	Vulnerabilities  []Vuln           `json:"vulnerabilities"`
	TransitiveImpact []string         `json:"transitive_impact"`
	APIStability     string           `json:"api_stability"`
	Popularity       PopularityInfo   `json:"popularity"`
	RiskClass        string           `json:"risk_class"` // high | medium | low
	Confidence       types.Confidence `json:"confidence"`
	Sources          []SourceSnapshot `json:"sources"`
	PolicyHash       string           `json:"policy_hash,omitempty"`
}

// PopularityInfo holds popularity metrics from deps.dev project scorecard.
type PopularityInfo struct {
	Stars      int     `json:"stars"`
	Forks      int     `json:"forks"`
	OpenIssues int     `json:"open_issues"`
	Scorecard  float64 `json:"scorecard"`
}

// SourceSnapshot records the state of a single data source at verdict time.
type SourceSnapshot struct {
	Name      string `json:"name"`
	Status    string `json:"status"` // ok | unavailable | skipped
	FetchedAt string `json:"fetched_at"`
}

// Deps bundles the optional data sources for verdict assembly.
// All fields are optional — callers may provide nil for any source.
type Deps struct {
	DepsDev *DepsDevClient
	OSV     *OSVClient
	GitHub  *GitHubEnricher
}

// AssembleVerdict orchestrates all available sources to produce a complete
// Verdict. Per-source degradation: each client failure records its source
// as unavailable while remaining sources still populate their fields. Overall
// Confidence is capped at Speculative when ANY source is unreachable.
//
// The optional graph parameter enables transitive impact analysis. When nil,
// TransitiveImpact is empty (no graph source added to sources).
func AssembleVerdict(ctx context.Context, deps Deps, module, version string, rules config.DepsRiskConfig, policyHash string, graph ...*codeintel.CodeGraph) (Verdict, error) {
	var g *codeintel.CodeGraph
	if len(graph) > 0 {
		g = graph[0]
	}

	verdict := Verdict{
		Module:           module,
		Version:          version,
		Vulnerabilities:  []Vuln{},
		TransitiveImpact: []string{},
		Sources:          []SourceSnapshot{},
		Popularity:       PopularityInfo{},
	}

	now := time.Now().UTC()
	sourcesQueried := 0
	sourcesFailed := 0

	// DepsDev client
	if deps.DepsDev != nil {
		sourcesQueried++
		fetchedAt := now.Format(time.RFC3339)
		pkg, err := deps.DepsDev.GetPackage(ctx, module)
		if err != nil {
			sourcesFailed++
			verdict.Sources = append(verdict.Sources, SourceSnapshot{
				Name: "depsdev", Status: "unavailable", FetchedAt: fetchedAt,
			})
		} else {
			verdict.Sources = append(verdict.Sources, SourceSnapshot{
				Name: "depsdev", Status: "ok", FetchedAt: fetchedAt,
			})
			processDepsDevPackage(&verdict, pkg, version)
		}
	} else {
		verdict.Sources = append(verdict.Sources, SourceSnapshot{
			Name: "depsdev", Status: "skipped", FetchedAt: now.Format(time.RFC3339),
		})
	}

	// OSV client
	if deps.OSV != nil {
		sourcesQueried++
		fetchedAt := now.Format(time.RFC3339)
		vulnQuery, err := deps.OSV.QueryVulns(ctx, module, version)
		if err != nil {
			sourcesFailed++
			verdict.Sources = append(verdict.Sources, SourceSnapshot{
				Name: "osv", Status: "unavailable", FetchedAt: fetchedAt,
			})
		} else {
			verdict.Sources = append(verdict.Sources, SourceSnapshot{
				Name: "osv", Status: "ok", FetchedAt: fetchedAt,
			})
			verdict.Vulnerabilities = vulnQuery.Vulns
		}
	} else {
		verdict.Sources = append(verdict.Sources, SourceSnapshot{
			Name: "osv", Status: "skipped", FetchedAt: now.Format(time.RFC3339),
		})
	}

	// GitHub enrichment client
	if deps.GitHub != nil {
		sourcesQueried++
		fetchedAt := now.Format(time.RFC3339)
		ownerRepo, err := RepoFromModule(module)
		if err != nil {
			verdict.Sources = append(verdict.Sources, SourceSnapshot{
				Name: "github", Status: "skipped", FetchedAt: fetchedAt,
			})
			// Not counted as queried since we couldn't even determine the repo
			sourcesQueried--
		} else {
			repo, err := deps.GitHub.RepoActivity(ctx, ownerRepo)
			if err != nil {
				sourcesFailed++
				verdict.Sources = append(verdict.Sources, SourceSnapshot{
					Name: "github", Status: "unavailable", FetchedAt: fetchedAt,
				})
			} else {
				verdict.Sources = append(verdict.Sources, SourceSnapshot{
					Name: "github", Status: "ok", FetchedAt: fetchedAt,
				})
				processGitHubRepo(&verdict, repo)
			}
		}
	} else {
		verdict.Sources = append(verdict.Sources, SourceSnapshot{
			Name: "github", Status: "skipped", FetchedAt: now.Format(time.RFC3339),
		})
	}

	// Compute risk classification from assembled data
	riskInput := buildRiskInput(verdict)
	assessment := ClassifyRisk(riskInput, rules)
	verdict.RiskClass = assessment.RiskClass

	// Determine confidence:
	// - Speculative if any queried source failed
	// - Verified if all queried sources succeeded AND no risk rules triggered
	// - Likely if all queried sources succeeded BUT risk rules triggered
	if sourcesFailed > 0 {
		verdict.Confidence = types.ConfidenceSpeculative
	} else if len(assessment.TriggeredRules) == 0 {
		verdict.Confidence = types.ConfidenceVerified
	} else {
		verdict.Confidence = types.ConfidenceLikely
	}

	// Transitive impact from graph
	if g != nil {
		downstream := g.Downstream(module, 3) // depth 3
		verdict.TransitiveImpact = downstream
	}

	return verdict, nil
}

// processDepsDevPackage extracts package metadata into the verdict.
func processDepsDevPackage(v *Verdict, pkg PackageInfo, targetVersion string) {
	if len(pkg.Versions) == 0 {
		v.Exists = false
		return
	}

	v.Exists = true

	// Find the target version or latest
	var targetVer *VersionInfo
	for i := range pkg.Versions {
		if pkg.Versions[i].VersionKey.Version == targetVersion {
			targetVer = &pkg.Versions[i]
			break
		}
	}
	if targetVer == nil {
		// Use latest (first in list, assuming sorted)
		targetVer = &pkg.Versions[0]
		v.Version = targetVer.VersionKey.Version
	}

	v.LatestRelease = targetVer.PublishedAt.UTC().Format(time.RFC3339)
	age := time.Since(targetVer.PublishedAt.UTC())
	v.AgeDays = int(age.Hours() / 24)

	if targetVer.IsDeprecated {
		v.MaintenanceClass = "deprecated"
	} else if v.AgeDays >= 365 {
		v.MaintenanceClass = "stale"
	} else {
		v.MaintenanceClass = "active"
	}

	// Licenses: use first non-empty
	for _, lic := range targetVer.Licenses {
		if lic != "" {
			v.License = lic
			break
		}
	}
}

// processGitHubRepo extracts GitHub repo metadata into the verdict.
func processGitHubRepo(v *Verdict, repo RepoActivity) {
	v.Popularity.Stars = repo.Stars
	v.Popularity.OpenIssues = repo.OpenIssuesAndPRs
	if repo.Archived {
		v.MaintenanceClass = "archived"
	}
}

// buildRiskInput constructs RiskInput from the assembled verdict.
func buildRiskInput(v Verdict) RiskInput {
	return RiskInput{
		HasVulns:              len(v.Vulnerabilities) > 0,
		VulnCount:             len(v.Vulnerabilities),
		LicenseSPDX:           v.License,
		LatestReleaseAgeDays:  v.AgeDays,
		RepoArchived:          v.MaintenanceClass == "archived",
		PopularityStars:       v.Popularity.Stars,
		ProjectAgeDays:        v.AgeDays,  // approximation
		APIInstabilitySignals: []string{}, // would be derived from version history
	}
}

// EmitDependencyChecked appends a DependencyChecked event to the store.
// If store is nil, this is a no-op (safe for headless runs without .m31a).
func EmitDependencyChecked(ctx context.Context, store types.EventStore, payload Verdict) error {
	if store == nil {
		return nil
	}

	data, err := json.Marshal(payload)
	if err != nil {
		return fmt.Errorf("marshal verdict: %w", err)
	}

	evt := types.Event{
		ID:        uuid.New(),
		Type:      types.EventDependencyChecked,
		Timestamp: time.Now().UTC(),
		Payload:   data,
		Metadata: types.EventMetadata{
			SchemaVersion: 1,
			Source:        "intelligence",
		},
	}
	return store.Append(ctx, evt)
}

// VerdictCache provides event-backed caching for dependency verdicts.
// Keys are (module, version, policyHash). Cache replays DependencyChecked
// events via the public Query API to build a latest-per-key in-memory index.
type VerdictCache struct {
	store types.EventStore
	index map[string]*CachedVerdict // key -> cached verdict
}

// CachedVerdict wraps a verdict with cache metadata.
type CachedVerdict struct {
	Verdict    Verdict
	PolicyHash string
	CachedAt   time.Time
}

// cacheKey generates the composite cache key.
func cacheKey(module, version, policyHash string) string {
	h := sha256.New()
	h.Write([]byte(module + "|" + version + "|" + policyHash))
	return fmt.Sprintf("%x", h.Sum(nil)[:16])
}

// NewVerdictCache creates a new cache backed by the event store.
func NewVerdictCache(store types.EventStore) *VerdictCache {
	return &VerdictCache{
		store: store,
		index: make(map[string]*CachedVerdict),
	}
}

// Lookup checks the cache for a matching verdict. Returns (nil, false, nil) on miss.
func (c *VerdictCache) Lookup(ctx context.Context, module, version, policyHash string) (*Verdict, bool, error) {
	key := cacheKey(module, version, policyHash)

	// If we have a memory index entry, validate it
	if cached, ok := c.index[key]; ok {
		if cached.PolicyHash == policyHash && cached.Verdict.Version == version {
			return &cached.Verdict, true, nil
		}
		// Stale entry - remove and fall through to replay
		delete(c.index, key)
	}

	// Replay DependencyChecked events to rebuild index
	events, err := c.store.Query(ctx, types.Query{
		Type:  ptrEventType(types.EventDependencyChecked),
		Limit: 10000,
	})
	if err != nil {
		return nil, false, err
	}

	// Build latest-per-key index using policyHash from stored events
	for _, evt := range events {
		var v Verdict
		if err := json.Unmarshal(evt.Payload, &v); err != nil {
			continue // skip malformed
		}
		// Use the policyHash from the stored event
		storedPolicyHash := v.PolicyHash
		k := cacheKey(v.Module, v.Version, storedPolicyHash)
		if v.Module == module && v.Version == version && storedPolicyHash == policyHash {
			c.index[k] = &CachedVerdict{
				Verdict:    v,
				PolicyHash: storedPolicyHash,
				CachedAt:   evt.Timestamp.UTC(),
			}
		}
	}

	// Check again after replay
	if cached, ok := c.index[key]; ok {
		if cached.PolicyHash == policyHash && cached.Verdict.Version == version {
			return &cached.Verdict, true, nil
		}
	}

	return nil, false, nil
}

// Store appends a DependencyChecked event if no identical key event exists.
// Recheck with unchanged version and policy hash produces zero new events.
func (c *VerdictCache) Store(ctx context.Context, verdict Verdict, policyHash string) error {
	// First check if we already have this exact key
	key := cacheKey(verdict.Module, verdict.Version, policyHash)
	if cached, ok := c.index[key]; ok {
		if cached.PolicyHash == policyHash && cached.Verdict.Version == verdict.Version {
			// Already cached - no new event
			return nil
		}
	}

	// Include policyHash in verdict for event storage
	verdict.PolicyHash = policyHash

	// Emit the event
	err := EmitDependencyChecked(ctx, c.store, verdict)
	if err != nil {
		return err
	}

	// Update memory index
	c.index[key] = &CachedVerdict{
		Verdict:    verdict,
		PolicyHash: policyHash,
		CachedAt:   time.Now().UTC(),
	}
	return nil
}

func ptrEventType(et types.EventType) *types.EventType {
	return &et
}
