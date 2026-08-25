package deps

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"strings"
	"time"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
)

// ErrNotEnrichable reports that a module path does not map to a GitHub
// repository this enricher can query (non-github.com hosts, incomplete
// paths).
var ErrNotEnrichable = errors.New("module path not enrichable via GitHub")

// RepoActivity is the GitHub repository snapshot consumed by risk rules:
// PushedAt feeds release-stale checks, Archived fires repo-archived, and
// OpenIssuesAndPRs is labeled honestly because GitHub's open_issues_count
// includes pull requests (Pitfall 6).
type RepoActivity struct {
	PushedAt         time.Time `json:"pushed_at"`
	Archived         bool      `json:"archived"`
	OpenIssuesAndPRs int       `json:"open_issues_count"`
	Stars            int       `json:"stargazers_count"`
}

// GitHubEnricher queries repo activity from api.github.com. The optional
// token travels ONLY in the Authorization header of outgoing requests —
// it is never logged or embedded in error strings (T-04-06c). Callers
// read GITHUB_TOKEN at their level; this package stays env-free.
type GitHubEnricher struct {
	api   *registryClient
	token string
}

// NewGitHubEnricher creates an enricher against baseURL (default
// production: https://api.github.com). A nil httpClient applies the
// shared hardened default. An empty token means unauthenticated requests.
func NewGitHubEnricher(baseURL string, httpClient *http.Client, token string) *GitHubEnricher {
	api := newRegistryClient(baseURL, httpClient)
	// GitHub-specific status mapping ahead of the generic handling:
	// 403 with an exhausted rate budget is the anonymous-quota signature,
	// surfaced as a typed error whose hint names GITHUB_TOKEN.
	api.onStatus = func(resp *http.Response) error {
		if resp.StatusCode == http.StatusForbidden && resp.Header.Get("X-RateLimit-Remaining") == "0" {
			return fmt.Errorf("%w: GitHub API rate limit exhausted; set GITHUB_TOKEN to raise the limit", coreerrors.ErrRateLimited)
		}
		return nil
	}
	return &GitHubEnricher{api: api, token: token}
}

// RepoFromModule extracts owner/repo from a module path: github.com
// prefixes with at least host/owner/repo segments qualify (subpackages
// and /vN major suffixes collapse onto the root repo); anything else is
// ErrNotEnrichable.
func RepoFromModule(module string) (string, error) {
	parts := strings.Split(strings.Trim(module, "/"), "/")
	if len(parts) >= 3 && parts[0] == "github.com" && parts[1] != "" && parts[2] != "" {
		return parts[1] + "/" + parts[2], nil
	}
	return "", fmt.Errorf("%w: %s", ErrNotEnrichable, module)
}

// RepoActivity fetches GET /repos/{owner}/{repo}. A missing repo maps to
// ErrPackageNotFound so verdict assembly records exists=false instead of
// failing. PushedAt normalizes to UTC.
func (g *GitHubEnricher) RepoActivity(ctx context.Context, ownerRepo string) (RepoActivity, error) {
	header := http.Header{}
	if g.token != "" {
		header.Set("Authorization", "Bearer "+g.token)
	}
	data, err := g.api.do(ctx, http.MethodGet, "/repos/"+ownerRepo, nil, header)
	if err != nil {
		return RepoActivity{}, err
	}
	var out RepoActivity
	if err := decodeJSON(data, &out); err != nil {
		return RepoActivity{}, err
	}
	out.PushedAt = out.PushedAt.UTC()
	return out, nil
}
