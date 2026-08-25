package deps

import (
	"context"
	"fmt"
	"net/http"
	"sort"
	"strconv"
	"strings"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
)

// Vuln is the minimal OSV advisory projection needed for verdict
// assembly. FixedIn is computed locally from affected ranges events; it
// never appears in the upstream payload.
type Vuln struct {
	ID      string   `json:"id"`
	Summary string   `json:"summary"`
	Aliases []string `json:"aliases"`
	FixedIn []string `json:"fixedIn"`
}

// VulnQuery carries a vulnerability lookup outcome. QueriedOK separates
// "OSV answered and returned N advisories" from "the source could not be
// reached": zero vulns with QueriedOK=false is an unknown state, never a
// clean one (T-04-06d; feeds plan 04-07 confidence capping).
type VulnQuery struct {
	Vulns     []Vuln
	QueriedOK bool
}

// osvVuln mirrors only the OSV-schema subset this package consumes;
// unknown fields are tolerated because the schema evolves.
type osvVuln struct {
	ID               string         `json:"id"`
	Summary          string         `json:"summary"`
	Aliases          []string       `json:"aliases"`
	Affected         []osvAffected  `json:"affected"`
	DatabaseSpecific map[string]any `json:"database_specific"`
}

type osvAffected struct {
	Ranges []osvRange `json:"ranges"`
}

type osvRange struct {
	Events []osvEvent `json:"events"`
}

type osvEvent struct {
	Introduced string `json:"introduced"`
	Fixed      string `json:"fixed"`
}

// OSVClient queries api.osv.dev's POST /v1/query endpoint. Only the fixed
// default host is ever contacted; tests inject httptest servers.
type OSVClient struct {
	api *registryClient
}

// NewOSVClient creates a client against baseURL (default production:
// https://api.osv.dev). A nil httpClient applies the shared hardened
// default.
func NewOSVClient(baseURL string, httpClient *http.Client) *OSVClient {
	return &OSVClient{api: newRegistryClient(baseURL, httpClient)}
}

// normalizeGoVersion ensures the Go-ecosystem semver form OSV expects:
// v-less input gains the leading v; already-prefixed input is unchanged;
// build-metadata suffixes are preserved verbatim so v1.0.0+build stays a
// distinct lookup from v1.0.0 (module-matching case rules per DEPEND-02).
func normalizeGoVersion(version string) string {
	if version == "" || strings.HasPrefix(version, "v") {
		return version
	}
	return "v" + version
}

// QueryVulns posts {"package":{"name",ecosystem:"Go"},"version"} to
// /v1/query and returns advisories sorted severity-descending then
// advisory-ID ascending. Severity rank derives from the advisory CVSS
// score when present (database_specific.cvss_score), else the GitHub-style
// severity enum (CRITICAL>HIGH>MODERATE/MEDIUM>LOW); advisories without
// either rank after all scored ones with lexical ID ordering as the
// documented fallback.
func (c *OSVClient) QueryVulns(ctx context.Context, module, version string) (VulnQuery, error) {
	body := fmt.Sprintf(`{"package":{"name":%q,"ecosystem":"Go"},"version":%q}`, module, normalizeGoVersion(version))
	data, err := c.api.do(ctx, http.MethodPost, "/v1/query", []byte(body), nil)
	if err != nil {
		// Transport failure: zero vulns plus QueriedOK=false so callers
		// can distinguish unknown from clean (DEPEND-02).
		return VulnQuery{}, fmt.Errorf("%w: osv query failed for %s@%s", coreerrors.ErrSourceUnreachable, module, version)
	}
	var payload struct {
		Vulns []osvVuln `json:"vulns"`
	}
	if err := decodeJSON(data, &payload); err != nil {
		return VulnQuery{}, err
	}
	vulns := make([]Vuln, 0, len(payload.Vulns))
	for _, raw := range payload.Vulns {
		v := Vuln{ID: raw.ID, Summary: raw.Summary}
		v.Aliases = append(v.Aliases, raw.Aliases...)
		for _, aff := range raw.Affected {
			for _, rng := range aff.Ranges {
				for _, ev := range rng.Events {
					if ev.Fixed != "" {
						v.FixedIn = append(v.FixedIn, ev.Fixed)
					}
				}
			}
		}
		vulns = append(vulns, v)
	}
	sort.SliceStable(vulns, func(i, j int) bool {
		si, sj := vulnSeverityRank(payload.Vulns, vulns[i].ID), vulnSeverityRank(payload.Vulns, vulns[j].ID)
		if si != sj {
			return si > sj // severity descending
		}
		return vulns[i].ID < vulns[j].ID // tie-break: advisory ID ascending
	})
	return VulnQuery{Vulns: vulns, QueriedOK: true}, nil
}

// severityRank maps a named severity to a sortable magnitude.
var severityRank = map[string]float64{
	"CRITICAL": 4,
	"HIGH":     3,
	"MODERATE": 2,
	"MEDIUM":   2,
	"LOW":      1,
}

// vulnSeverityRank extracts the sort magnitude for an advisory by ID:
// numeric CVSS score first, then the enum mapping, else 0 ("unknown sorts
// last" fallback).
func vulnSeverityRank(raws []osvVuln, id string) float64 {
	for _, raw := range raws {
		if raw.ID != id {
			continue
		}
		if raw.DatabaseSpecific == nil {
			return 0
		}
		if score, ok := raw.DatabaseSpecific["cvss_score"]; ok {
			switch s := score.(type) {
			case float64:
				return s
			case string:
				if f, err := strconv.ParseFloat(s, 64); err == nil {
					return f
				}
			}
		}
		if name, ok := raw.DatabaseSpecific["severity"].(string); ok {
			if rank, ok := severityRank[strings.ToUpper(name)]; ok {
				return rank
			}
		}
		return 0
	}
	return 0
}
